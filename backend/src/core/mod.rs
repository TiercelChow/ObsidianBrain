// 核心服务层
pub mod agent_tool_gateway;
pub mod book_wiki; // 阅境轩书籍知识库
pub mod book_wiki_export;
pub mod code_repo; // 代码仓管理
pub mod external_research; // 逐任务授权的只读外部研究
#[allow(dead_code)]
pub mod markdown_parser; // Markdown 解析器 (未来使用)
pub mod memory_service; // 记忆服务 (通过 Obsidian API)
pub mod presentation;
pub mod skill_archive;
pub mod tasks; // 个人任务管理
pub mod timeline; // 时间线
