# 阅境轩路径选择器

- `GET /v1/reader/locations` 返回 `{ platform, home, roots }`。Windows 用 [GetLogicalDrives](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getlogicaldrives) 枚举当前账号可见的已分配盘符，不逐个探测磁盘；个人目录单独显示。macOS/Linux 提供个人目录和 `/`。这是后端主机的位置，与浏览器所在设备无关。
- 已有路径先用 `stat_local_path` 区分目录和文件；没有路径时，Windows 从「此电脑」开始，其他系统从个人目录开始。通过 `list_local_dir` 的 `depth: 0` 单层浏览；目录无法读取时保留上一个有效位置，关闭或改选时取消旧请求。
- 地址栏接受 Windows 盘符、UNC 共享及常见的资源管理器引号格式。根路径按实际边界计算，向上从盘根或共享根回到「位置」，不会把盘符当成当前工作目录，也不会枚举网络共享。目录和书籍保存前仍由后端验证。
- 面包屑仅展示根位置与当前目录，中间层级收入有界的上级目录菜单。编辑按钮可切换到地址输入，不依赖横向滚动。手机和桌面共用系统材质和图标。
- `verify-reader-path-picker.mjs` 使用隔离的 Windows/macOS/Linux 路径响应覆盖窄屏、桌面、不可读盘符、切盘、深层目录和 UNC 书籍回填；Rust 单测验证主机位置和盘符掩码，前端单测验证各类根路径及书名。
