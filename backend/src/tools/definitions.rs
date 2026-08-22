//! JSON Schema definitions for all tool handlers.
//!
//! Each function returns a `serde_json::Value` describing the tool's input
//! parameters, suitable for use as `input_schema()` in `ToolHandler` implementations
//! and for JSON Schema validation in the tool protocol layer.

use serde_json::{json, Value};

fn expected_task_version_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "revision": { "type": "integer", "minimum": 1 },
            "content_hash": { "type": "string", "pattern": "^sha256:[0-9a-f]{64}$" }
        },
        "required": ["revision", "content_hash"],
        "additionalProperties": false
    })
}

fn task_fields_schema() -> Value {
    json!({
        "title": { "type": "string", "minLength": 1, "maxLength": 200 },
        "description": { "type": "string", "maxLength": 10000 },
        "start_date": { "type": "string", "format": "date" },
        "end_date": { "type": "string", "format": "date" },
        "importance": { "type": "string", "enum": ["low", "normal", "high", "urgent"] }
    })
}

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

// ── Personal Task Tools ──

pub fn create_task_schema() -> Value {
    let fields = task_fields_schema();
    json!({
        "type": "object",
        "properties": {
            "kind": { "type": "string", "enum": ["short", "long"] },
            "title": fields["title"],
            "description": fields["description"],
            "start_date": fields["start_date"],
            "end_date": fields["end_date"],
            "importance": fields["importance"]
        },
        "required": ["kind", "title", "start_date", "end_date", "importance"],
        "additionalProperties": false
    })
}

pub fn list_tasks_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "kinds": { "type": "array", "items": { "type": "string", "enum": ["short", "long"] } },
            "statuses": { "type": "array", "items": { "type": "string", "enum": ["open", "planned", "in_progress", "blocked", "completed", "cancelled"] } },
            "importance": { "type": "array", "items": { "type": "string", "enum": ["low", "normal", "high", "urgent"] } },
            "start_date": { "type": "string", "format": "date" },
            "end_date": { "type": "string", "format": "date" },
            "query": { "type": "string" },
            "include_archived": { "type": "boolean", "default": false },
            "include_subtasks": { "type": "boolean", "default": false },
            "sort": { "type": "string", "enum": ["priority", "start_date", "updated_at", "created_at", "importance"], "default": "priority" },
            "cursor": { "type": "string" },
            "limit": { "type": "integer", "minimum": 1, "maximum": 200, "default": 50 }
        },
        "additionalProperties": false
    })
}

pub fn get_task_schema() -> Value {
    json!({
        "type": "object",
        "properties": { "task_id": { "type": "string", "format": "uuid" } },
        "required": ["task_id"],
        "additionalProperties": false
    })
}

pub fn update_task_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "task_id": { "type": "string", "format": "uuid" },
            "patch": { "type": "object", "properties": task_fields_schema(), "minProperties": 1, "additionalProperties": false },
            "expected_version": expected_task_version_schema()
        },
        "required": ["task_id", "patch", "expected_version"],
        "additionalProperties": false
    })
}

pub fn set_task_status_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "task_id": { "type": "string", "format": "uuid" },
            "status": { "type": "string", "enum": ["open", "planned", "in_progress", "blocked", "completed", "cancelled"] },
            "closure_note": { "type": "string", "maxLength": 10000 },
            "cascade": { "type": "boolean", "default": false },
            "expected_version": expected_task_version_schema()
        },
        "required": ["task_id", "status", "expected_version"],
        "additionalProperties": false
    })
}

pub fn add_subtask_schema() -> Value {
    let fields = task_fields_schema();
    json!({
        "type": "object",
        "properties": {
            "parent_id": { "type": "string", "format": "uuid" },
            "title": fields["title"],
            "description": fields["description"],
            "start_date": fields["start_date"],
            "end_date": fields["end_date"],
            "importance": fields["importance"],
            "expected_version": expected_task_version_schema()
        },
        "required": ["parent_id", "title", "start_date", "end_date", "importance", "expected_version"],
        "additionalProperties": false
    })
}

pub fn move_subtask_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "task_id": { "type": "string", "format": "uuid" },
            "new_parent_id": { "type": "string", "format": "uuid" },
            "position": { "type": "integer", "minimum": 0 },
            "expected_version": expected_task_version_schema()
        },
        "required": ["task_id", "new_parent_id", "position", "expected_version"],
        "additionalProperties": false
    })
}

pub fn add_task_progress_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "task_id": { "type": "string", "format": "uuid" },
            "note": { "type": "string", "minLength": 1, "maxLength": 10000 },
            "percent_after": { "type": "integer", "minimum": 0, "maximum": 100 },
            "expected_version": expected_task_version_schema()
        },
        "required": ["task_id", "note", "expected_version"],
        "additionalProperties": false
    })
}

pub fn get_task_calendar_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "start_date": { "type": "string", "format": "date" },
            "end_date": { "type": "string", "format": "date" },
            "include_subtasks": { "type": "boolean", "default": false },
            "include_archived": { "type": "boolean", "default": false },
            "kinds": { "type": "array", "items": { "type": "string", "enum": ["short", "long"] } },
            "statuses": { "type": "array", "items": { "type": "string", "enum": ["open", "planned", "in_progress", "blocked", "completed", "cancelled"] } },
            "importance": { "type": "array", "items": { "type": "string", "enum": ["low", "normal", "high", "urgent"] } }
        },
        "required": ["start_date", "end_date"],
        "additionalProperties": false
    })
}

pub fn archive_task_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "task_id": { "type": "string", "format": "uuid" },
            "archived": { "type": "boolean" },
            "expected_version": expected_task_version_schema()
        },
        "required": ["task_id", "archived", "expected_version"],
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

/// Schema for `create_memo` — create a memo (Time Machine).
pub fn create_memo_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "content": {
                "type": "string",
                "description": "小记内容（支持 Markdown）"
            },
            "images": {
                "type": "array",
                "items": { "type": "string" },
                "description": "图片路径列表"
            },
            "tags": {
                "type": "array",
                "items": { "type": "string" },
                "description": "标签列表"
            }
        },
        "required": ["content"],
        "additionalProperties": false
    })
}

/// Schema for `browse_timeline` — browse the Time Machine timeline.
pub fn browse_timeline_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "start_date": {
                "type": "string",
                "description": "起始日期（YYYY-MM-DD）"
            },
            "end_date": {
                "type": "string",
                "description": "结束日期（YYYY-MM-DD）"
            },
            "limit": {
                "type": "integer",
                "default": 20,
                "minimum": 1,
                "maximum": 100,
                "description": "每页数量"
            },
            "offset": {
                "type": "integer",
                "default": 0,
                "minimum": 0,
                "description": "偏移量"
            }
        },
        "additionalProperties": false
    })
}

/// Schema for `search_memos` — search memos in the Time Machine.
pub fn search_memos_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "query": {
                "type": "string",
                "description": "搜索关键词"
            },
            "start_date": {
                "type": "string",
                "description": "起始日期（YYYY-MM-DD）"
            },
            "end_date": {
                "type": "string",
                "description": "结束日期（YYYY-MM-DD）"
            },
            "tags": {
                "type": "array",
                "items": { "type": "string" },
                "description": "标签筛选"
            },
            "limit": {
                "type": "integer",
                "default": 20,
                "minimum": 1,
                "maximum": 100,
                "description": "每页数量"
            }
        },
        "required": ["query"],
        "additionalProperties": false
    })
}

/// Schema for `sync_memos` — sync memos from Obsidian files.
pub fn sync_memos_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "months": {
                "type": "integer",
                "default": 3,
                "minimum": 1,
                "maximum": 24,
                "description": "同步最近几个月的数据"
            }
        },
        "additionalProperties": false
    })
}

/// Schema for `get_memo_stats` — get memo statistics.
pub fn get_memo_stats_schema() -> Value {
    json!({
        "type": "object",
        "properties": {},
        "additionalProperties": false
    })
}

/// Schema for `get_knowledge_insights` — get knowledge base insights.
pub fn get_knowledge_insights_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "force": {
                "type": "boolean",
                "default": false,
                "description": "强制重新统计（忽略缓存）"
            }
        },
        "additionalProperties": false
    })
}

// ── System Config Tools ──

/// Schema for `get_config`.
pub fn get_config_schema() -> Value {
    json!({
        "type": "object",
        "properties": {},
        "additionalProperties": false
    })
}

/// Schema for `save_config`.
pub fn save_config_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "vault": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Vault 根目录路径" },
                    "name": { "type": "string", "description": "Vault 名称" }
                }
            },
            "obsidian": {
                "type": "object",
                "properties": {
                    "enabled": { "type": "boolean" },
                    "url": { "type": "string" },
                    "api_key": { "type": "string" }
                }
            },
            "llm": {
                "type": "object",
                "properties": {
                    "provider": { "type": "string", "enum": ["openai", "claude", "ollama"] },
                    "model": { "type": "string" },
                    "max_tokens": { "type": "integer" },
                    "temperature": { "type": "number" }
                }
            }
        },
        "additionalProperties": false
    })
}

// ── Inspiration Tools ──

/// Schema for `get_inspiration` — generate creative inspiration.
pub fn get_inspiration_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "type": {
                "type": "string",
                "description": "灵感模式",
                "enum": ["concept_combo", "reverse_question", "counterpoint"],
                "default": "concept_combo"
            },
            "note_path": {
                "type": "string",
                "description": "目标笔记路径（vault 内相对路径）"
            }
        },
        "additionalProperties": false
    })
}

// ── Radar Tools ──

/// Schema for `get_radar` — get recommended articles.
pub fn get_radar_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "limit": {
                "type": "integer",
                "default": 10,
                "minimum": 1,
                "maximum": 50,
                "description": "返回结果数量"
            }
        },
        "additionalProperties": false
    })
}

/// Schema for `add_to_vault` — save article to vault.
pub fn add_to_vault_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "article_id": { "type": "string", "description": "雷达条目 ID" },
            "target_dir": { "type": "string", "description": "目标目录（默认 radar/）" }
        },
        "required": ["article_id"],
        "additionalProperties": false
    })
}

/// Schema for `dismiss_radar_item` — dismiss a radar item.
pub fn dismiss_radar_item_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "article_id": { "type": "string", "description": "雷达条目 ID" }
        },
        "required": ["article_id"],
        "additionalProperties": false
    })
}
