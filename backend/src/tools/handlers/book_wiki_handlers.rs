use std::process::Command;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::error::BrainError;
use crate::models::book_wiki::RuntimeHealth;
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
                "limit": { "type": "integer", "minimum": 1, "maximum": 200, "default": 100 }
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
        let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(100) as usize;
        Ok(json!({
            "entries": ctx.book_wiki_service.store().list_entries(
                base_id,
                query,
                entry_type,
                limit,
            )?
        }))
    }
}

pub struct GetKnowledgeEntryHandler;

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

#[async_trait]
impl ToolHandler for CreateKnowledgeTaskHandler {
    fn name(&self) -> &str {
        "create_knowledge_task"
    }

    fn description(&self) -> &str {
        "创建一项书籍研究任务；在 Harness 可用前保持草稿状态"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "knowledge_base_id": { "type": "string" },
                "title": { "type": "string", "minLength": 1 },
                "description": { "type": "string", "default": "" },
                "task_type": {
                    "type": "string",
                    "enum": ["research", "refresh", "review"],
                    "default": "research"
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
        let task = ctx.book_wiki_service.store().create_task(
            required_string(&args, "knowledge_base_id")?,
            required_string(&args, "title")?,
            args.get("description")
                .and_then(Value::as_str)
                .unwrap_or(""),
            args.get("task_type")
                .and_then(Value::as_str)
                .unwrap_or("research"),
        )?;
        serde_json::to_value(task)
            .map_err(|error| BrainError::Internal(format!("结果序列化失败: {error}")))
    }
}

pub struct GetBookWikiSettingsHandler;

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
        let profiles = ctx.book_wiki_service.store().list_runtime_profiles()?;
        let health = tokio::task::spawn_blocking(move || inspect_runtime_profiles(profiles))
            .await
            .map_err(|error| BrainError::Internal(format!("运行时检测任务失败: {error}")))?;
        Ok(json!({
            "runtime_profiles": health,
            "documents": ctx.book_wiki_service.store().list_config_documents(base_id)?
        }))
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
        "保存 DeepSeek Harness 或 Claude Code 的本地运行配置"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "profile_id": { "type": "string" },
                "executable": { "type": "string", "minLength": 1 },
                "model": { "type": "string" },
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
        let profile = ctx.book_wiki_service.store().save_runtime_profile(
            required_string(&args, "profile_id")?,
            required_string(&args, "executable")?,
            args.get("model").and_then(Value::as_str).unwrap_or(""),
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

fn inspect_runtime_profiles(
    profiles: Vec<crate::models::book_wiki::RuntimeProfile>,
) -> Vec<RuntimeHealth> {
    profiles
        .into_iter()
        .map(|profile| {
            if !profile.enabled {
                return RuntimeHealth {
                    profile,
                    available: false,
                    version: None,
                    message: "运行时已停用".to_string(),
                };
            }
            match Command::new(&profile.executable).arg("--version").output() {
                Ok(output) if output.status.success() => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let version = stdout
                        .lines()
                        .chain(stderr.lines())
                        .find(|line| !line.trim().is_empty())
                        .map(|line| line.trim().to_string());
                    RuntimeHealth {
                        profile,
                        available: true,
                        version,
                        message: "运行时可用".to_string(),
                    }
                }
                Ok(output) => RuntimeHealth {
                    profile,
                    available: false,
                    version: None,
                    message: format!("运行时返回状态 {}", output.status),
                },
                Err(error) => RuntimeHealth {
                    profile,
                    available: false,
                    version: None,
                    message: format!("未找到可执行文件: {error}"),
                },
            }
        })
        .collect()
}

fn required_string<'a>(args: &'a Value, key: &str) -> Result<&'a str, BrainError> {
    args.get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| BrainError::KnowledgeValidation(format!("缺少必需参数 {key}")))
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
    use crate::models::book_wiki::RuntimeProfile;

    #[test]
    fn test_inspect_runtime_profiles_reports_missing_executable() {
        let health = inspect_runtime_profiles(vec![RuntimeProfile {
            id: "runtime-test".to_string(),
            name: "Test".to_string(),
            runtime: "deepseek_harness".to_string(),
            executable: "/path/that/does/not/exist/deepseek-harness".to_string(),
            model: String::new(),
            enabled: true,
            revision: 1,
            updated_at: String::new(),
        }]);

        assert_eq!(health.len(), 1);
        assert!(!health[0].available);
        assert!(health[0].message.contains("未找到"));
    }
}
