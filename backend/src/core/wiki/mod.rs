//! LLM Wiki 知识引擎
//!
//! 基于 Karpathy 的 LLM Wiki 模式：LLM 增量构建和维护持久的 Markdown 知识库。
//! 不同于 RAG（每次从零检索），Wiki 是预编译的、持续复利的知识产物。

pub mod engine;
pub mod index_manager;
pub mod link_graph;
pub mod page_writer;

pub use engine::WikiEngine;
