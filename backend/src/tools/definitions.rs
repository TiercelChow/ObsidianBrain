//! JSON Schema definitions for all 8 core tool handlers.
//!
//! Each function returns a `serde_json::Value` describing the tool's input
//! parameters, suitable for use as `input_schema()` in `ToolHandler` implementations
//! and for JSON Schema validation in the tool protocol layer.

use serde_json::{json, Value};

/// Schema for `search_notes` — note-level hybrid search.
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
            },
            "tags": {
                "type": "array",
                "items": { "type": "string" },
                "description": "按标签过滤（匹配所有指定标签）"
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

/// Schema for `search_memory` — chunk-level hybrid search.
pub fn search_memory_schema() -> Value {
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
            },
            "tags": {
                "type": "array",
                "items": { "type": "string" },
                "description": "按标签过滤（匹配所有指定标签）"
            }
        },
        "required": ["query"],
        "additionalProperties": false
    })
}

/// Schema for `add_memory` — create a new memory chunk.
pub fn add_memory_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "note_path": {
                "type": "string",
                "description": "所属笔记路径"
            },
            "content": {
                "type": "string",
                "description": "记忆内容"
            },
            "tags": {
                "type": "array",
                "items": { "type": "string" },
                "description": "标签列表"
            }
        },
        "required": ["note_path", "content"],
        "additionalProperties": false
    })
}

/// Schema for `update_memory` — update an existing memory chunk's content.
pub fn update_memory_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "memory_id": {
                "type": "string",
                "description": "记忆 ID（UUID 格式）"
            },
            "content": {
                "type": "string",
                "description": "更新后的内容"
            }
        },
        "required": ["memory_id", "content"],
        "additionalProperties": false
    })
}

/// Schema for `forget_memory` — delete a memory chunk.
pub fn forget_memory_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "memory_id": {
                "type": "string",
                "description": "记忆 ID（UUID 格式）"
            }
        },
        "required": ["memory_id"],
        "additionalProperties": false
    })
}

/// Schema for `get_memory_stats` — return indexed content statistics.
pub fn get_memory_stats_schema() -> Value {
    json!({
        "type": "object",
        "properties": {},
        "additionalProperties": false
    })
}
