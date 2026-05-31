//! JSON Schema definitions for all tool handlers.
//!
//! Each function returns a `serde_json::Value` describing the tool's input
//! parameters, suitable for use as `input_schema()` in `ToolHandler` implementations
//! and for JSON Schema validation in the tool protocol layer.

use serde_json::{json, Value};

/// Schema for `search_notes` — note search via Obsidian API.
pub fn search_notes_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "query": {
                "type": "string",
                "description": "搜索查询词"
            },
            "top_k": {
                "type": "integer",
                "default": 5,
                "minimum": 1,
                "maximum": 50,
                "description": "返回结果数量"
            }
        },
        "required": ["query"],
        "additionalProperties": false
    })
}

/// Schema for `get_note` — read a note's full content.
pub fn get_note_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "path": {
                "type": "string",
                "description": "笔记路径（相对于 Vault 根目录）"
            }
        },
        "required": ["path"],
        "additionalProperties": false
    })
}

/// Schema for `list_recent_notes` — list recently modified notes.
pub fn list_recent_notes_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "days": {
                "type": "integer",
                "default": 7,
                "minimum": 1,
                "maximum": 365,
                "description": "最近修改天数范围"
            },
            "limit": {
                "type": "integer",
                "default": 20,
                "minimum": 1,
                "maximum": 100,
                "description": "返回结果数量上限"
            }
        },
        "additionalProperties": false
    })
}

/// Schema for `get_memory_stats` — return vault statistics.
pub fn get_memory_stats_schema() -> Value {
    json!({
        "type": "object",
        "properties": {},
        "additionalProperties": false
    })
}
