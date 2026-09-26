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
    "knowledge_list_compiled_catalog",
    "knowledge_search_entries",
    "knowledge_get_entry",
    "knowledge_get_neighbors",
    "knowledge_propose_changes",
    "knowledge_create_task",
    "knowledge_report_progress",
    "knowledge_get_review_result",
];

pub const AGENT_EXTERNAL_RESEARCH_TOOL: &str = "book_fetch_external";

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
            "分页读取当前授权书籍中的一个来源片段；超长片段需按字符偏移继续读取",
            object_schema(
                json!({
                    "knowledge_base_id": { "type": "string" },
                    "source_span_id": { "type": "string" },
                    "offset_chars": { "type": "integer", "minimum": 0 },
                    "max_chars": { "type": "integer", "minimum": 1, "maximum": 4000 }
                }),
                &["knowledge_base_id", "source_span_id"],
            ),
        ),
        tool_schema(
            "knowledge_list_compiled_catalog",
            "分页浏览当前书籍的编译知识标题、别名和简短概述；用于跨主题语义筛选，不返回正文。offset 从 0 开始，结合 total 浏览后续页",
            object_schema(
                json!({
                    "knowledge_base_id": { "type": "string" },
                    "offset": { "type": "integer", "minimum": 0 },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 200 }
                }),
                &["knowledge_base_id"],
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
            "分页读取授权知识条目的正文、论断、关系与引用；结果 citation 给出本 Run 可引用的稳定 S 编号，has_more 为真可继续读取",
            object_schema(
                json!({
                    "entry_id": { "type": "string" },
                    "offset_chars": { "type": "integer", "minimum": 0 },
                    "max_chars": { "type": "integer", "minimum": 1, "maximum": 12000 }
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
        tool_schema(
            AGENT_EXTERNAL_RESEARCH_TOOL,
            "读取用户为当前研究任务明确授权的 HTTPS 域名中的一份文本资料；只读、限流且不跟随重定向",
            object_schema(
                json!({
                    "url": { "type": "string", "format": "uri", "minLength": 1, "maxLength": 2048 }
                }),
                &["url"],
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
    if tool == AGENT_EXTERNAL_RESEARCH_TOOL {
        return Err(BrainError::KnowledgeValidation(
            "外部研究工具必须通过异步 MCP 网关调用".to_string(),
        ));
    }
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
            let spans = store.search_source_spans(
                &args.knowledge_base_id,
                &args.query,
                args.limit.unwrap_or(12).clamp(1, 50),
            )?;
            Ok(json!({
                "spans": spans.into_iter().map(|span| json!({
                    "id": span.id,
                    "source_path": span.source_path,
                    "heading": span.heading,
                    "line_start": span.line_start,
                    "line_end": span.line_end,
                    "preview": span.content.chars().take(240).collect::<String>(),
                })).collect::<Vec<_>>()
            }))
        }
        "book_read_source_span" => {
            let args: ReadSourceSpanArgs = parse_arguments(arguments)?;
            require_scope(&grant, &args.knowledge_base_id)?;
            let span =
                store.get_current_source_span(&args.knowledge_base_id, &args.source_span_id)?;
            let total_chars = span.content.chars().count();
            let offset_chars = args.offset_chars.unwrap_or(0).min(total_chars);
            let max_chars = args.max_chars.unwrap_or(4_000).clamp(1, 4_000);
            let content = span
                .content
                .chars()
                .skip(offset_chars)
                .take(max_chars)
                .collect::<String>();
            Ok(json!({
                "span": {
                    "id": span.id,
                    "source_document_id": span.source_document_id,
                    "source_version_id": span.source_version_id,
                    "source_path": span.source_path,
                    "heading": span.heading,
                    "line_start": span.line_start,
                    "line_end": span.line_end,
                    "content": content,
                    "offset_chars": offset_chars,
                    "total_chars": total_chars,
                    "has_more": offset_chars.saturating_add(max_chars) < total_chars,
                }
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
        "knowledge_list_compiled_catalog" => {
            let args: CompiledCatalogArgs = parse_arguments(arguments)?;
            require_scope(&grant, &args.knowledge_base_id)?;
            let offset = args.offset.unwrap_or(0);
            let limit = args.limit.unwrap_or(100).clamp(1, 200);
            let (entries, total) =
                store.list_qa_catalog_page(&args.knowledge_base_id, offset, limit)?;
            Ok(json!({
                "entries": entries.into_iter().map(|entry| json!({
                    "id": entry.id,
                    "title": entry.title,
                    "aliases": entry.aliases.into_iter().take(4).collect::<Vec<_>>(),
                    "summary": entry.summary.chars().take(160).collect::<String>(),
                    "type": entry.entry_type,
                })).collect::<Vec<_>>(),
                "offset": offset,
                "total": total,
                "has_more": offset.saturating_add(limit) < total.max(0) as usize,
            }))
        }
        "knowledge_get_entry" => {
            let args: ReadEntryArgs = parse_arguments(arguments)?;
            let mut entry = store.get_entry(&args.entry_id)?;
            require_scope(&grant, &entry.entry.knowledge_base_id)?;
            let total_chars = entry.content_md.chars().count();
            let offset = args.offset_chars.unwrap_or(0).min(total_chars);
            let max_chars = args.max_chars.unwrap_or(12_000).clamp(1, 12_000);
            entry.content_md = entry
                .content_md
                .chars()
                .skip(offset)
                .take(max_chars)
                .collect();
            let counts = json!({"claims":entry.claims.len(),"relations":entry.relations.len(),"citations":entry.citations.len()});
            entry.claims.truncate(50);
            entry.relations.truncate(50);
            entry.citations.truncate(50);
            entry.versions.clear();
            let mut result = serde_json::to_value(entry)
                .map_err(|error| BrainError::Internal(format!("知识条目序列化失败: {error}")))?;
            result["offset_chars"] = json!(offset);
            result["total_chars"] = json!(total_chars);
            result["has_more"] = json!(offset.saturating_add(max_chars) < total_chars);
            result["metadata_counts"] = counts;
            Ok(result)
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
    offset_chars: Option<usize>,
    max_chars: Option<usize>,
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
struct CompiledCatalogArgs {
    knowledge_base_id: String,
    offset: Option<usize>,
    limit: Option<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EntryArgs {
    entry_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadEntryArgs {
    entry_id: String,
    offset_chars: Option<usize>,
    max_chars: Option<usize>,
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
                &[
                    "book_search_sources".to_string(),
                    "knowledge_list_compiled_catalog".to_string(),
                ],
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

        let source_search = call_agent_knowledge_tool(
            &store,
            &capability.token,
            "book_search_sources",
            json!({ "knowledge_base_id": base_a.id, "query": "内容" }),
        )
        .unwrap();
        assert!(source_search["spans"][0]["preview"].is_string());
        assert!(source_search["spans"][0].get("content").is_none());
        let catalog = call_agent_knowledge_tool(
            &store,
            &capability.token,
            "knowledge_list_compiled_catalog",
            json!({ "knowledge_base_id": base_a.id, "offset": 0, "limit": 20 }),
        )
        .unwrap();
        assert_eq!(catalog["total"], 0);
        assert_eq!(catalog["entries"].as_array().unwrap().len(), 0);
    }
}
