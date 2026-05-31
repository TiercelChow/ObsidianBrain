// 核心服务层
pub mod chunker; // 智能分块器
pub mod markdown_parser; // Markdown 解析器
pub mod memory_service; // 记忆服务 (索引管线 + CRUD)
#[allow(dead_code)]
pub mod search_engine; // 混合搜索引擎 (RRF)
                       // TODO: Phase 2-3 实现
                       // pub mod timeline;     // 时间线
                       // pub mod code_repo;    // 代码仓管理
                       // pub mod inspiration;  // 灵感熔炉
                       // pub mod radar;        // 智识雷达
