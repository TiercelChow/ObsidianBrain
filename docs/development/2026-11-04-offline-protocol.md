# 手机离线只读同步协议

2026-11-04。实现位于 `backend/src/api/handlers/offline.rs`。手机端方案见同级 HMObsidianBrain 项目的 `docs/2026-11-04-offline-readonly-design.md`。

## 身份

`GET /v1/offline/identity` 返回：

```json
{"protocol":1,"instance_id":"UUID","dataset_id":"UUID","generation":"UUID","capabilities":["text_snapshot","resource_manifest","full_replacement","chunked_records","mobile_incremental"]}
```

实例 ID 保存在数据库同目录的独立标识文件，数据集 ID 与代次保存在 SQLite 的专用 `app_state` 键中。普通重启保持不变；清除数据库产生新数据集；通过应用的数据库恢复入口恢复时更换代次。直接在应用外覆盖数据库文件不属于恢复入口的保证范围。更新版本与上述身份一起使用。手机切换数据集时，不能向新数据集沿用旧副本的版本。

## 手机增量同步

支持 `mobile_incremental` 能力时，手机使用同一快照接口，显式传入 `profile:"mobile"`：

```json
{"profile":"mobile","book_ids":[],"reader_files":[],"include_resources":false,"known_versions":{"overview":"hex","timeline":"hex","tasks":"hex"}}
```

自动更新响应为 `mode:"mobile_incremental"`。服务以导出的内容计算各范围的 SHA-256 版本，版本覆盖顺序稳定的记录及相应资源的路径和内容哈希。首页的生成时间按本地日期固定，避免每次轮询都改变版本。客户端只传已成功启用的版本。

| 版本范围 | 自动保存内容 |
| --- | --- |
| `overview` | 首页、阅境轩书架/目录/历史、Wiki 知识库与全局研究任务列表 |
| `timeline` | 小记全文、标签与受管附件缩略图 |
| `tasks` | 全部任务列表、父子任务、详情和进展 |

响应新增 `versions` 和 `unchanged_scopes`。服务只导出发生变化范围的记录和资源清单；三个版本都一致时，两项数量为 0，手机无需请求分页或替换本机副本。一个业务更新可以改变多个范围，例如新增小记也会改变首页摘要。若核心范围读取失败，响应省略该范围的版本，不能把失败当成空内容或确认删除。

`reader_metadata:<书籍ID>` 和 `timeline_assets` 单独报告不可用的文集路径或缩略图。此时有效的概览和小记全文仍可启用，手机可保留已有的相关文件树或缩略图。版本覆盖本次实际能导出的目录和缩略图，资源恢复后会重新产生版本，长期缺失时也不会重复传输同一批正文。缩略图生成失败时，手机导出不会回退到原图。自动同步不传阅读正文、Wiki 详情、历史问答或研究报告和成果附件。

### 下载所选阅读内容

选择文集时传 `book_ids:["书籍ID"]`。选择一篇 Markdown 或一份 PDF 时传 `reader_files:[{"book_id":"书籍ID","path":"文件绝对路径"}]`。两类请求都传 `profile:"mobile", "include_resources":true`，响应为 `mode:"mobile_reader"`。单文件须为对应书架文集中的 Markdown 或 PDF，不接受路径穿越和符号链接。

所选阅读内容以 `reader:<书籍ID>` 标记覆盖与版本。版本包含实际选择、书籍元数据、目录、Markdown 正文及相关资源 SHA-256。阅读位置由书架同步，不会使已下载内容失效。客户端针对同一份下载选择传回此版本，版本相同时不重复传正文或资源清单。版本有变化时，手机可按路径和 SHA-256 继续复用本机已验证的文件。Markdown 通过 `read_local_file` 记录传递，其本地图片和 PDF 通过资源清单传递，不再额外下载一份 Markdown 原文件。若目录、正文或依赖未完整导出，不产生阅读版本；手机保留原下载。

## 不可变文本快照（旧版兼容）

`POST /v1/offline/snapshots` 的请求为 `{"book_ids":[],"include_resources":false}`，两个字段均可省略。POST 只准备离线导出，不创建业务内容或模型运行。服务先使用 SQLite Online Backup 得到一致副本，再对副本运行明确白名单内的读取处理器；不在大文件下载期间持有业务数据库事务。

响应包含 `snapshot_id`、`identity`、`created_at`、`expires_at`、`record_count`、`resource_count`、`mode:"full_replacement"` 和 `coverage`。

`GET /v1/offline/snapshots/:id?cursor=0&limit=20` 返回：

```json
{"snapshot_id":"UUID","records":[{"tool":"get_reader_books","arguments":{},"result":{"books":[]}}],"next_cursor":"20","complete":false,"coverage":[{"scope":"reader","complete":true,"errors":[]}]}
```

`next_cursor` 是下一页游标字符串，结束时为 null。每页最多 100 项，响应记录部分按约 1 MiB 控制；单条大结果按 UTF-8 边界分为不超过 256 KiB 的片段：

```json
{"tool":"get_knowledge_entry","arguments":{"entry_id":"UUID"},"result_json":"JSON fragment","chunk_index":0,"chunk_count":3}
```

片段不能单独作为完整记录使用，必须按同一工具与参数、片段序号收齐并解析 JSON 后再发布。快照游标计算的是传输记录数，包含片段。

消费完所需记录与资源清单后，调用 `POST /v1/offline/snapshots/:id/release`（空 JSON 请求）释放导出缓存；也支持 `DELETE /v1/offline/snapshots/:id`。响应 `{"released":true}`，重复释放返回 false。冻结资源下载 URL 独立存续，释放文本快照不会使已经获取的资源下载地址失效。客户端应在完成或取消时释放，避免达到同时快照数量上限。

默认包括书架与历史、全部小记文本、任务根与子任务详情和进展、知识库、条目、历史问答、已有研究工作区与阶段正文、已保存完整报告、依据、配置文档与 Skills。不会运行配置探测、问答补存、编译、研究或模型。配置的运行时状态仅说明“离线快照未运行环境检查”，不代表实时健康检查失败。

`get_memo` 和 `home_overview` 是适配 GET 页面数据的快照记录名，不能据此作为普通 RPC 调用。普通读取记录保持现有工具结果形状。

### 覆盖与替换范围

| scope | 含义 |
| --- | --- |
| `home` | 保存时的首页概览 |
| `reader` | 全书架与历史元数据 |
| `timeline` | 全部小记文本 |
| `tasks` | 全部任务详情及列表 |
| `wiki` | 全局 Wiki 条目、会话、工作区、已有阶段与 Skills |
| `wiki:<知识库ID>` | 指定知识库元数据、分页列表、配置文档 |
| `wiki_reports` | 读取已有完整报告时的失败范围 |
| `reader_metadata:<书籍ID>` | 文件树元数据 |
| `reader:<书籍ID>` | 仅显式选择书籍时导出的正文与资源范围 |
| `timeline_assets` / `wiki_assets` | 选取附件导出时的失败范围 |

未选择正文的文集只保存文件树，不产生“完整文集”的覆盖声明。目录无法访问、部分图片缺失等记录到对应范围的 `errors`，不能将失败当成空库。客户端只有在范围完整、全部分页与片段校验成功后才能替换该范围并确认删除；部分导出可合并有效内容，但保留此前副本且不能声称覆盖完整。

旧版后台文本同步不能清除用户此前浏览保存的 Markdown 正文或固定离线文件，因为默认文本快照不覆盖它们。正在运行、尚无报告的任务通过工作区状态呈现，不伪造报告正文。

选取 `book_ids` 后导出文集支持的 Markdown、PDF 和图片，并解析 Markdown 本地图片依赖。外链保留为链接，不进入离线包。缺失本地图片、不可访问目录、超过目录深度、单篇 Markdown 超过 4 MiB 或符号链接等未支持的范围使该文集为部分完成。`book_ids` 非空时只准备所选文集的资源，不能顺带下载全部照片。`book_ids` 为空且 `include_resources:true` 时准备受管小记原图、缩略图和研究附件。资源清单分页地址为 `GET /v1/offline/snapshots/:id/resources?cursor=0&limit=20`，结果字段为 `snapshot_id`、`resources`、`next_cursor`、`complete`。

## 冻结资源

`GET /v1/offline/resource?path=<编码后的现有服务相对GET地址>` 支持阅境轩原文件、小记原图/缩略图和研究成果附件。返回：

```json
{"id":"UUID","request_path":"/v1/reader/raw?path=...","path":"/v1/reader/raw?path=...","size":123,"size_bytes":123,"sha256":"hex","version":"hex","download_path":"/v1/offline/resources/UUID","mime_type":"application/pdf","identity":{},"expires_at":"RFC3339"}
```

`identity` 与身份接口结构相同；`version` 当前使用内容 SHA-256。先复制与校验源文件，再发布清单；后续通过 `GET download_path` 读取冻结字节，原文件修改不会改变旧下载 URL 的内容。资源更新过程不能覆盖此前已验证文件。文件和下载响应包含长度、MIME 和 ETag；下载为完整文件流，目前不支持 Range，客户端不能将 200 响应错误拼接到旧断点后。

冻结资源当前有效期 30 分钟。相同路径和内容重用冻结副本。Unix 上文件身份、大小和精确修改时间均相同的有效副本可直接复用；新副本边复制边计算哈希，并检查复制前后文件身份及时间，缺乏可靠文件标识的平台用第二次读取确认。过期或不存在返回 404，重新获取清单。单文件上限 1 GiB，冻结文件总预算 2 GiB；超限返回 413，不发布半文件。资源位于数据库旁独立临时导出目录，仅清理自己命名的过期文件，异常退出的遗留文件计入磁盘预算并在之后清理。

文本快照有效期 30 分钟，同时最多 3 个，每次最多选 100 本，单次最多 100000 条传输记录、128 MiB 文本。超出文本保存范围时标记部分覆盖；无法枚举的超大范围拒绝导出，不静默截断成完整快照。客户端应及时消费，过期后重新开始暂存。实例、数据集或代次在导出期间改变时返回 409，不能把不同数据集混入同一批。

## 验证

测试覆盖身份重启与隔离、应用内恢复旋转代次、不可变分页、原文件变化后的冻结下载、多字节大结果分片、凭据字段移除、禁止执行工具、文集依赖与缺失覆盖、手机首轮与无变化同步、单范围变化、单篇下载版本及依赖更新。手机端仍需独立验证本地存储、只读入口、主题、重启和重连；服务端测试不能代替设备验证。
