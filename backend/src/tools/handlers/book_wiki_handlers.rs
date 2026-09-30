use std::sync::Arc;

use async_trait::async_trait;
use chrono::{Duration, Local};
use serde_json::{json, Value};

use crate::error::BrainError;
use crate::infra::book_wiki_store::{
    KnowledgeEntryEditProposal, KnowledgeEntryMergeProposal, KnowledgeEntryRevision,
    KnowledgeEntrySplitPart, KnowledgeEntrySplitProposal,
};
use crate::infra::deepseek_harness::inspect_runtime_profiles;
use crate::models::book_wiki::{ResearchBrief, SaveModelProviderRequest};
use crate::tools::traits::ToolHandler;
use crate::AppContext;

pub struct ListBookKnowledgeBasesHandler;

#[async_trait]
impl ToolHandler for ListBookKnowledgeBasesHandler {
    fn name(&self) -> &str {
        "list_book_knowledge_bases"
    }

    fn description(&self) -> &str {
        "列出阅境轩书架及每本书对应的数据库知识库状态"
    }

    fn input_schema(&self) -> Value {
        empty_schema()
    }

    fn module(&self) -> &str {
        "book_wiki"
    }

    async fn handle(&self, _args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        Ok(json!({
            "items": ctx.book_wiki_service.store().list_book_cards()?
        }))
    }
}

pub struct InitializeBookKnowledgeBaseHandler;

#[async_trait]
impl ToolHandler for InitializeBookKnowledgeBaseHandler {
    fn name(&self) -> &str {
        "initialize_book_knowledge_base"
    }

    fn description(&self) -> &str {
        "为书架中的一本书建立独立知识库，并可立即扫描来源"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "book_id": { "type": "string" },
                "sync": { "type": "boolean", "default": true }
            },
            "required": ["book_id"],
            "additionalProperties": false
        })
    }

    fn module(&self) -> &str {
        "book_wiki"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let book_id = required_string(&args, "book_id")?.to_string();
        let should_sync = args.get("sync").and_then(Value::as_bool).unwrap_or(true);
        let service = ctx.book_wiki_service.clone();
        if should_sync {
            let result = tokio::task::spawn_blocking(move || service.initialize_and_sync(&book_id))
                .await
                .map_err(|error| {
                    BrainError::Internal(format!("知识库初始化任务失败: {error}"))
                })??;
            serde_json::to_value(result)
                .map_err(|error| BrainError::Internal(format!("结果序列化失败: {error}")))
        } else {
            Ok(json!({
                "knowledge_base": ctx.book_wiki_service.store().initialize_base(&book_id)?,
                "scanned_sources": 0,
                "indexed_entries": 0,
                "requires_harness": false,
                "message": "知识库已创建，尚未扫描来源"
            }))
        }
    }
}

pub struct SyncBookKnowledgeBaseHandler;

#[async_trait]
impl ToolHandler for SyncBookKnowledgeBaseHandler {
    fn name(&self) -> &str {
        "sync_book_knowledge_base"
    }

    fn description(&self) -> &str {
        "重新扫描一本书的来源，并刷新数据库中的章节实体"
    }

    fn input_schema(&self) -> Value {
        required_id_schema("knowledge_base_id")
    }

    fn module(&self) -> &str {
        "book_wiki"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let base_id = required_string(&args, "knowledge_base_id")?.to_string();
        let service = ctx.book_wiki_service.clone();
        let result = tokio::task::spawn_blocking(move || service.sync(&base_id))
            .await
            .map_err(|error| BrainError::Internal(format!("知识库同步任务失败: {error}")))??;
        serde_json::to_value(result)
            .map_err(|error| BrainError::Internal(format!("结果序列化失败: {error}")))
    }
}

pub struct GetBookKnowledgeBaseHandler;

pub struct SetBookKnowledgeBaseLifecycleHandler;

#[async_trait]
impl ToolHandler for SetBookKnowledgeBaseLifecycleHandler {
    fn name(&self) -> &str {
        "set_book_knowledge_base_lifecycle"
    }
    fn description(&self) -> &str {
        "暂停、恢复或归档一本书的知识库"
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "knowledge_base_id": { "type": "string" },
                "lifecycle": { "type": "string", "enum": ["active", "paused", "archived"] }
            },
            "required": ["knowledge_base_id", "lifecycle"],
            "additionalProperties": false
        })
    }
    fn module(&self) -> &str {
        "book_wiki"
    }
    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        serde_json::to_value(ctx.book_wiki_service.store().set_base_lifecycle(
            required_string(&args, "knowledge_base_id")?,
            required_string(&args, "lifecycle")?,
        )?)
        .map_err(|error| BrainError::Internal(format!("生命周期结果序列化失败: {error}")))
    }
}

pub struct DeleteBookKnowledgeBaseHandler;

#[async_trait]
impl ToolHandler for DeleteBookKnowledgeBaseHandler {
    fn name(&self) -> &str {
        "delete_book_knowledge_base"
    }
    fn description(&self) -> &str {
        "删除知识库生成数据；阅境轩原书和书架条目不会被删除"
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "knowledge_base_id": { "type": "string" },
                "confirmation": { "type": "string", "maxLength": 300 }
            },
            "required": ["knowledge_base_id", "confirmation"],
            "additionalProperties": false
        })
    }
    fn module(&self) -> &str {
        "book_wiki"
    }
    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        ctx.book_wiki_service.store().delete_base(
            required_string(&args, "knowledge_base_id")?,
            required_string(&args, "confirmation")?,
        )?;
        Ok(json!({ "deleted": true }))
    }
}

pub struct CompileBookKnowledgeBaseHandler;

#[async_trait]
impl ToolHandler for CompileBookKnowledgeBaseHandler {
    fn name(&self) -> &str {
        "compile_book_knowledge_base"
    }

    fn description(&self) -> &str {
        "将跨章节语义 Wiki 编译加入后台队列，并立即返回可轮询状态"
    }

    fn input_schema(&self) -> Value {
        required_id_schema("knowledge_base_id")
    }

    fn module(&self) -> &str {
        "book_wiki"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let knowledge_base = ctx
            .book_wiki_service
            .queue_semantic_compile(required_string(&args, "knowledge_base_id")?)?;
        Ok(json!({
            "knowledge_base": knowledge_base,
            "queued": true,
            "message": "智能编译任务已进入后台，可以离开当前页面"
        }))
    }
}

pub struct CancelBookKnowledgeCompileHandler;

pub struct RetryKnowledgeSourceReviewHandler;

#[async_trait]
impl ToolHandler for RetryKnowledgeSourceReviewHandler {
    fn name(&self) -> &str {
        "retry_knowledge_source_review"
    }
    fn description(&self) -> &str {
        "用户显式重新分析受影响主题的当前来源，不清除历史或假装解除过期状态"
    }
    fn input_schema(&self) -> Value {
        required_id_schema("knowledge_base_id")
    }
    fn module(&self) -> &str {
        "book_wiki"
    }
    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let base = ctx
            .book_wiki_service
            .queue_source_review_compile(required_string(&args, "knowledge_base_id")?)?;
        Ok(
            json!({"knowledge_base":base,"queued":true,"message":"来源复核已进入后台队列；真实变更仍需审核"}),
        )
    }
}

pub struct ProposeKnowledgeEntryArchiveHandler;

#[async_trait]
impl ToolHandler for ProposeKnowledgeEntryArchiveHandler {
    fn name(&self) -> &str {
        "propose_knowledge_entry_archive"
    }
    fn description(&self) -> &str {
        "人工提交仅改变状态的历史归档，保留旧正文和依据，审核后生效"
    }
    fn input_schema(&self) -> Value {
        json!({"type":"object","properties":{"entry_id":{"type":"string"},"expected_revision":{"type":"integer","minimum":1}},"required":["entry_id","expected_revision"],"additionalProperties":false})
    }
    fn module(&self) -> &str {
        "book_wiki"
    }
    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        serde_json::to_value(ctx.book_wiki_service.store().propose_entry_archive(
            required_string(&args, "entry_id")?,
            required_i64(&args, "expected_revision")?,
        )?)
        .map_err(|error| BrainError::Internal(format!("归档候选序列化失败: {error}")))
    }
}

pub struct GetKnowledgeCompileReportHandler;

#[async_trait]
impl ToolHandler for GetKnowledgeCompileReportHandler {
    fn name(&self) -> &str {
        "get_knowledge_compile_report"
    }
    fn description(&self) -> &str {
        "分页查看真实编译分析覆盖、主题归并及审核状态；不是事实完整性评分"
    }
    fn module(&self) -> &str {
        "book_wiki"
    }
    fn input_schema(&self) -> Value {
        json!({"type":"object","properties":{
            "knowledge_base_id":{"type":"string"},"report_id":{"type":"string"},
            "fragment_offset":{"type":"integer","minimum":0},"topic_offset":{"type":"integer","minimum":0},
            "limit":{"type":"integer","minimum":1,"maximum":100}
        },"required":["knowledge_base_id"],"additionalProperties":false})
    }
    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let offset = |key: &str, default: usize| -> Result<usize, BrainError> {
            args.get(key)
                .map(|value| {
                    value
                        .as_u64()
                        .and_then(|v| usize::try_from(v).ok())
                        .ok_or_else(|| {
                            BrainError::KnowledgeValidation(format!("{key} 必须是非负整数"))
                        })
                })
                .transpose()
                .map(|v| v.unwrap_or(default))
        };
        let report = ctx.book_wiki_service.store().get_compile_report(
            required_string(&args, "knowledge_base_id")?,
            args.get("report_id").and_then(Value::as_str),
            offset("fragment_offset", 0)?,
            offset("topic_offset", 0)?,
            offset("limit", 50)?,
        )?;
        Ok(json!({"report":report}))
    }
}

#[async_trait]
impl ToolHandler for CancelBookKnowledgeCompileHandler {
    fn name(&self) -> &str {
        "cancel_book_knowledge_compile"
    }

    fn description(&self) -> &str {
        "请求取消一本书当前正在执行的后台智能编译"
    }

    fn input_schema(&self) -> Value {
        required_id_schema("knowledge_base_id")
    }

    fn module(&self) -> &str {
        "book_wiki"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let knowledge_base = ctx
            .book_wiki_service
            .request_semantic_compile_cancel(required_string(&args, "knowledge_base_id")?)?;
        Ok(json!({
            "knowledge_base": knowledge_base,
            "cancel_requested": true
        }))
    }
}

pub struct ListKnowledgeChangeSetsHandler;

#[async_trait]
impl ToolHandler for ListKnowledgeChangeSetsHandler {
    fn name(&self) -> &str {
        "list_knowledge_change_sets"
    }

    fn description(&self) -> &str {
        "列出一本书的语义 Wiki 候选变更与审核状态"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "knowledge_base_id": { "type": "string" },
                "status": {
                    "type": "string",
                    "enum": ["proposed", "approved", "rejected", "applied", "conflicted"]
                }
            },
            "required": ["knowledge_base_id"],
            "additionalProperties": false
        })
    }

    fn module(&self) -> &str {
        "book_wiki"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        Ok(json!({
            "change_sets": ctx.book_wiki_service.store().list_change_sets(
                required_string(&args, "knowledge_base_id")?,
                args.get("status").and_then(Value::as_str),
            )?
        }))
    }
}

pub struct ResolveKnowledgeChangeSetHandler;

#[async_trait]
impl ToolHandler for ResolveKnowledgeChangeSetHandler {
    fn name(&self) -> &str {
        "resolve_knowledge_change_set"
    }

    fn description(&self) -> &str {
        "批准并原子应用语义知识变更，或驳回候选且保留审核记录"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "change_set_id": { "type": "string" },
                "decision": { "type": "string", "enum": ["approve", "reject"] },
                "note": { "type": "string", "maxLength": 2000 }
            },
            "required": ["change_set_id", "decision"],
            "additionalProperties": false
        })
    }

    fn module(&self) -> &str {
        "book_wiki"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let decision = required_string(&args, "decision")?;
        let change_set = ctx.book_wiki_service.store().resolve_change_set(
            required_string(&args, "change_set_id")?,
            decision == "approve",
            args.get("note").and_then(Value::as_str).unwrap_or(""),
        )?;
        serde_json::to_value(change_set)
            .map_err(|error| BrainError::Internal(format!("审核结果序列化失败: {error}")))
    }
}

#[async_trait]
impl ToolHandler for GetBookKnowledgeBaseHandler {
    fn name(&self) -> &str {
        "get_book_knowledge_base"
    }

    fn description(&self) -> &str {
        "获取单个书籍知识库的状态与统计"
    }

    fn input_schema(&self) -> Value {
        required_id_schema("knowledge_base_id")
    }

    fn module(&self) -> &str {
        "book_wiki"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let base_id = required_string(&args, "knowledge_base_id")?;
        serde_json::to_value(ctx.book_wiki_service.store().get_base(base_id)?)
            .map_err(|error| BrainError::Internal(format!("结果序列化失败: {error}")))
    }
}

pub struct ListKnowledgeEntriesHandler;

#[async_trait]
impl ToolHandler for ListKnowledgeEntriesHandler {
    fn name(&self) -> &str {
        "list_knowledge_entries"
    }

    fn description(&self) -> &str {
        "浏览或检索一本书知识库中的实体与来源章节"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "knowledge_base_id": { "type": "string" },
                "query": { "type": "string" },
                "entry_type": { "type": "string" },
                "include_stale": { "type": "boolean", "default": false, "description": "工作台复核时包含过期知识；不用于问答证据召回" },
                "offset": { "type": "integer", "minimum": 0, "maximum": 100000, "default": 0 },
                "limit": { "type": "integer", "minimum": 1, "maximum": 100, "default": 60 }
            },
            "required": ["knowledge_base_id"],
            "additionalProperties": false
        })
    }

    fn module(&self) -> &str {
        "book_wiki"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let base_id = required_string(&args, "knowledge_base_id")?;
        let query = args.get("query").and_then(Value::as_str);
        let entry_type = args.get("entry_type").and_then(Value::as_str);
        let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(60) as usize;
        let offset = args.get("offset").and_then(Value::as_u64).unwrap_or(0) as usize;
        serde_json::to_value(
            ctx.book_wiki_service.store().list_entries_page_with_stale(
                base_id,
                query,
                entry_type,
                offset,
                limit,
                args.get("include_stale")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            )?,
        )
        .map_err(|error| BrainError::Internal(format!("实体分页结果序列化失败: {error}")))
    }
}

pub struct GetKnowledgeEntryHandler;

pub struct ProposeKnowledgeEntryEditHandler;

pub struct ProposeKnowledgeEntryMergeHandler;

pub struct ProposeKnowledgeEntrySplitHandler;

pub struct ProposeReaderSelectionHandler;

#[async_trait]
impl ToolHandler for ProposeReaderSelectionHandler {
    fn name(&self) -> &str {
        "propose_reader_selection"
    }

    fn description(&self) -> &str {
        "把阅境轩 Markdown 原文中的选区提交为带当前来源引用的待审核知识候选"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "knowledge_base_id": { "type": "string" },
                "source_path": { "type": "string", "maxLength": 2000 },
                "selection": { "type": "string", "maxLength": 5000 },
                "title": { "type": "string", "maxLength": 1000 }
            },
            "required": ["knowledge_base_id", "source_path", "selection"],
            "additionalProperties": false
        })
    }

    fn module(&self) -> &str {
        "book_wiki"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        serde_json::to_value(ctx.book_wiki_service.store().propose_reader_selection(
            required_string(&args, "knowledge_base_id")?,
            required_string(&args, "source_path")?,
            required_string(&args, "selection")?,
            args.get("title").and_then(Value::as_str),
        )?)
        .map_err(|error| BrainError::Internal(format!("阅境轩知识候选序列化失败: {error}")))
    }
}

#[async_trait]
impl ToolHandler for ProposeKnowledgeEntryEditHandler {
    fn name(&self) -> &str {
        "propose_knowledge_entry_edit"
    }
    fn description(&self) -> &str {
        "把人工实体编辑保存为高风险待审核变更，不直接覆盖正式知识"
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "entry_id": { "type": "string" },
                "title": { "type": "string", "maxLength": 1000 },
                "summary": { "type": "string", "maxLength": 1000 },
                "content_md": { "type": "string", "maxLength": 30000 },
                "aliases": { "type": "array", "maxItems": 30, "items": { "type": "string", "maxLength": 100 } },
                "status": { "type": "string", "enum": ["draft", "verified", "archived"] },
                "expected_revision": { "type": "integer", "minimum": 1 }
            },
            "required": ["entry_id", "title", "summary", "content_md", "aliases", "status", "expected_revision"],
            "additionalProperties": false
        })
    }
    fn module(&self) -> &str {
        "book_wiki"
    }
    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let aliases = args
            .get("aliases")
            .and_then(Value::as_array)
            .ok_or_else(|| BrainError::KnowledgeValidation("aliases 必须是字符串数组".to_string()))?
            .iter()
            .map(|value| {
                value.as_str().map(str::to_string).ok_or_else(|| {
                    BrainError::KnowledgeValidation("aliases 必须是字符串数组".to_string())
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let proposal = KnowledgeEntryEditProposal {
            title: required_string(&args, "title")?,
            summary: required_string(&args, "summary")?,
            content_md: required_string(&args, "content_md")?,
            aliases: &aliases,
            status: required_string(&args, "status")?,
            expected_revision: args
                .get("expected_revision")
                .and_then(Value::as_i64)
                .ok_or_else(|| {
                    BrainError::KnowledgeValidation("expected_revision 必须是正整数".to_string())
                })?,
        };
        serde_json::to_value(
            ctx.book_wiki_service
                .store()
                .propose_entry_edit(required_string(&args, "entry_id")?, &proposal)?,
        )
        .map_err(|error| BrainError::Internal(format!("实体变更序列化失败: {error}")))
    }
}

#[async_trait]
impl ToolHandler for ProposeKnowledgeEntryMergeHandler {
    fn name(&self) -> &str {
        "propose_knowledge_entry_merge"
    }
    fn description(&self) -> &str {
        "把多个同书实体合并为高风险待审核变更，批准后归档来源并迁移关系"
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "target_entry_id": { "type": "string" },
                "title": { "type": "string", "maxLength": 1000 },
                "summary": { "type": "string", "maxLength": 1000 },
                "content_md": { "type": "string", "maxLength": 30000 },
                "aliases": { "type": "array", "maxItems": 30, "items": { "type": "string", "maxLength": 100 } },
                "status": { "type": "string", "enum": ["draft", "verified"] },
                "expected_revision": { "type": "integer", "minimum": 1 },
                "sources": {
                    "type": "array", "minItems": 1, "maxItems": 20,
                    "items": {
                        "type": "object",
                        "properties": {
                            "entry_id": { "type": "string" },
                            "expected_revision": { "type": "integer", "minimum": 1 }
                        },
                        "required": ["entry_id", "expected_revision"],
                        "additionalProperties": false
                    }
                }
            },
            "required": ["target_entry_id", "title", "summary", "content_md", "aliases", "status", "expected_revision", "sources"],
            "additionalProperties": false
        })
    }
    fn module(&self) -> &str {
        "book_wiki"
    }
    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let aliases = string_array(&args, "aliases")?;
        let sources = args
            .get("sources")
            .and_then(Value::as_array)
            .ok_or_else(|| BrainError::KnowledgeValidation("sources 必须是数组".to_string()))?
            .iter()
            .map(|source| {
                Ok(KnowledgeEntryRevision {
                    entry_id: required_string(source, "entry_id")?.to_string(),
                    expected_revision: required_i64(source, "expected_revision")?,
                })
            })
            .collect::<Result<Vec<_>, BrainError>>()?;
        let proposal = KnowledgeEntryMergeProposal {
            title: required_string(&args, "title")?,
            summary: required_string(&args, "summary")?,
            content_md: required_string(&args, "content_md")?,
            aliases: &aliases,
            status: required_string(&args, "status")?,
            expected_revision: required_i64(&args, "expected_revision")?,
            sources: &sources,
        };
        serde_json::to_value(
            ctx.book_wiki_service
                .store()
                .propose_entry_merge(required_string(&args, "target_entry_id")?, &proposal)?,
        )
        .map_err(|error| BrainError::Internal(format!("实体合并候选序列化失败: {error}")))
    }
}

#[async_trait]
impl ToolHandler for ProposeKnowledgeEntrySplitHandler {
    fn name(&self) -> &str {
        "propose_knowledge_entry_split"
    }
    fn description(&self) -> &str {
        "把一个实体拆成多个继承原始引用的新实体，批准前不修改正式知识"
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "entry_id": { "type": "string" },
                "expected_revision": { "type": "integer", "minimum": 1 },
                "parts": {
                    "type": "array", "minItems": 2, "maxItems": 12,
                    "items": {
                        "type": "object",
                        "properties": {
                            "title": { "type": "string", "maxLength": 1000 },
                            "summary": { "type": "string", "maxLength": 1000 },
                            "content_md": { "type": "string", "maxLength": 30000 },
                            "aliases": { "type": "array", "maxItems": 30, "items": { "type": "string", "maxLength": 100 } }
                        },
                        "required": ["title", "summary", "content_md", "aliases"],
                        "additionalProperties": false
                    }
                }
            },
            "required": ["entry_id", "expected_revision", "parts"],
            "additionalProperties": false
        })
    }
    fn module(&self) -> &str {
        "book_wiki"
    }
    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let parts = args
            .get("parts")
            .and_then(Value::as_array)
            .ok_or_else(|| BrainError::KnowledgeValidation("parts 必须是数组".to_string()))?
            .iter()
            .map(|part| {
                Ok(KnowledgeEntrySplitPart {
                    title: required_string(part, "title")?.to_string(),
                    summary: required_string(part, "summary")?.to_string(),
                    content_md: required_string(part, "content_md")?.to_string(),
                    aliases: string_array(part, "aliases")?,
                })
            })
            .collect::<Result<Vec<_>, BrainError>>()?;
        let proposal = KnowledgeEntrySplitProposal {
            expected_revision: required_i64(&args, "expected_revision")?,
            parts: &parts,
        };
        serde_json::to_value(
            ctx.book_wiki_service
                .store()
                .propose_entry_split(required_string(&args, "entry_id")?, &proposal)?,
        )
        .map_err(|error| BrainError::Internal(format!("实体拆分候选序列化失败: {error}")))
    }
}

pub struct GetKnowledgeGraphOverviewHandler;

#[async_trait]
impl ToolHandler for GetKnowledgeGraphOverviewHandler {
    fn name(&self) -> &str {
        "get_knowledge_graph_overview"
    }
    fn description(&self) -> &str {
        "查看一本书的关系数量、孤立实体和高连接枢纽"
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "knowledge_base_id": { "type": "string" },
                "limit": { "type": "integer", "minimum": 1, "maximum": 100, "default": 20 }
            },
            "required": ["knowledge_base_id"],
            "additionalProperties": false
        })
    }
    fn module(&self) -> &str {
        "book_wiki"
    }
    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        serde_json::to_value(ctx.book_wiki_service.store().get_graph_overview(
            required_string(&args, "knowledge_base_id")?,
            args.get("limit").and_then(Value::as_u64).unwrap_or(20) as usize,
        )?)
        .map_err(|error| BrainError::Internal(format!("关系洞察序列化失败: {error}")))
    }
}

pub struct FindKnowledgeGraphPathHandler;

pub struct GetKnowledgeGraphSnapshotHandler;

#[async_trait]
impl ToolHandler for GetKnowledgeGraphSnapshotHandler {
    fn name(&self) -> &str {
        "get_knowledge_graph_snapshot"
    }
    fn description(&self) -> &str {
        "按连接度读取桌面知识图谱画布所需的有限节点和关系快照"
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "knowledge_base_id": { "type": "string" },
                "limit": { "type": "integer", "minimum": 10, "maximum": 300, "default": 120 }
            },
            "required": ["knowledge_base_id"],
            "additionalProperties": false
        })
    }
    fn module(&self) -> &str {
        "book_wiki"
    }
    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        serde_json::to_value(ctx.book_wiki_service.store().get_graph_snapshot(
            required_string(&args, "knowledge_base_id")?,
            args.get("limit").and_then(Value::as_u64).unwrap_or(120) as usize,
        )?)
        .map_err(|error| BrainError::Internal(format!("关系图谱快照序列化失败: {error}")))
    }
}

#[async_trait]
impl ToolHandler for FindKnowledgeGraphPathHandler {
    fn name(&self) -> &str {
        "find_knowledge_graph_path"
    }
    fn description(&self) -> &str {
        "查找同一本书两个知识实体之间的最短关系路径"
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "knowledge_base_id": { "type": "string" },
                "from_entry_id": { "type": "string" },
                "to_entry_id": { "type": "string" },
                "max_depth": { "type": "integer", "minimum": 1, "maximum": 8, "default": 5 }
            },
            "required": ["knowledge_base_id", "from_entry_id", "to_entry_id"],
            "additionalProperties": false
        })
    }
    fn module(&self) -> &str {
        "book_wiki"
    }
    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        serde_json::to_value(ctx.book_wiki_service.store().find_graph_path(
            required_string(&args, "knowledge_base_id")?,
            required_string(&args, "from_entry_id")?,
            required_string(&args, "to_entry_id")?,
            args.get("max_depth").and_then(Value::as_u64).unwrap_or(5) as usize,
        )?)
        .map_err(|error| BrainError::Internal(format!("关系路径序列化失败: {error}")))
    }
}

pub struct LintBookKnowledgeBaseHandler;

#[async_trait]
impl ToolHandler for LintBookKnowledgeBaseHandler {
    fn name(&self) -> &str {
        "lint_book_knowledge_base"
    }

    fn description(&self) -> &str {
        "检查一本书的来源覆盖、主题引用、论断证据和重复主题"
    }

    fn input_schema(&self) -> Value {
        required_id_schema("knowledge_base_id")
    }

    fn module(&self) -> &str {
        "book_wiki"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        serde_json::to_value(
            ctx.book_wiki_service
                .store()
                .lint_knowledge_base(required_string(&args, "knowledge_base_id")?)?,
        )
        .map_err(|error| BrainError::Internal(format!("知识体检结果序列化失败: {error}")))
    }
}

#[async_trait]
impl ToolHandler for GetKnowledgeEntryHandler {
    fn name(&self) -> &str {
        "get_knowledge_entry"
    }

    fn description(&self) -> &str {
        "读取一个知识实体的 Markdown 内容与来源引用"
    }

    fn input_schema(&self) -> Value {
        required_id_schema("entry_id")
    }

    fn module(&self) -> &str {
        "book_wiki"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let entry_id = required_string(&args, "entry_id")?;
        serde_json::to_value(ctx.book_wiki_service.store().get_entry(entry_id)?)
            .map_err(|error| BrainError::Internal(format!("结果序列化失败: {error}")))
    }
}

pub struct AskBookKnowledgeHandler;

pub struct SaveKnowledgeAnswerHandler;

#[async_trait]
impl ToolHandler for SaveKnowledgeAnswerHandler {
    fn name(&self) -> &str {
        "save_knowledge_answer"
    }

    fn description(&self) -> &str {
        "把已完成且有当前来源引用的问答结论保存为待审核 Wiki 变更"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "knowledge_base_id": { "type": "string" },
                "run_id": { "type": "string" }
            },
            "required": ["knowledge_base_id", "run_id"],
            "additionalProperties": false
        })
    }

    fn module(&self) -> &str {
        "book_wiki"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        serde_json::to_value(ctx.book_wiki_service.save_answer_to_wiki(
            required_string(&args, "knowledge_base_id")?,
            required_string(&args, "run_id")?,
        )?)
        .map_err(|error| BrainError::Internal(format!("问答变更序列化失败: {error}")))
    }
}

pub struct ListKnowledgeConversationsHandler;

#[async_trait]
impl ToolHandler for ListKnowledgeConversationsHandler {
    fn name(&self) -> &str {
        "list_knowledge_conversations"
    }

    fn description(&self) -> &str {
        "列出一本书知识库中持久化的问答会话"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "knowledge_base_id": { "type": "string" },
                "limit": { "type": "integer", "minimum": 1, "maximum": 100, "default": 30 }
            },
            "required": ["knowledge_base_id"],
            "additionalProperties": false
        })
    }

    fn module(&self) -> &str {
        "book_wiki"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        Ok(json!({
            "conversations": ctx.book_wiki_service.store().list_conversations(
                required_string(&args, "knowledge_base_id")?,
                args.get("limit").and_then(Value::as_u64).unwrap_or(30) as usize,
            )?
        }))
    }
}

pub struct GetKnowledgeConversationHandler;

pub struct ListUnfinishedQaRunsHandler;

#[async_trait]
impl ToolHandler for ListUnfinishedQaRunsHandler {
    fn name(&self) -> &str {
        "list_unfinished_qa_runs"
    }

    fn description(&self) -> &str {
        "列出一本书中未完成且尚未被成功恢复的问答运行；不会将草稿当作正式会话"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "knowledge_base_id": { "type": "string" },
                "limit": { "type": "integer", "minimum": 1, "maximum": 30, "default": 10 }
            },
            "required": ["knowledge_base_id"],
            "additionalProperties": false
        })
    }

    fn module(&self) -> &str {
        "book_wiki"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        Ok(json!({
            "runs": ctx.book_wiki_service.store().list_unfinished_qa_runs(
                required_string(&args, "knowledge_base_id")?,
                args.get("limit").and_then(Value::as_u64).unwrap_or(10) as usize,
            )?
        }))
    }
}

#[async_trait]
impl ToolHandler for GetKnowledgeConversationHandler {
    fn name(&self) -> &str {
        "get_knowledge_conversation"
    }

    fn description(&self) -> &str {
        "读取问答会话的历史消息与来源引用"
    }

    fn input_schema(&self) -> Value {
        required_id_schema("conversation_id")
    }

    fn module(&self) -> &str {
        "book_wiki"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        serde_json::to_value(
            ctx.book_wiki_service
                .store()
                .get_conversation(required_string(&args, "conversation_id")?)?,
        )
        .map_err(|error| BrainError::Internal(format!("结果序列化失败: {error}")))
    }
}

#[async_trait]
impl ToolHandler for AskBookKnowledgeHandler {
    fn name(&self) -> &str {
        "ask_book_knowledge"
    }

    fn description(&self) -> &str {
        "检索当前书籍的数据库证据，并通过 DeepSeek Harness ACP 生成带来源编号的回答"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "knowledge_base_id": { "type": "string" },
                "question": { "type": "string", "minLength": 1, "maxLength": 2000 },
                "conversation_id": { "type": "string" }
            },
            "required": ["knowledge_base_id", "question"],
            "additionalProperties": false
        })
    }

    fn module(&self) -> &str {
        "book_wiki"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let result = ctx
            .book_wiki_service
            .ask(
                required_string(&args, "knowledge_base_id")?,
                required_string(&args, "question")?,
                args.get("conversation_id").and_then(Value::as_str),
            )
            .await?;
        serde_json::to_value(result)
            .map_err(|error| BrainError::Internal(format!("结果序列化失败: {error}")))
    }
}

pub struct ListKnowledgeTasksHandler;

#[async_trait]
impl ToolHandler for ListKnowledgeTasksHandler {
    fn name(&self) -> &str {
        "list_knowledge_tasks"
    }

    fn description(&self) -> &str {
        "列出书籍知识库的研究、刷新与审核任务"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "knowledge_base_id": { "type": "string" }
            },
            "additionalProperties": false
        })
    }

    fn module(&self) -> &str {
        "book_wiki"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let base_id = args.get("knowledge_base_id").and_then(Value::as_str);
        Ok(json!({
            "tasks": ctx.book_wiki_service.store().list_tasks(base_id)?
        }))
    }
}

pub struct CreateKnowledgeTaskHandler;

pub struct PreviewKnowledgeTaskBriefHandler;

#[async_trait]
impl ToolHandler for PreviewKnowledgeTaskBriefHandler {
    fn name(&self) -> &str {
        "preview_knowledge_task_brief"
    }

    fn description(&self) -> &str {
        "分析研究诉求并提出需要用户确认的材料决策，不启动研究"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type":"object",
            "properties":{
                "knowledge_base_id":{"type":"string"},
                "title":{"type":"string","minLength":1,"maxLength":200},
                "description":{"type":"string","maxLength":4000,"default":""},
                "task_type":{"type":"string","enum":["research","refresh","review"]},
                "deliverable_type":{"type":"string","enum":["report","presentation"]}
            },
            "required":["knowledge_base_id","title","task_type","deliverable_type"],
            "additionalProperties":false
        })
    }

    fn module(&self) -> &str {
        "book_wiki"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let preview = ctx
            .book_wiki_service
            .preview_research_brief(
                required_string(&args, "knowledge_base_id")?,
                required_string(&args, "title")?,
                args.get("description")
                    .and_then(Value::as_str)
                    .unwrap_or(""),
                required_string(&args, "task_type")?,
                required_string(&args, "deliverable_type")?,
            )
            .await?;
        serde_json::to_value(preview)
            .map_err(|error| BrainError::Internal(format!("研究预分析序列化失败: {error}")))
    }
}

#[async_trait]
impl ToolHandler for CreateKnowledgeTaskHandler {
    fn name(&self) -> &str {
        "create_knowledge_task"
    }

    fn description(&self) -> &str {
        "创建一项书籍研究任务，等待用户确认后通过 Harness 执行"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "knowledge_base_id": { "type": "string" },
                "title": { "type": "string", "minLength": 1, "maxLength": 200 },
                "description": { "type": "string", "maxLength": 4000, "default": "" },
                "task_type": {
                    "type": "string",
                    "enum": ["research", "refresh", "review"],
                    "default": "research"
                },
                "deliverable_type": {
                    "type": "string",
                    "enum": ["report", "presentation"],
                    "default": "report"
                },
                "external_research_enabled": {
                    "type": "boolean",
                    "default": false
                },
                "external_domains": {
                    "type": "array",
                    "maxItems": 20,
                    "items": { "type": "string", "minLength": 3, "maxLength": 253 },
                    "default": []
                },
                "external_request_limit": {
                    "type": "integer",
                    "minimum": 0,
                    "maximum": 50,
                    "default": 0
                },
                "brief": {
                    "type": "object",
                    "properties": {
                        "confirmed": {"type":"boolean"},
                        "audience": {"type":"string","enum":["general","specialist","beginner","self"]},
                        "purpose": {"type":"string","enum":["understand","decision","teach","reference"]},
                        "tone": {"type":"string","enum":["analytical","technical","narrative","concise"]},
                        "depth": {"type":"string","enum":["brief","standard","deep"]},
                        "presentation_theme": {"type":"string","enum":["editorial","midnight","sage"]},
                        "emphasis": {"type":"string","maxLength":500}
                    },
                    "required": ["confirmed","audience","purpose","tone","depth","presentation_theme","emphasis"],
                    "additionalProperties": false
                }
            },
            "required": ["knowledge_base_id", "title"],
            "additionalProperties": false
        })
    }

    fn module(&self) -> &str {
        "book_wiki"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let external_research_enabled = args
            .get("external_research_enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let external_domains = match args.get("external_domains") {
            Some(_) => string_array(&args, "external_domains")?,
            None => Vec::new(),
        };
        let brief = args
            .get("brief")
            .map(|value| {
                serde_json::from_value::<ResearchBrief>(value.clone()).map_err(|error| {
                    BrainError::KnowledgeValidation(format!("研究简报格式无效: {error}"))
                })
            })
            .transpose()?
            .unwrap_or_default();
        let task = ctx.book_wiki_service.store().create_task_with_brief(
            required_string(&args, "knowledge_base_id")?,
            required_string(&args, "title")?,
            args.get("description")
                .and_then(Value::as_str)
                .unwrap_or(""),
            args.get("task_type")
                .and_then(Value::as_str)
                .unwrap_or("research"),
            args.get("deliverable_type")
                .and_then(Value::as_str)
                .unwrap_or("report"),
            external_research_enabled,
            &external_domains,
            args.get("external_request_limit")
                .and_then(Value::as_i64)
                .unwrap_or(0),
            &brief,
        )?;
        serde_json::to_value(task)
            .map_err(|error| BrainError::Internal(format!("结果序列化失败: {error}")))
    }
}

pub struct GetKnowledgeTaskResultHandler;

#[async_trait]
impl ToolHandler for GetKnowledgeTaskResultHandler {
    fn name(&self) -> &str {
        "get_knowledge_task_result"
    }

    fn description(&self) -> &str {
        "读取已完成研究任务的报告、运行记录与当前仍可访问的来源证据"
    }

    fn input_schema(&self) -> Value {
        required_id_schema("task_id")
    }

    fn module(&self) -> &str {
        "book_wiki"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        serde_json::to_value(
            ctx.book_wiki_service
                .get_task_result(required_string(&args, "task_id")?)?,
        )
        .map_err(|error| BrainError::Internal(format!("结果序列化失败: {error}")))
    }
}

pub struct ExecuteKnowledgeTaskHandler;

pub struct GetKnowledgeResearchWorkspaceHandler;
pub struct GetKnowledgeResearchStageHandler;

#[async_trait]
impl ToolHandler for GetKnowledgeResearchWorkspaceHandler {
    fn name(&self) -> &str {
        "get_knowledge_research_workspace"
    }
    fn description(&self) -> &str {
        "读取持久研究目标与阶段元数据，不加载全部章节正文；旧任务返回 null，不补造历史"
    }
    fn input_schema(&self) -> Value {
        required_id_schema("task_id")
    }
    fn module(&self) -> &str {
        "book_wiki"
    }
    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        serde_json::to_value(
            ctx.book_wiki_service
                .store()
                .get_research_workspace(required_string(&args, "task_id")?)?,
        )
        .map_err(|error| BrainError::Internal(format!("研究工作区序列化失败: {error}")))
    }
}

#[async_trait]
impl ToolHandler for GetKnowledgeResearchStageHandler {
    fn name(&self) -> &str {
        "get_knowledge_research_stage"
    }
    fn description(&self) -> &str {
        "按需读取一个研究阶段的完整成果、证据矩阵与版本；不给未完成阶段伪造结果"
    }
    fn input_schema(&self) -> Value {
        json!({"type":"object","properties":{"task_id":{"type":"string","minLength":1},"stage_key":{"type":"string","minLength":1,"maxLength":128},"revision":{"type":"integer","minimum":1}},"required":["task_id","stage_key"],"additionalProperties":false})
    }
    fn module(&self) -> &str {
        "book_wiki"
    }
    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        serde_json::to_value(ctx.book_wiki_service.store().get_research_stage_content(
            required_string(&args, "task_id")?,
            required_string(&args, "stage_key")?,
            args.get("revision").and_then(Value::as_i64),
        )?)
        .map_err(|error| BrainError::Internal(format!("研究阶段序列化失败: {error}")))
    }
}

pub struct CancelKnowledgeTaskHandler;

#[async_trait]
impl ToolHandler for CancelKnowledgeTaskHandler {
    fn name(&self) -> &str {
        "cancel_knowledge_task"
    }

    fn description(&self) -> &str {
        "取消尚未结束的研究任务；运行中的 DeepSeek Harness ACP 会话会立即收到中断请求"
    }

    fn input_schema(&self) -> Value {
        required_id_schema("task_id")
    }

    fn module(&self) -> &str {
        "book_wiki"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        serde_json::to_value(
            ctx.book_wiki_service
                .request_task_cancel(required_string(&args, "task_id")?)?,
        )
        .map_err(|error| BrainError::Internal(format!("取消结果序列化失败: {error}")))
    }
}

#[async_trait]
impl ToolHandler for ExecuteKnowledgeTaskHandler {
    fn name(&self) -> &str {
        "execute_knowledge_task"
    }

    fn description(&self) -> &str {
        "将研究任务加入持久化后台队列，立即返回当前状态"
    }

    fn input_schema(&self) -> Value {
        required_id_schema("task_id")
    }

    fn module(&self) -> &str {
        "book_wiki"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let result = ctx
            .book_wiki_service
            .queue_task(required_string(&args, "task_id")?)?;
        serde_json::to_value(result)
            .map_err(|error| BrainError::Internal(format!("排队结果序列化失败: {error}")))
    }
}

pub struct GetBookWikiSettingsHandler;

pub struct ListKnowledgeBackupsHandler;

#[async_trait]
impl ToolHandler for ListKnowledgeBackupsHandler {
    fn name(&self) -> &str {
        "list_knowledge_backups"
    }

    fn description(&self) -> &str {
        "列出 SQLite Online Backup 创建的一致性数据库快照"
    }

    fn input_schema(&self) -> Value {
        empty_schema()
    }

    fn module(&self) -> &str {
        "book_wiki"
    }

    async fn handle(&self, _args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        Ok(json!({
            "backups": ctx.db.list_managed_backups()?,
            "retention": ctx.db.backup_retention()
        }))
    }
}

pub struct CreateKnowledgeBackupHandler;

#[async_trait]
impl ToolHandler for CreateKnowledgeBackupHandler {
    fn name(&self) -> &str {
        "create_knowledge_backup"
    }

    fn description(&self) -> &str {
        "立即创建包含当前 WAL 写入的一致性数据库快照"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "reason": { "type": "string", "maxLength": 80, "default": "manual" }
            },
            "additionalProperties": false
        })
    }

    fn module(&self) -> &str {
        "book_wiki"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let db = ctx.db.clone();
        let reason = args
            .get("reason")
            .and_then(Value::as_str)
            .unwrap_or("manual")
            .to_string();
        let backup = tokio::task::spawn_blocking(move || {
            db.create_managed_backup(&reason, db.backup_retention())
        })
        .await
        .map_err(|error| BrainError::Internal(format!("备份任务失败: {error}")))??;
        serde_json::to_value(backup)
            .map_err(|error| BrainError::Internal(format!("备份结果序列化失败: {error}")))
    }
}

pub struct RestoreKnowledgeBackupHandler;

#[async_trait]
impl ToolHandler for RestoreKnowledgeBackupHandler {
    fn name(&self) -> &str {
        "restore_knowledge_backup"
    }

    fn description(&self) -> &str {
        "从应用管理的快照安全恢复数据库，并重建派生全文索引"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "filename": { "type": "string", "minLength": 1, "maxLength": 180 },
                "confirmation": { "type": "string", "const": "RESTORE" }
            },
            "required": ["filename", "confirmation"],
            "additionalProperties": false
        })
    }

    fn module(&self) -> &str {
        "book_wiki"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let filename = required_string(&args, "filename")?;
        let confirmation = required_string(&args, "confirmation")?.to_string();
        let path = ctx.db.managed_backup_path(filename)?;
        let db = ctx.db.clone();
        let report = tokio::task::spawn_blocking(move || {
            db.restore_database_file(&path, &confirmation, db.backup_retention())
        })
        .await
        .map_err(|error| BrainError::Internal(format!("恢复任务失败: {error}")))??;
        ctx.book_wiki_service
            .store()
            .rebuild_knowledge_search_indexes()?;
        Ok(json!({ "validation": report }))
    }
}

pub struct ListWikiSkillsHandler;

pub struct GetWikiSkillDetailHandler;

#[async_trait]
impl ToolHandler for ListWikiSkillsHandler {
    fn name(&self) -> &str {
        "list_wiki_skills"
    }

    fn description(&self) -> &str {
        "列出数据库中版本化保存的 Wiki Skills 及当前书籍的启用范围"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "knowledge_base_id": { "type": "string" }
            },
            "additionalProperties": false
        })
    }

    fn module(&self) -> &str {
        "book_wiki"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        Ok(json!({
            "skills": ctx.book_wiki_service.store().list_wiki_skills(
                args.get("knowledge_base_id").and_then(Value::as_str)
            )?
        }))
    }
}

#[async_trait]
impl ToolHandler for GetWikiSkillDetailHandler {
    fn name(&self) -> &str {
        "get_wiki_skill_detail"
    }

    fn description(&self) -> &str {
        "读取 Wiki Skill 的完整内容、全部版本、资源文件、权限和当前书籍绑定"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "skill_id": { "type": "string" },
                "knowledge_base_id": { "type": "string" }
            },
            "required": ["skill_id"],
            "additionalProperties": false
        })
    }

    fn module(&self) -> &str {
        "book_wiki"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        serde_json::to_value(ctx.book_wiki_service.store().get_wiki_skill_detail(
            required_string(&args, "skill_id")?,
            args.get("knowledge_base_id").and_then(Value::as_str),
        )?)
        .map_err(|error| BrainError::Internal(format!("Skill 详情序列化失败: {error}")))
    }
}

pub struct SaveCustomWikiSkillHandler;

#[async_trait]
impl ToolHandler for SaveCustomWikiSkillHandler {
    fn name(&self) -> &str {
        "save_custom_wiki_skill"
    }

    fn description(&self) -> &str {
        "创建或更新一个只包含受控 Markdown 指令的自定义 Wiki Skill"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "skill_id": { "type": "string" },
                "slug": { "type": "string", "minLength": 1, "maxLength": 64 },
                "name": { "type": "string", "minLength": 1, "maxLength": 100 },
                "description": { "type": "string", "maxLength": 500 },
                "instructions": { "type": "string", "minLength": 1, "maxLength": 12000 },
                "expected_revision": { "type": "integer", "minimum": 1 }
            },
            "required": ["slug", "name", "description", "instructions"],
            "additionalProperties": false
        })
    }

    fn module(&self) -> &str {
        "book_wiki"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let skill = ctx.book_wiki_service.store().save_custom_wiki_skill(
            args.get("skill_id").and_then(Value::as_str),
            required_string(&args, "slug")?,
            required_string(&args, "name")?,
            args.get("description")
                .and_then(Value::as_str)
                .unwrap_or(""),
            required_string(&args, "instructions")?,
            args.get("expected_revision").and_then(Value::as_i64),
        )?;
        serde_json::to_value(skill)
            .map_err(|error| BrainError::Internal(format!("Skill 序列化失败: {error}")))
    }
}

pub struct SetWikiSkillBindingHandler;

pub struct EvaluateWikiSkillVersionHandler;

#[async_trait]
impl ToolHandler for EvaluateWikiSkillVersionHandler {
    fn name(&self) -> &str {
        "evaluate_wiki_skill_version"
    }

    fn description(&self) -> &str {
        "使用固定的离线结构评测集检查一个 Wiki Skill 候选版本，并保存与当前版本的对比结果"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "skill_id": { "type": "string" },
                "version_id": { "type": "string" },
                "suite_slug": {
                    "type": "string",
                    "enum": ["semantic-ingest", "grounded-query", "evidence-research", "evidence-presentation"]
                }
            },
            "required": ["skill_id", "version_id", "suite_slug"],
            "additionalProperties": false
        })
    }

    fn module(&self) -> &str {
        "book_wiki"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        serde_json::to_value(ctx.book_wiki_service.store().evaluate_wiki_skill_version(
            required_string(&args, "skill_id")?,
            required_string(&args, "version_id")?,
            required_string(&args, "suite_slug")?,
        )?)
        .map_err(|error| BrainError::Internal(format!("Skill 评测结果序列化失败: {error}")))
    }
}

pub struct PublishWikiSkillVersionHandler;

pub struct StartWikiSkillBenchmarkHandler;

#[async_trait]
impl ToolHandler for StartWikiSkillBenchmarkHandler {
    fn name(&self) -> &str {
        "start_wiki_skill_benchmark"
    }

    fn description(&self) -> &str {
        "在受限的 DeepSeek Harness 中后台运行固定真实样例，对比 Wiki Skill 候选版本与当前版本"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "knowledge_base_id": { "type": "string" },
                "skill_id": { "type": "string" },
                "version_id": { "type": "string" },
                "suite_slug": {
                    "type": "string",
                    "enum": ["semantic-ingest", "grounded-query", "evidence-research", "evidence-presentation"]
                }
            },
            "required": ["knowledge_base_id", "skill_id", "version_id", "suite_slug"],
            "additionalProperties": false
        })
    }

    fn module(&self) -> &str {
        "book_wiki"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        serde_json::to_value(ctx.book_wiki_service.queue_wiki_skill_benchmark(
            required_string(&args, "knowledge_base_id")?,
            required_string(&args, "skill_id")?,
            required_string(&args, "version_id")?,
            required_string(&args, "suite_slug")?,
        )?)
        .map_err(|error| BrainError::Internal(format!("Skill 基准运行序列化失败: {error}")))
    }
}

pub struct GetWikiSkillBenchmarkHandler;

#[async_trait]
impl ToolHandler for GetWikiSkillBenchmarkHandler {
    fn name(&self) -> &str {
        "get_wiki_skill_benchmark"
    }

    fn description(&self) -> &str {
        "读取 Wiki Skill 真实模型基准的状态、分数、差异指标与逐样例结果"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": { "run_id": { "type": "string" } },
            "required": ["run_id"],
            "additionalProperties": false
        })
    }

    fn module(&self) -> &str {
        "book_wiki"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        serde_json::to_value(
            ctx.book_wiki_service
                .store()
                .get_wiki_skill_benchmark(required_string(&args, "run_id")?)?,
        )
        .map_err(|error| BrainError::Internal(format!("Skill 基准运行序列化失败: {error}")))
    }
}

#[async_trait]
impl ToolHandler for PublishWikiSkillVersionHandler {
    fn name(&self) -> &str {
        "publish_wiki_skill_version"
    }

    fn description(&self) -> &str {
        "发布一个已通过固定评测的 Wiki Skill 候选版本"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "skill_id": { "type": "string" },
                "version_id": { "type": "string" }
            },
            "required": ["skill_id", "version_id"],
            "additionalProperties": false
        })
    }

    fn module(&self) -> &str {
        "book_wiki"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        serde_json::to_value(ctx.book_wiki_service.store().publish_wiki_skill_version(
            required_string(&args, "skill_id")?,
            required_string(&args, "version_id")?,
        )?)
        .map_err(|error| BrainError::Internal(format!("Skill 发布结果序列化失败: {error}")))
    }
}

pub struct RollbackWikiSkillVersionHandler;

#[async_trait]
impl ToolHandler for RollbackWikiSkillVersionHandler {
    fn name(&self) -> &str {
        "rollback_wiki_skill_version"
    }

    fn description(&self) -> &str {
        "把 Wiki Skill 回滚到一个历史已发布版本"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "skill_id": { "type": "string" },
                "version_id": { "type": "string" }
            },
            "required": ["skill_id", "version_id"],
            "additionalProperties": false
        })
    }

    fn module(&self) -> &str {
        "book_wiki"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        serde_json::to_value(ctx.book_wiki_service.store().rollback_wiki_skill_version(
            required_string(&args, "skill_id")?,
            required_string(&args, "version_id")?,
        )?)
        .map_err(|error| BrainError::Internal(format!("Skill 回滚结果序列化失败: {error}")))
    }
}

#[async_trait]
impl ToolHandler for SetWikiSkillBindingHandler {
    fn name(&self) -> &str {
        "set_wiki_skill_binding"
    }

    fn description(&self) -> &str {
        "为一本书启用或停用 Wiki Skill，并限制到问答、研究或智能编译场景"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "knowledge_base_id": { "type": "string" },
                "skill_id": { "type": "string" },
                "enabled": { "type": "boolean" },
                "usage_scope": {
                    "type": "string",
                    "enum": ["qa", "research", "both", "ingest", "all"]
                }
            },
            "required": ["knowledge_base_id", "skill_id", "enabled", "usage_scope"],
            "additionalProperties": false
        })
    }

    fn module(&self) -> &str {
        "book_wiki"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let skill = ctx.book_wiki_service.store().set_wiki_skill_binding(
            required_string(&args, "knowledge_base_id")?,
            required_string(&args, "skill_id")?,
            args.get("enabled")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            required_string(&args, "usage_scope")?,
        )?;
        serde_json::to_value(skill)
            .map_err(|error| BrainError::Internal(format!("Skill 绑定序列化失败: {error}")))
    }
}

pub struct GetAgentRunEventsHandler;

pub struct GetAgentRunInspectionHandler;
pub struct GetAgentRunCitationHandler;

#[async_trait]
impl ToolHandler for GetAgentRunCitationHandler {
    fn name(&self) -> &str {
        "get_agent_run_citation"
    }
    fn description(&self) -> &str {
        "按本次运行的引用编号读取冻结证据与已读范围，不替换为当前实体版本"
    }
    fn module(&self) -> &str {
        "book_wiki"
    }
    fn input_schema(&self) -> Value {
        json!({"type":"object","properties":{"run_id":{"type":"string"},"source_index":{"type":"integer","minimum":0}},"required":["run_id","source_index"],"additionalProperties":false})
    }
    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let index = args
            .get("source_index")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| BrainError::KnowledgeValidation("source_index 必须是非负整数".into()))?;
        serde_json::to_value(
            ctx.book_wiki_service
                .store()
                .get_agent_run_citation(required_string(&args, "run_id")?, index)?,
        )
        .map_err(|error| BrainError::Internal(format!("引用快照序列化失败: {error}")))
    }
}

pub struct GetKnowledgeTaskActivityHandler;

#[async_trait]
impl ToolHandler for GetKnowledgeTaskActivityHandler {
    fn name(&self) -> &str {
        "get_knowledge_task_activity"
    }

    fn description(&self) -> &str {
        "读取研究任务最近一次 Agent 运行及其原生进度事件"
    }

    fn input_schema(&self) -> Value {
        required_id_schema("task_id")
    }

    fn module(&self) -> &str {
        "book_wiki"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let run = ctx
            .book_wiki_service
            .store()
            .get_latest_task_run(required_string(&args, "task_id")?)?;
        let events = match &run {
            Some(run) => ctx
                .book_wiki_service
                .store()
                .list_agent_run_events(&run.id)?,
            None => Vec::new(),
        };
        Ok(json!({ "run": run, "events": events }))
    }
}

#[async_trait]
impl ToolHandler for GetAgentRunEventsHandler {
    fn name(&self) -> &str {
        "get_agent_run_events"
    }

    fn description(&self) -> &str {
        "按序读取 Agent 运行的持久化进度、完成或失败事件"
    }

    fn input_schema(&self) -> Value {
        required_id_schema("run_id")
    }

    fn module(&self) -> &str {
        "book_wiki"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        Ok(json!({
            "events": ctx.book_wiki_service.store().list_agent_run_events(
                required_string(&args, "run_id")?
            )?
        }))
    }
}

#[async_trait]
impl ToolHandler for GetAgentRunInspectionHandler {
    fn name(&self) -> &str {
        "get_agent_run_inspection"
    }

    fn description(&self) -> &str {
        "读取 Agent 运行、阶段事件、有效 Prompt、Skill/配置快照和工具白名单"
    }

    fn input_schema(&self) -> Value {
        required_id_schema("run_id")
    }

    fn module(&self) -> &str {
        "book_wiki"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        serde_json::to_value(
            ctx.book_wiki_service
                .store()
                .get_agent_run_inspection(required_string(&args, "run_id")?)?,
        )
        .map_err(|error| BrainError::Internal(format!("Agent 运行检查信息序列化失败: {error}")))
    }
}

#[async_trait]
impl ToolHandler for GetBookWikiSettingsHandler {
    fn name(&self) -> &str {
        "get_book_wiki_settings"
    }

    fn description(&self) -> &str {
        "读取 Agent 运行配置与数据库中的 Markdown 配置文档"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "knowledge_base_id": { "type": "string" }
            },
            "additionalProperties": false
        })
    }

    fn module(&self) -> &str {
        "book_wiki"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let base_id = args.get("knowledge_base_id").and_then(Value::as_str);
        let store = ctx.book_wiki_service.store();
        let profiles = store.list_runtime_profiles()?;
        let model_providers = store.list_model_provider_profiles()?;
        let documents = store.list_config_documents(base_id)?;
        let health = tokio::task::spawn_blocking(move || inspect_runtime_profiles(profiles))
            .await
            .map_err(|error| BrainError::Internal(format!("运行时检测任务失败: {error}")))?;
        Ok(json!({
            "runtime_profiles": health,
            "model_providers": model_providers,
            "documents": documents
        }))
    }
}

pub struct GetAgentUsageStatsHandler;

#[async_trait]
impl ToolHandler for GetAgentUsageStatsHandler {
    fn name(&self) -> &str {
        "get_agent_usage_stats"
    }

    fn description(&self) -> &str {
        "按日期与调用方汇总 Agent Token 用量，并标明实测或估算来源"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "start_date": { "type": "string", "description": "YYYY-MM-DD" },
                "end_date": { "type": "string", "description": "YYYY-MM-DD" },
                "caller": {
                    "type": "string",
                    "enum": ["knowledge_qa", "knowledge_task"]
                }
            },
            "additionalProperties": false
        })
    }

    fn module(&self) -> &str {
        "book_wiki"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let today = Local::now().date_naive();
        let default_start = today - Duration::days(29);
        let start_date = args
            .get("start_date")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| default_start.format("%Y-%m-%d").to_string());
        let end_date = args
            .get("end_date")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| today.format("%Y-%m-%d").to_string());
        serde_json::to_value(ctx.book_wiki_service.store().get_agent_usage_stats(
            &start_date,
            &end_date,
            args.get("caller").and_then(Value::as_str),
        )?)
        .map_err(|error| BrainError::Internal(format!("Token 统计序列化失败: {error}")))
    }
}

pub struct SaveBookWikiConfigDocumentHandler;

#[async_trait]
impl ToolHandler for SaveBookWikiConfigDocumentHandler {
    fn name(&self) -> &str {
        "save_book_wiki_config_document"
    }

    fn description(&self) -> &str {
        "保存数据库中的知识库 Markdown 配置文档"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "document_id": { "type": "string" },
                "content_md": { "type": "string" },
                "expected_revision": { "type": "integer", "minimum": 1 }
            },
            "required": ["document_id", "content_md", "expected_revision"],
            "additionalProperties": false
        })
    }

    fn module(&self) -> &str {
        "book_wiki"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let document = ctx.book_wiki_service.store().save_config_document(
            required_string(&args, "document_id")?,
            args.get("content_md").and_then(Value::as_str).unwrap_or(""),
            args.get("expected_revision")
                .and_then(Value::as_i64)
                .ok_or_else(|| {
                    BrainError::KnowledgeValidation("缺少 expected_revision".to_string())
                })?,
        )?;
        serde_json::to_value(document)
            .map_err(|error| BrainError::Internal(format!("结果序列化失败: {error}")))
    }
}

pub struct SaveAgentRuntimeProfileHandler;

#[async_trait]
impl ToolHandler for SaveAgentRuntimeProfileHandler {
    fn name(&self) -> &str {
        "save_agent_runtime_profile"
    }

    fn description(&self) -> &str {
        "保存 DeepSeek Harness 或 Claude Code 的本地运行配置，通过 provider_id 关联已配置的模型供应商"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "profile_id": { "type": "string" },
                "executable": { "type": "string", "minLength": 1 },
                "model": { "type": "string" },
                "provider_id": {
                    "type": "string",
                    "description": "关联的模型供应商 ID；留空则使用 Harness 默认 Profile"
                },
                "enabled": { "type": "boolean" },
                "expected_revision": { "type": "integer", "minimum": 1 }
            },
            "required": ["profile_id", "executable", "model", "enabled", "expected_revision"],
            "additionalProperties": false
        })
    }

    fn module(&self) -> &str {
        "book_wiki"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let provider_id = args
            .get("provider_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let profile = ctx.book_wiki_service.store().save_runtime_profile(
            required_string(&args, "profile_id")?,
            required_string(&args, "executable")?,
            args.get("model").and_then(Value::as_str).unwrap_or(""),
            provider_id.as_deref(),
            args.get("enabled")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            args.get("expected_revision")
                .and_then(Value::as_i64)
                .ok_or_else(|| {
                    BrainError::KnowledgeValidation("缺少 expected_revision".to_string())
                })?,
        )?;
        serde_json::to_value(profile)
            .map_err(|error| BrainError::Internal(format!("结果序列化失败: {error}")))
    }
}

pub struct ListModelProvidersHandler;

#[async_trait]
impl ToolHandler for ListModelProvidersHandler {
    fn name(&self) -> &str {
        "list_model_providers"
    }

    fn description(&self) -> &str {
        "列出所有已配置的第三方模型供应商及其凭据状态"
    }

    fn input_schema(&self) -> Value {
        empty_schema()
    }

    fn module(&self) -> &str {
        "book_wiki"
    }

    async fn handle(&self, _args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let providers = ctx
            .book_wiki_service
            .store()
            .list_model_provider_profiles()?;
        serde_json::to_value(providers)
            .map(|value| json!({ "model_providers": value }))
            .map_err(|error| BrainError::Internal(format!("模型供应商列表序列化失败: {error}")))
    }
}

pub struct SaveModelProviderHandler;

#[async_trait]
impl ToolHandler for SaveModelProviderHandler {
    fn name(&self) -> &str {
        "save_model_provider"
    }

    fn description(&self) -> &str {
        "新增或更新模型供应商配置；keychain 模式下 API Key 写入系统凭据库，不落盘"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "provider_id": { "type": "string", "description": "留空则自动生成" },
                "display_name": { "type": "string", "minLength": 1 },
                "api_protocol": {
                    "type": "string",
                    "enum": ["openai-completions", "openai-responses", "anthropic-messages"]
                },
                "base_url": { "type": "string", "minLength": 1 },
                "model": { "type": "string", "minLength": 1 },
                "credential_source": {
                    "type": "string",
                    "enum": ["keychain", "environment"]
                },
                "api_key_env": { "type": "string", "description": "environment 模式必填；keychain 模式忽略" },
                "api_key": { "type": "string", "description": "keychain 模式下写入系统凭据库；编辑时留空保留现有密钥" },
                "clear_api_key": { "type": "boolean", "description": "keychain 模式下清除已保存的密钥" },
                "enabled": { "type": "boolean" },
                "context_window": { "type": ["integer", "null"], "minimum": 1, "maximum": 4294967295_u64 },
                "max_output_tokens": { "type": ["integer", "null"], "minimum": 1, "maximum": 4294967295_u64 },
                "reasoning_policy": { "type": "string", "enum": ["auto", "off", "minimal", "low", "medium", "high", "xhigh", "max"] },
                "expected_revision": { "type": "integer", "minimum": 0 }
            },
            "required": ["display_name", "api_protocol", "base_url", "model", "credential_source"],
            "additionalProperties": false
        })
    }

    fn module(&self) -> &str {
        "book_wiki"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let request = SaveModelProviderRequest {
            provider_id: args
                .get("provider_id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            display_name: required_string(&args, "display_name")?.to_string(),
            api_protocol: required_string(&args, "api_protocol")?.to_string(),
            base_url: required_string(&args, "base_url")?.to_string(),
            model: required_string(&args, "model")?.to_string(),
            credential_source: required_string(&args, "credential_source")?.to_string(),
            api_key_env: args
                .get("api_key_env")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            api_key: args
                .get("api_key")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            clear_api_key: args
                .get("clear_api_key")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            enabled: args.get("enabled").and_then(Value::as_bool).unwrap_or(true),
            context_window: args
                .get("context_window")
                .and_then(Value::as_u64)
                .and_then(|v| u32::try_from(v).ok()),
            max_output_tokens: args
                .get("max_output_tokens")
                .and_then(Value::as_u64)
                .and_then(|v| u32::try_from(v).ok()),
            reasoning_policy: args
                .get("reasoning_policy")
                .and_then(Value::as_str)
                .unwrap_or("auto")
                .to_string(),
            expected_revision: args
                .get("expected_revision")
                .and_then(Value::as_i64)
                .unwrap_or(0),
        };
        let provider = ctx.book_wiki_service.save_model_provider(request).await?;
        serde_json::to_value(provider)
            .map_err(|error| BrainError::Internal(format!("模型供应商序列化失败: {error}")))
    }
}

pub struct DeleteModelProviderHandler;

#[async_trait]
impl ToolHandler for DeleteModelProviderHandler {
    fn name(&self) -> &str {
        "delete_model_provider"
    }

    fn description(&self) -> &str {
        "删除模型供应商配置并清理系统凭据库中的 API Key"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "provider_id": { "type": "string", "minLength": 1 },
                "expected_revision": { "type": "integer", "minimum": 1 }
            },
            "required": ["provider_id", "expected_revision"],
            "additionalProperties": false
        })
    }

    fn module(&self) -> &str {
        "book_wiki"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        ctx.book_wiki_service
            .delete_model_provider(
                required_string(&args, "provider_id")?,
                args.get("expected_revision")
                    .and_then(Value::as_i64)
                    .ok_or_else(|| {
                        BrainError::KnowledgeValidation("缺少 expected_revision".to_string())
                    })?,
            )
            .await?;
        Ok(json!({ "deleted": true }))
    }
}

pub struct VerifyAgentRuntimeHandler;

#[async_trait]
impl ToolHandler for VerifyAgentRuntimeHandler {
    fn name(&self) -> &str {
        "verify_agent_runtime"
    }

    fn description(&self) -> &str {
        "发起一次最小 ACP 模型请求，验证 DeepSeek Harness 会话与凭据"
    }

    fn input_schema(&self) -> Value {
        required_id_schema("profile_id")
    }

    fn module(&self) -> &str {
        "book_wiki"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let result = ctx
            .book_wiki_service
            .verify_runtime(required_string(&args, "profile_id")?)
            .await?;
        serde_json::to_value(result)
            .map_err(|error| BrainError::Internal(format!("结果序列化失败: {error}")))
    }
}

fn required_string<'a>(args: &'a Value, key: &str) -> Result<&'a str, BrainError> {
    args.get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| BrainError::KnowledgeValidation(format!("缺少必需参数 {key}")))
}

fn required_i64(args: &Value, key: &str) -> Result<i64, BrainError> {
    args.get(key)
        .and_then(Value::as_i64)
        .filter(|value| *value > 0)
        .ok_or_else(|| BrainError::KnowledgeValidation(format!("{key} 必须是正整数")))
}

fn string_array(args: &Value, key: &str) -> Result<Vec<String>, BrainError> {
    args.get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| BrainError::KnowledgeValidation(format!("{key} 必须是字符串数组")))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| BrainError::KnowledgeValidation(format!("{key} 必须是字符串数组")))
        })
        .collect()
}

fn empty_schema() -> Value {
    json!({
        "type": "object",
        "properties": {},
        "additionalProperties": false
    })
}

fn required_id_schema(key: &str) -> Value {
    json!({
        "type": "object",
        "properties": {
            (key): { "type": "string" }
        },
        "required": [key],
        "additionalProperties": false
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_research_read_tools_distinguish_legacy_pending_content_and_invalid_mutations() {
        use crate::models::book_wiki::{BookKind, ReaderBook};
        use axum::{body::Body, http::Request};
        use tower::ServiceExt;
        let (ctx, dir, _vault) = crate::AppContext::for_test();
        let store = ctx.book_wiki_service.store();
        store
            .save_reader_books(&[ReaderBook {
                id: "read-research".into(),
                path: dir.path().display().to_string(),
                kind: BookKind::Folder,
                name: "读取研究状态".into(),
                description: String::new(),
                category: String::new(),
                added_at: 1,
                progress: None,
            }])
            .unwrap();
        let base = store.initialize_base("read-research").unwrap();
        let task = store
            .create_task(&base.id, "读取阶段", "不启动真实模型", "research")
            .unwrap();
        ctx.tool_registry
            .register(Arc::new(GetKnowledgeResearchWorkspaceHandler))
            .await;
        ctx.tool_registry
            .register(Arc::new(GetKnowledgeResearchStageHandler))
            .await;
        let app = crate::api::router::create_router(ctx.clone());
        for (arguments, tool, success, legacy) in [
            (
                json!({"task_id":task.id}),
                "get_knowledge_research_workspace",
                true,
                true,
            ),
            (
                json!({"task_id":task.id,"stage_key":"plan","revision":0}),
                "get_knowledge_research_stage",
                false,
                false,
            ),
            (
                json!({"task_id":task.id,"claim_id":"inject-write"}),
                "get_knowledge_research_workspace",
                false,
                false,
            ),
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::post("/v1/tools/call")
                        .header("content-type", "application/json")
                        .body(Body::from(
                            json!({"tool":tool,"arguments":arguments}).to_string(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();
            let raw = axum::body::to_bytes(response.into_body(), 1024 * 1024)
                .await
                .unwrap();
            let value: Value = serde_json::from_slice(&raw).unwrap();
            assert_eq!(value["status"] == "success", success);
            if legacy {
                assert!(value["result"].is_null())
            }
        }
        store.start_task_execution(&task.id).unwrap();
        store.ensure_research_workspace(&task.id).unwrap();
        let workspace = GetKnowledgeResearchWorkspaceHandler
            .handle(json!({"task_id":task.id}), &ctx)
            .await
            .unwrap();
        assert_eq!(workspace["stages"][0]["status"], "pending");
        assert!(workspace["stages"][0].get("content_md").is_none());
        let stage = GetKnowledgeResearchStageHandler
            .handle(json!({"task_id":task.id,"stage_key":"plan"}), &ctx)
            .await
            .unwrap();
        assert_eq!(stage["stage"]["status"], "pending");
        assert_eq!(stage["content_md"], "");
        assert!(GetKnowledgeResearchStageHandler
            .handle(json!({"task_id":task.id,"stage_key":"unknown"}), &ctx)
            .await
            .is_err());
    }

    #[test]
    fn test_model_provider_handlers_expose_schemas_and_module() {
        let handlers: Vec<Box<dyn ToolHandler>> = vec![
            Box::new(ListModelProvidersHandler),
            Box::new(SaveModelProviderHandler),
            Box::new(DeleteModelProviderHandler),
        ];
        assert_eq!(handlers.len(), 3);
        assert!(handlers
            .iter()
            .all(|handler| handler.module() == "book_wiki" && handler.input_schema().is_object()));
    }

    #[test]
    fn test_unfinished_qa_handler_requires_book_scope_and_bounds_limit() {
        let handler = ListUnfinishedQaRunsHandler;
        assert_eq!(handler.module(), "book_wiki");
        let schema = handler.input_schema();
        assert_eq!(schema["required"][0], "knowledge_base_id");
        assert_eq!(schema["properties"]["limit"]["maximum"], 30);
        assert_eq!(schema["additionalProperties"], false);
    }

    #[test]
    fn test_save_agent_runtime_profile_schema_references_provider_id_not_inline_config() {
        let schema = SaveAgentRuntimeProfileHandler.input_schema();
        assert_eq!(schema["properties"]["provider_id"]["type"], "string");
        assert!(schema["properties"].get("provider_config").is_none());
    }

    #[test]
    fn test_save_model_provider_schema_requires_identity_fields_and_optional_api_key() {
        let schema = SaveModelProviderHandler.input_schema();
        let required = schema["required"]
            .as_array()
            .expect("save_model_provider requires a required array");
        assert!(required.iter().any(|value| value == "display_name"));
        assert!(required.iter().any(|value| value == "api_protocol"));
        assert!(required.iter().any(|value| value == "base_url"));
        assert!(required.iter().any(|value| value == "model"));
        assert!(required.iter().any(|value| value == "credential_source"));
        assert!(
            required.iter().all(|value| value != "api_key"),
            "api_key must stay optional so edits can keep the existing secret"
        );
        assert_eq!(
            schema["properties"]["credential_source"]["enum"][0],
            "keychain"
        );
        assert_eq!(
            schema["properties"]["credential_source"]["enum"][1],
            "environment"
        );
    }

    #[test]
    fn test_delete_model_provider_schema_requires_provider_id_and_revision() {
        let schema = DeleteModelProviderHandler.input_schema();
        let required = schema["required"]
            .as_array()
            .expect("delete_model_provider requires a required array");
        assert!(required.iter().any(|value| value == "provider_id"));
        assert!(required.iter().any(|value| value == "expected_revision"));
    }
}
