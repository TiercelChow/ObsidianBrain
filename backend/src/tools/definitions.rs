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

// ── Code Repo Tools ──

/// Schema for `add_code_repo` — register a code repository.
pub fn add_code_repo_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "path": {
                "type": "string",
                "description": "代码仓库的本地绝对路径"
            },
            "name": {
                "type": "string",
                "description": "仓库显示名称（唯一标识）"
            }
        },
        "required": ["path", "name"],
        "additionalProperties": false
    })
}

/// Schema for `list_code_repos` — list all registered repos.
pub fn list_code_repos_schema() -> Value {
    json!({
        "type": "object",
        "properties": {},
        "additionalProperties": false
    })
}

/// Schema for `get_repo_detail` — get repo details.
pub fn get_repo_detail_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "name": {
                "type": "string",
                "description": "仓库名称"
            }
        },
        "required": ["name"],
        "additionalProperties": false
    })
}

/// Schema for `link_note_to_repo` — link a note to a repo.
pub fn link_note_to_repo_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "note_path": {
                "type": "string",
                "description": "笔记路径（相对于 Vault 根目录）"
            },
            "repo_name": {
                "type": "string",
                "description": "仓库名称"
            }
        },
        "required": ["note_path", "repo_name"],
        "additionalProperties": false
    })
}

/// Schema for `get_linked_notes` — get notes linked to a repo.
pub fn get_linked_notes_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "repo_name": {
                "type": "string",
                "description": "仓库名称"
            }
        },
        "required": ["repo_name"],
        "additionalProperties": false
    })
}

/// Schema for `open_in_vscode` — open repo in VSCode.
pub fn open_in_vscode_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "name": {
                "type": "string",
                "description": "仓库名称"
            }
        },
        "required": ["name"],
        "additionalProperties": false
    })
}

// ── Timeline Tools ──

/// Schema for `get_timeline` — query timeline events.
pub fn get_timeline_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "start_date": {
                "type": "string",
                "description": "起始日期（YYYY-MM-DD 格式）"
            },
            "end_date": {
                "type": "string",
                "description": "结束日期（YYYY-MM-DD 格式）"
            }
        },
        "required": ["start_date", "end_date"],
        "additionalProperties": false
    })
}
