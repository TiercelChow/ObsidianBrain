//! 代码仓工具处理器

use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::error::BrainError;
use crate::tools::definitions;
use crate::tools::traits::ToolHandler;
use crate::AppContext;

/// 注册代码仓库
pub struct AddCodeRepoHandler;

#[async_trait]
impl ToolHandler for AddCodeRepoHandler {
    fn name(&self) -> &str { "add_code_repo" }
    fn description(&self) -> &str { "注册本地代码仓库" }
    fn input_schema(&self) -> Value { definitions::add_code_repo_schema() }
    fn module(&self) -> &str { "code_repo" }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let path = args.get("path")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| BrainError::Internal("缺少必需参数 'path'".to_string()))?;
        let name = args.get("name")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| BrainError::Internal("缺少必需参数 'name'".to_string()))?;

        tracing::debug!(path = %path, name = %name, "add_code_repo 调用");
        let repo = ctx.repo_manager.register(path, name)?;
        Ok(json!({
            "name": repo.name,
            "path": repo.path.to_string_lossy(),
            "current_branch": repo.current_branch,
            "is_dirty": repo.is_dirty,
            "status": "registered"
        }))
    }
}

/// 列出所有仓库
pub struct ListCodeReposHandler;

#[async_trait]
impl ToolHandler for ListCodeReposHandler {
    fn name(&self) -> &str { "list_code_repos" }
    fn description(&self) -> &str { "列出所有已注册的代码仓库" }
    fn input_schema(&self) -> Value { definitions::list_code_repos_schema() }
    fn module(&self) -> &str { "code_repo" }

    async fn handle(&self, _args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        tracing::debug!("list_code_repos 调用");
        let repos = ctx.repo_manager.list()?;
        let repos_json: Vec<Value> = repos.iter().map(|r| {
            json!({
                "name": r.name,
                "path": r.path.to_string_lossy(),
                "current_branch": r.current_branch,
                "is_dirty": r.is_dirty,
                "languages": r.languages,
                "linked_notes_count": r.linked_notes_count,
            })
        }).collect();
        Ok(json!({ "repos": repos_json, "total": repos_json.len() }))
    }
}

/// 获取仓库详情
pub struct GetRepoDetailHandler;

#[async_trait]
impl ToolHandler for GetRepoDetailHandler {
    fn name(&self) -> &str { "get_repo_detail" }
    fn description(&self) -> &str { "获取代码仓库的详细信息" }
    fn input_schema(&self) -> Value { definitions::get_repo_detail_schema() }
    fn module(&self) -> &str { "code_repo" }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let name = args.get("name")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| BrainError::Internal("缺少必需参数 'name'".to_string()))?;

        tracing::debug!(name = %name, "get_repo_detail 调用");
        let detail = ctx.repo_manager.detail(name)?;
        Ok(json!({
            "name": detail.base.name,
            "path": detail.base.path.to_string_lossy(),
            "current_branch": detail.base.current_branch,
            "language_stats": detail.base.language_stats,
            "is_dirty": detail.base.is_dirty,
            "branches": detail.branches,
            "remote_urls": detail.remote_urls,
            "head_hash": detail.head_hash,
            "total_commits": detail.total_commits,
            "contributors": detail.contributors,
            "linked_notes": detail.base.linked_notes,
            "vscode_uri": detail.vscode_uri,
            "working_dir_status": detail.working_dir_status,
            "recent_commits": detail.base.recent_commits,
        }))
    }
}

/// 关联笔记到仓库
pub struct LinkNoteToRepoHandler;

#[async_trait]
impl ToolHandler for LinkNoteToRepoHandler {
    fn name(&self) -> &str { "link_note_to_repo" }
    fn description(&self) -> &str { "将笔记关联到代码仓库" }
    fn input_schema(&self) -> Value { definitions::link_note_to_repo_schema() }
    fn module(&self) -> &str { "code_repo" }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let note_path = args.get("note_path")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| BrainError::Internal("缺少必需参数 'note_path'".to_string()))?;
        let repo_name = args.get("repo_name")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| BrainError::Internal("缺少必需参数 'repo_name'".to_string()))?;

        tracing::debug!(note_path = %note_path, repo_name = %repo_name, "link_note_to_repo 调用");
        let link = ctx.note_linker.link_note(note_path, repo_name)?;
        Ok(json!({
            "note_path": link.note_path,
            "repo_name": link.repo_name,
            "linked_at": link.linked_at.to_rfc3339(),
            "status": "linked"
        }))
    }
}

/// 获取仓库关联的笔记
pub struct GetLinkedNotesHandler;

#[async_trait]
impl ToolHandler for GetLinkedNotesHandler {
    fn name(&self) -> &str { "get_linked_notes" }
    fn description(&self) -> &str { "获取代码仓库关联的笔记列表" }
    fn input_schema(&self) -> Value { definitions::get_linked_notes_schema() }
    fn module(&self) -> &str { "code_repo" }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let repo_name = args.get("repo_name")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| BrainError::Internal("缺少必需参数 'repo_name'".to_string()))?;

        tracing::debug!(repo_name = %repo_name, "get_linked_notes 调用");
        let notes = ctx.note_linker.get_linked_notes(repo_name)?;
        Ok(json!({ "repo_name": repo_name, "notes": notes, "total": notes.len() }))
    }
}

/// 在 VSCode 中打开仓库
pub struct OpenInVscodeHandler;

#[async_trait]
impl ToolHandler for OpenInVscodeHandler {
    fn name(&self) -> &str { "open_in_vscode" }
    fn description(&self) -> &str { "在 VSCode 中打开代码仓库" }
    fn input_schema(&self) -> Value { definitions::open_in_vscode_schema() }
    fn module(&self) -> &str { "code_repo" }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let name = args.get("name")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| BrainError::Internal("缺少必需参数 'name'".to_string()))?;

        tracing::debug!(name = %name, "open_in_vscode 调用");
        let detail = ctx.repo_manager.detail(name)?;
        let result = crate::core::code_repo::vscode::VscodeOpener::open(name, &detail.base.path);
        Ok(json!({
            "repo_name": result.repo_name,
            "vscode_uri": result.vscode_uri,
            "opened": result.opened,
            "message": result.message
        }))
    }
}
