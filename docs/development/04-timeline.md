# 时光机 — 开发设计

> DEV-04 · v3.0 · 2026-10-04 · 已实施
> 上游：[需求设计](../requirement/04-timeline.md) · [实施与风险](2026-10-04-local-timeline-storage.md)

## 1. 分层

Timeline.vue → Tool API / 图片 HTTP API → MemoManager → SqliteStore + TimelineImages。

MemoManager 位于 core/timeline，负责 CRUD、校验、版本和查询；infra/TimelineImages 管理本地图片、安全复制、缩略图、LRU 和 GC。API 不再访问笔记应用。ObsidianClient、MemoryService 和旧搜索/同步 handler 已移除。

原图和缓存跟随配置的数据库目录，默认 ~/.obsidian-brain/timeline/{images,cache}。原图文件名 UUID，逻辑引用保留 Timeline/images/...；旧逻辑路径可映射新 UUID，不必修改小记正文。

默认 brain.db 使用 timeline，其他数据库文件名用 timeline-<文件名哈希> 隔离，不共享 GC。相对路径使用本地同级目录，不回落全局数据区；实际目录由存储面板显示。

## 2. 迁移与数据合同

v59 保留所有旧记录和历史迁移，条件式添加 memos.revision（默认 1），创建：

| 表 | 用途 |
|---|---|
| memos | ID、时间戳、本地日期、正文、图片/标签 JSON、revision；旧 file_path 兼容保留 |
| timeline_images | path → 唯一 filename、MIME、字节大小、创建时间、pending |
| timeline_image_gc | 持久化待清理 path |
| timeline_image_cache | 缓存 key、实际字节大小、last_used 单调 LRU 次序 |

app_state.timeline_cache_limit_bytes 默认 256 * 1024 * 1024；timeline_legacy_directory 保存手动导入的默认旧目录。启动剥离 system_config 的 obsidian/vault 字段，其他设置保留；不自动尝试旧图片迁入。迁移前沿用在线数据库快照，不改历史 SQL。

## 3. 并发与完整性

TimelineImages.mutation 串行化发布、复制、CRUD、读取和 GC；缓存另设 cache_lock。锁顺序 mutation → cache → SQLite 短事务，不在 SQLite 事务内 await。恢复数据库同样持有 mutation，避免与清理竞争。

上传校验真实图片，20 MiB、宽高 16000、解码内存 128 MiB；CPU 解码 spawn_blocking。临时文件写入并 sync 后以不覆盖方式原子发布，登记失败删除新文件；pending 图片保存后成为正式引用。

创建至少正文或图片一项，正文 ≤100000 字节、图片 ≤9、标签 ≤30。新引用必须是存在的受管资产；编辑允许原样保留尚未迁入的旧附件。

更新/删除 WHERE 匹配 ID 与 expected_revision，影响行数不是 1 则回滚并返回 MEMO_VERSION_CONFLICT。编辑保留记录 ID/timestamp/date/created_at，revision +1。

事务同时写待清理队列，不先删原图。GC 再检查全体 memos 图片 JSON 和正文路径引用，无引用才删缓存/原图，再事务移除元数据与队列。权限/IO 失败保留队列，删除返回 pending_cleanup，启动和每 300 秒重试。

discard 只释放 pending 图片，已发布或已被引用的附件仍保护。pending 超过 24 小时且无引用才回收。宕机留下的未登记原图有 24 小时宽限，派生缓存孤儿立即清理；只处理专用目录普通文件，不递归删除旧库。

## 4. LRU 缓存

key 的 SHA-256 是文件名。按字节统计，last_used 升序淘汰；命中设为 MAX +1，避免同一秒碰撞。生成 400×400 内 JPEG，发布前预留空间并淘汰，发布后核对容量。单张超额不缓存；0 禁用；缩容量立即淘汰；清空后保留原上限。

缓存写入失败仍返回生成图片；缩略图生成失败尝试原图。HTTP 使用真实 MIME、Cache-Control: no-store、X-Content-Type-Options: nosniff。没有无限图片内存缓存，原图不参与 LRU。

## 5. 一次性复制

从 memos JSON 和 Markdown/wiki 图片嵌入发现路径。无效 JSON 不直接交给 json_each，正文引用依旧保护。安全路径拒绝绝对路径、..、反斜线、冒号与 NUL，来源 canonicalize 后需在选定旧目录内。目标使用 UUID，读取拒绝符号链接原图。

只复制缺少本地映射的被引用图片，不改正文/记录/旧 Markdown。返回 copied/missing，可重复执行。复制期间小记被删除，刚复制的无引用图片也进入 GC。旧目录永远只读。

触发入口仅为 import_timeline_images 工具（存储面板）或 migrate-timeline-images --database <已有数据库> --source <旧根目录> CLI。CLI 不启动 HTTP/Agent/其他服务；先验证输入和数据库完整性，以 SQLite 在线备份 API 创建一致快照（含 WAL），保留现有备份，再执行 schema 迁移和图片复制。报告复制数量、缺失路径和快照位置。服务启动与每 300 秒只维护本地 GC/LRU，不导入旧图。

## 6. API

| Tool | 合同 |
|---|---|
| create_memo | content、images、tags；返回 ID/时间/revision |
| update_memo | memo_id、expected_revision、content、images、tags |
| delete_memo | memo_id、expected_revision；返回 deleted/pending_cleanup |
| browse_timeline | start_date、end_date、limit、offset |
| search_memos | query、日期/标签、limit、offset；正文及标签搜索 |
| get_timeline_storage | 目录、原图/缓存 bytes、容量、missing_images、pending_cleanup、legacy_directory |
| import_timeline_images | directory；只复制旧图，返回 copied/missing |
| discard_memo_images | paths（最多 9 项），释放新暂存 |
| clear_timeline_image_cache | 清派生缓存，不改原图 |
| save_config | timeline.cache_limit_mb，整数 0–4096，立即生效 |

HTTP：POST /v1/upload/images（multipart images）；GET /v1/timeline/images/*path；GET /v1/timeline/thumbnails/*path。旧 /v1/vault/{images,thumbnails}/*path 仅为本地读取兼容别名，不调用外部 API。

退役：search_notes、get_note、list_recent_notes、list_files、get_memory_stats、sync_memos。健康检查仅 server/sqlite 与本地目录。内部旧 timeline_events 读取工具为兼容保留，与小记编辑独立。

## 7. 前端和验证

复用 MotionModal 编辑器与删除确认；桌面图标、手机操作抽屉。已有附件不再上传；取消只释放本次新资源，失败保留草稿。请求序列防止陈旧查询覆盖新筛选，错误 envelope 不得作为成功。

TimelineStoragePanel 仅用于时光机「图片存储」；首页移除整个系统配置区，不加载配置或挂载存储面板，只请求状态/统计。memoImageUrl 按路径段编码特殊字符，正文旧图也解析到本地入口；日期分组用持久化 date。查看器使用 load/error 初始化缩放，不依赖固定延迟。

测试覆盖格式/路径、只复制迁移、正文引用、共享图与最后引用删除、更新冲突、IO 失败跨重启 GC、LRU/超额/降容量、不可写缓存回退、HTTP 完整生命周期、旧连接设置清理。前端覆盖日期、URL、旧嵌入、错误结果，界面验收使用隔离数据库。

## 8. 备份限制

SQLite 快照与恢复只包含数据库。完整备份需要一致数据库快照与 timeline/images/，备份时暂停写入，缓存无需复制。恢复不能找回已物理删除的照片，页面/手册明确告知；不得因数据库恢复立即无条件清空原图目录。
