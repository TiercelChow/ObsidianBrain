use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::BrainError;
use crate::infra::book_wiki_store::{AgentCapabilityGrant, BookWikiStore};

pub const AGENT_KNOWLEDGE_TOOLS: &[&str] = &[
    "book_get_context",
    "book_list_sources",
    "book_search_sources",
    "book_read_source_span",
    "knowledge_search_entries",
    "knowledge_get_entry",
    "knowledge_get_neighbors",
    "knowledge_propose_changes",
    "knowledge_create_task",
    "knowledge_report_progress",
    "knowledge_get_review_result",
];

pub fn agent_knowledge_tool_schemas() -> Vec<Value> {
    vec![
        tool_schema(
            "book_get_context",
            "读取当前授权书籍、知识库状态和配置文档",
            object_schema(
                json!({
                    "knowledge_base_id": { "type": "string" }
                }),
                &["knowledge_base_id"],
            ),
        ),
        tool_schema(
            "book_list_sources",
            "列出当前授权书籍的 Markdown 来源及版本",
            object_schema(
                json!({
                    "knowledge_base_id": { "type": "string" }
                }),
                &["knowledge_base_id"],
            ),
        ),
        tool_schema(
            "book_search_sources",
            "全文检索当前授权书籍的 Markdown 来源片段",
            object_schema(
                json!({
                    "knowledge_base_id": { "type": "string" },
                    "query": { "type": "string", "minLength": 1, "maxLength": 2000 },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 50 }
                }),
                &["knowledge_base_id", "query"],
            ),
        ),
        tool_schema(
            "book_read_source_span",
            "读取当前授权书籍中的一个来源片段",
            object_schema(
                json!({
                    "knowledge_base_id": { "type": "string" },
                    "source_span_id": { "type": "string" }
                }),
                &["knowledge_base_id", "source_span_id"],
            ),
        ),
        tool_schema(
            "knowledge_search_entries",
            "检索当前授权知识库的实体和章节索引",
            object_schema(
                json!({
                    "knowledge_base_id": { "type": "string" },
                    "query": { "type": "string", "maxLength": 2000 },
                    "entry_type": { "type": "string" },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 50 }
                }),
                &["knowledge_base_id"],
            ),
        ),
        tool_schema(
            "knowledge_get_entry",
            "读取一个授权知识条目的正文、论断、关系、引用和版本",
            object_schema(
                json!({
                    "entry_id": { "type": "string" }
                }),
                &["entry_id"],
            ),
        ),
        tool_schema(
            "knowledge_get_neighbors",
            "读取一个授权知识条目的关系邻居",
            object_schema(
                json!({
                    "entry_id": { "type": "string" }
                }),
                &["entry_id"],
            ),
        ),
        tool_schema(
            "knowledge_propose_changes",
            "提交结构化知识候选供用户审核，不直接修改正式知识",
            object_schema(
                json!({
                    "knowledge_base_id": { "type": "string" },
                    "title": { "type": "string", "minLength": 1, "maxLength": 200 },
                    "reason": { "type": "string", "maxLength": 2000 },
                    "idempotency_key": { "type": "string", "minLength": 1, "maxLength": 200 },
                    "candidates": { "type": "array", "minItems": 1, "maxItems": 100, "items": { "type": "object" } }
                }),
                &[
                    "knowledge_base_id",
                    "title",
                    "idempotency_key",
                    "candidates",
                ],
            ),
        ),
        tool_schema(
            "knowledge_create_task",
            "在当前授权知识库中创建研究任务草稿",
            object_schema(
                json!({
                    "knowledge_base_id": { "type": "string" },
                    "title": { "type": "string", "minLength": 1, "maxLength": 200 },
                    "description": { "type": "string", "maxLength": 4000 },
                    "task_type": { "type": "string", "enum": ["research", "refresh", "review"] },
                    "deliverable_type": { "type": "string", "enum": ["report", "presentation"] }
                }),
                &["knowledge_base_id", "title", "task_type"],
            ),
        ),
        tool_schema(
            "knowledge_report_progress",
            "记录当前 Agent 运行阶段和进度",
            object_schema(
                json!({
                    "phase": { "type": "string", "minLength": 1, "maxLength": 100 },
                    "percent": { "type": "number", "minimum": 0, "maximum": 100 },
                    "message": { "type": "string", "maxLength": 500 }
                }),
                &["phase", "percent", "message"],
            ),
        ),
        tool_schema(
            "knowledge_get_review_result",
            "读取当前授权知识库中的变更集审核结果",
            object_schema(
                json!({
                    "change_set_id": { "type": "string" }
                }),
                &["change_set_id"],
            ),
        ),
    ]
}

pub fn call_agent_knowledge_tool(
    store: &BookWikiStore,
    token: &str,
    tool: &str,
    arguments: Value,
) -> Result<Value, BrainError> {
    if !AGENT_KNOWLEDGE_TOOLS.contains(&tool) {
        return Err(BrainError::KnowledgeValidation(format!(
            "未知的 Agent 知识工具: {tool}"
        )));
    }
    let grant = store.validate_agent_run_capability(token, tool)?;
    match tool {
        "book_get_context" => {
            let args: BaseArgs = parse_arguments(arguments)?;
            require_scope(&grant, &args.knowledge_base_id)?;
            Ok(json!({
                "knowledge_base": store.get_base(&args.knowledge_base_id)?,
                "config_documents": store.list_config_documents(Some(&args.knowledge_base_id))?,
            }))
        }
        "book_list_sources" => {
            let args: BaseArgs = parse_arguments(arguments)?;
            require_scope(&grant, &args.knowledge_base_id)?;
            Ok(json!({
                "sources": store.list_current_source_documents(&args.knowledge_base_id)?
            }))
        }
        "book_search_sources" => {
            let args: SearchSourcesArgs = parse_arguments(arguments)?;
            require_scope(&grant, &args.knowledge_base_id)?;
            validate_query(&args.query)?;
            Ok(json!({
                "spans": store.search_source_spans(
                    &args.knowledge_base_id,
                    &args.query,
                    args.limit.unwrap_or(12).clamp(1, 50),
                )?
            }))
        }
        "book_read_source_span" => {
            let args: ReadSourceSpanArgs = parse_arguments(arguments)?;
            require_scope(&grant, &args.knowledge_base_id)?;
            Ok(json!({
                "span": store.get_current_source_span(
                    &args.knowledge_base_id,
                    &args.source_span_id,
                )?
            }))
        }
        "knowledge_search_entries" => {
            let args: SearchEntriesArgs = parse_arguments(arguments)?;
            require_scope(&grant, &args.knowledge_base_id)?;
            if let Some(query) = args.query.as_deref() {
                validate_query(query)?;
            }
            Ok(json!({
                "entries": store.list_entries(
                    &args.knowledge_base_id,
                    args.query.as_deref(),
                    args.entry_type.as_deref(),
                    args.limit.unwrap_or(12).clamp(1, 50),
                )?
            }))
        }
        "knowledge_get_entry" => {
            let args: EntryArgs = parse_arguments(arguments)?;
            let entry = store.get_entry(&args.entry_id)?;
            require_scope(&grant, &entry.entry.knowledge_base_id)?;
            serde_json::to_value(entry)
                .map_err(|error| BrainError::Internal(format!("知识条目序列化失败: {error}")))
        }
        "knowledge_get_neighbors" => {
            let args: EntryArgs = parse_arguments(arguments)?;
            let entry = store.get_entry(&args.entry_id)?;
            require_scope(&grant, &entry.entry.knowledge_base_id)?;
            Ok(json!({
                "entry_id": entry.entry.id,
                "relations": entry.relations,
            }))
        }
        "knowledge_propose_changes" => {
            let args: ProposeChangesArgs = parse_arguments(arguments)?;
            require_scope(&grant, &args.knowledge_base_id)?;
            let change_set = store.create_semantic_change_set(
                &args.knowledge_base_id,
                &grant.run_id,
                &args.title,
                args.reason
                    .as_deref()
                    .unwrap_or("Agent 通过受控工具提交知识候选"),
                &format!("agent-tool:{}:{}", grant.run_id, args.idempotency_key),
                &args.candidates,
            )?;
            serde_json::to_value(change_set)
                .map_err(|error| BrainError::Internal(format!("变更集序列化失败: {error}")))
        }
        "knowledge_create_task" => {
            let args: CreateTaskArgs = parse_arguments(arguments)?;
            require_scope(&grant, &args.knowledge_base_id)?;
            let task = store.create_task_with_deliverable(
                &args.knowledge_base_id,
                &args.title,
                args.description.as_deref().unwrap_or(""),
                &args.task_type,
                args.deliverable_type.as_deref().unwrap_or("report"),
            )?;
            serde_json::to_value(task)
                .map_err(|error| BrainError::Internal(format!("研究任务序列化失败: {error}")))
        }
        "knowledge_report_progress" => {
            let args: ReportProgressArgs = parse_arguments(arguments)?;
            if !(0.0..=100.0).contains(&args.percent) {
                return Err(BrainError::KnowledgeValidation(
                    "Agent 进度必须在 0 到 100 之间".to_string(),
                ));
            }
            let event = store.append_agent_run_event(
                &grant.run_id,
                "run.progress",
                Some(args.phase.trim()),
                &args.message,
                &json!({ "percent": args.percent }),
            )?;
            serde_json::to_value(event)
                .map_err(|error| BrainError::Internal(format!("进度事件序列化失败: {error}")))
        }
        "knowledge_get_review_result" => {
            let args: ReviewArgs = parse_arguments(arguments)?;
            let change_set = store.get_change_set(&args.change_set_id)?;
            require_scope(&grant, &change_set.knowledge_base_id)?;
            serde_json::to_value(change_set)
                .map_err(|error| BrainError::Internal(format!("审核结果序列化失败: {error}")))
        }
        _ => unreachable!("agent tool allowlist and dispatcher must stay aligned"),
    }
}

fn tool_schema(name: &str, description: &str, input_schema: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema,
    })
}

fn object_schema(properties: Value, required: &[&str]) -> Value {
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false,
    })
}

fn parse_arguments<T: DeserializeOwned>(arguments: Value) -> Result<T, BrainError> {
    serde_json::from_value(arguments).map_err(|error| {
        BrainError::KnowledgeValidation(format!("Agent 工具参数校验失败: {error}"))
    })
}

fn require_scope(grant: &AgentCapabilityGrant, base_id: &str) -> Result<(), BrainError> {
    if grant
        .knowledge_base_ids
        .iter()
        .any(|allowed| allowed == base_id)
    {
        Ok(())
    } else {
        Err(BrainError::KnowledgeValidation(
            "Agent 工具不能访问未授权的知识库".to_string(),
        ))
    }
}

fn validate_query(query: &str) -> Result<(), BrainError> {
    let query = query.trim();
    if query.is_empty() || query.chars().count() > 2_000 {
        Err(BrainError::KnowledgeValidation(
            "Agent 检索词不能为空且不能超过 2000 个字符".to_string(),
        ))
    } else {
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BaseArgs {
    knowledge_base_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SearchSourcesArgs {
    knowledge_base_id: String,
    query: String,
    limit: Option<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadSourceSpanArgs {
    knowledge_base_id: String,
    source_span_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SearchEntriesArgs {
    knowledge_base_id: String,
    query: Option<String>,
    entry_type: Option<String>,
    limit: Option<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EntryArgs {
    entry_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProposeChangesArgs {
    knowledge_base_id: String,
    title: String,
    reason: Option<String>,
    idempotency_key: String,
    candidates: Vec<Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateTaskArgs {
    knowledge_base_id: String,
    title: String,
    description: Option<String>,
    task_type: String,
    deliverable_type: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReportProgressArgs {
    phase: String,
    percent: f64,
    message: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewArgs {
    change_set_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use crate::infra::book_wiki_store::{MarkdownSourceDraft, SourceSectionDraft};
    use crate::infra::sqlite_store::SqliteStore;
    use crate::models::book_wiki::{BookKind, ReaderBook};

    fn book(id: &str, path: &str) -> ReaderBook {
        ReaderBook {
            id: id.to_string(),
            path: path.to_string(),
            kind: BookKind::Folder,
            name: id.to_string(),
            description: String::new(),
            category: String::new(),
            added_at: 1,
            progress: None,
        }
    }

    fn source(id: &str, text: &str) -> MarkdownSourceDraft {
        MarkdownSourceDraft {
            id: format!("source-{id}"),
            version_id: format!("version-{id}"),
            original_path: format!("/tmp/{id}.md"),
            relative_path: format!("{id}.md"),
            title: id.to_string(),
            ordinal: 0,
            content_hash: format!("hash-{id}"),
            size_bytes: 10,
            modified_at: None,
            sections: vec![SourceSectionDraft {
                id: format!("span-{id}"),
                entry_id: format!("entry-{id}"),
                slug: id.to_string(),
                title: id.to_string(),
                summary: text.to_string(),
                content_md: text.to_string(),
                line_start: 1,
                line_end: 1,
                content_hash: format!("section-hash-{id}"),
            }],
        }
    }

    #[test]
    fn test_agent_tool_gateway_rejects_cross_book_and_unknown_arguments() {
        let dir = tempfile::tempdir().unwrap();
        let db = Arc::new(SqliteStore::new(&dir.path().join("tools.db")).unwrap());
        let store = BookWikiStore::new(db);
        store
            .save_reader_books(&[book("book-a", "/tmp/book-a"), book("book-b", "/tmp/book-b")])
            .unwrap();
        let base_a = store.initialize_base("book-a").unwrap();
        let base_b = store.initialize_base("book-b").unwrap();
        store
            .sync_markdown_sources(&base_a.id, &[source("a", "只属于 A 的内容")])
            .unwrap();
        store
            .sync_markdown_sources(&base_b.id, &[source("b", "只属于 B 的内容")])
            .unwrap();
        let run = store
            .start_agent_run(&base_a.id, "deepseek_harness", "knowledge_qa", &json!({}))
            .unwrap();
        let capability = store
            .issue_agent_run_capability(
                &run.id,
                std::slice::from_ref(&base_a.id),
                &["book_search_sources".to_string()],
                300,
            )
            .unwrap();

        let cross_book = call_agent_knowledge_tool(
            &store,
            &capability.token,
            "book_search_sources",
            json!({ "knowledge_base_id": base_b.id, "query": "内容" }),
        )
        .unwrap_err();
        assert!(cross_book.to_string().contains("未授权"));

        let unknown = call_agent_knowledge_tool(
            &store,
            &capability.token,
            "book_search_sources",
            json!({
                "knowledge_base_id": base_a.id,
                "query": "内容",
                "unexpected": true
            }),
        )
        .unwrap_err();
        assert!(unknown.to_string().contains("unknown field"));
    }
}
