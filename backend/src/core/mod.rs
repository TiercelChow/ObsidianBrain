// 核心服务层
pub mod code_repo; // 代码仓管理
#[allow(dead_code)]
pub mod markdown_parser; // Markdown 解析器 (未来使用)
pub mod memory_service; // 记忆服务 (通过 Obsidian API)
pub mod timeline; // 时间线
                  // TODO: Phase 3 实现
                  // pub mod inspiration;  // 灵感熔炉
                  // pub mod radar;        // 智识雷达
