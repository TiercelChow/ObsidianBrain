// 核心服务层
pub mod code_repo; // 代码仓管理
pub mod inspiration; // 灵感熔炉
#[allow(dead_code)]
pub mod markdown_parser; // Markdown 解析器 (未来使用)
pub mod memory_service; // 记忆服务 (通过 Obsidian API)
                         // TODO: Phase 3 实现
                         // pub mod radar;        // 智识雷达
pub mod timeline; // 时间线
