//! Tool handler implementations and centralized registration.
//!
//! This module contains all tool handlers organized by module:
//! - `search_handlers` — search_notes, get_note, list_recent_notes
//! - `memory_handlers` — get_memory_stats
//! - `reader_handlers` — list_local_dir, read_local_file, stat_local_path, get/save_reader_history, get/save_reader_books (Markdown Reader)
//! - `code_repo_handlers` — add_code_repo, list_code_repos, get_repo_detail, link_note_to_repo, get_linked_notes, open_in_vscode
//! - `timeline_handlers` — get_timeline

pub mod book_wiki_handlers;
pub mod code_repo_handlers;
pub mod config_handlers;
pub mod memory_handlers;
pub mod reader_handlers;
pub mod search_handlers;
pub mod task_handlers;
pub mod timeline_handlers;

use std::sync::Arc;

use crate::tools::handlers::book_wiki_handlers::*;
use crate::tools::handlers::code_repo_handlers::*;
use crate::tools::handlers::config_handlers::{
    GetConfigHandler, SaveConfigHandler, VerifyLlmHandler,
};
use crate::tools::handlers::memory_handlers::GetMemoryStatsHandler;
use crate::tools::handlers::reader_handlers::*;
use crate::tools::handlers::search_handlers::*;
use crate::tools::handlers::task_handlers::*;
use crate::tools::handlers::timeline_handlers::*;
use crate::tools::registry::ToolRegistry;
use crate::AppContext;

#[cfg(test)]
mod retirement_tests;

/// Register all tool handlers into the given registry.
///
/// Called at startup after AppContext is constructed. Since `ToolRegistry::register`
/// is async, this function is async and must be `.await`ed in the tokio runtime.
pub async fn register_all_tools(registry: &ToolRegistry, _ctx: Arc<AppContext>) {
    // Search module
    registry.register(Arc::new(SearchNotesHandler)).await;
    registry.register(Arc::new(GetNoteHandler)).await;
    registry.register(Arc::new(ListRecentNotesHandler)).await;
    registry.register(Arc::new(ListFilesHandler)).await;

    // Memory module
    registry.register(Arc::new(GetMemoryStatsHandler)).await;

    // Code Repo module
    registry.register(Arc::new(AddCodeRepoHandler)).await;
    registry.register(Arc::new(ListCodeReposHandler)).await;
    registry.register(Arc::new(GetRepoDetailHandler)).await;
    registry.register(Arc::new(LinkNoteToRepoHandler)).await;
    registry.register(Arc::new(GetLinkedNotesHandler)).await;
    registry.register(Arc::new(OpenInVscodeHandler)).await;

    // Timeline module
    registry.register(Arc::new(GetTimelineHandler)).await;
    registry.register(Arc::new(CreateMemoHandler)).await;
    registry.register(Arc::new(BrowseTimelineHandler)).await;
    registry.register(Arc::new(SearchMemosHandler)).await;
    registry.register(Arc::new(SyncMemosHandler)).await;
    registry.register(Arc::new(GetMemoStatsHandler)).await;

    // Personal task management
    registry.register(Arc::new(CreateTaskHandler)).await;
    registry.register(Arc::new(ListTasksHandler)).await;
    registry.register(Arc::new(GetTaskHandler)).await;
    registry.register(Arc::new(UpdateTaskHandler)).await;
    registry.register(Arc::new(SetTaskStatusHandler)).await;
    registry.register(Arc::new(AddSubtaskHandler)).await;
    registry.register(Arc::new(MoveSubtaskHandler)).await;
    registry.register(Arc::new(AddTaskProgressHandler)).await;
    registry.register(Arc::new(GetTaskCalendarHandler)).await;
    registry.register(Arc::new(ArchiveTaskHandler)).await;

    // System config
    registry.register(Arc::new(GetConfigHandler)).await;
    registry.register(Arc::new(SaveConfigHandler)).await;
    registry.register(Arc::new(VerifyLlmHandler)).await;

    // Database-native, per-book knowledge bases
    registry
        .register(Arc::new(ListBookKnowledgeBasesHandler))
        .await;
    registry
        .register(Arc::new(InitializeBookKnowledgeBaseHandler))
        .await;
    registry
        .register(Arc::new(SyncBookKnowledgeBaseHandler))
        .await;
    registry
        .register(Arc::new(GetBookKnowledgeBaseHandler))
        .await;
    registry
        .register(Arc::new(SetBookKnowledgeBaseLifecycleHandler))
        .await;
    registry
        .register(Arc::new(DeleteBookKnowledgeBaseHandler))
        .await;
    registry
        .register(Arc::new(CompileBookKnowledgeBaseHandler))
        .await;
    registry
        .register(Arc::new(RetryKnowledgeSourceReviewHandler))
        .await;
    registry
        .register(Arc::new(ProposeKnowledgeEntryArchiveHandler))
        .await;
    registry
        .register(Arc::new(GetKnowledgeCompileReportHandler))
        .await;
    registry
        .register(Arc::new(CancelBookKnowledgeCompileHandler))
        .await;
    registry
        .register(Arc::new(ListKnowledgeChangeSetsHandler))
        .await;
    registry
        .register(Arc::new(ResolveKnowledgeChangeSetHandler))
        .await;
    registry
        .register(Arc::new(ListKnowledgeEntriesHandler))
        .await;
    registry.register(Arc::new(GetKnowledgeEntryHandler)).await;
    registry
        .register(Arc::new(ProposeKnowledgeEntryEditHandler))
        .await;
    registry
        .register(Arc::new(ProposeKnowledgeEntryMergeHandler))
        .await;
    registry
        .register(Arc::new(ProposeKnowledgeEntrySplitHandler))
        .await;
    registry
        .register(Arc::new(ProposeReaderSelectionHandler))
        .await;
    registry
        .register(Arc::new(GetKnowledgeGraphOverviewHandler))
        .await;
    registry
        .register(Arc::new(FindKnowledgeGraphPathHandler))
        .await;
    registry
        .register(Arc::new(GetKnowledgeGraphSnapshotHandler))
        .await;
    registry
        .register(Arc::new(LintBookKnowledgeBaseHandler))
        .await;
    registry
        .register(Arc::new(ListKnowledgeConversationsHandler))
        .await;
    registry
        .register(Arc::new(GetKnowledgeConversationHandler))
        .await;
    registry
        .register(Arc::new(ListUnfinishedQaRunsHandler))
        .await;
    registry.register(Arc::new(AskBookKnowledgeHandler)).await;
    registry
        .register(Arc::new(SaveKnowledgeAnswerHandler))
        .await;
    registry.register(Arc::new(ListKnowledgeTasksHandler)).await;
    registry
        .register(Arc::new(CreateKnowledgeTaskHandler))
        .await;
    registry
        .register(Arc::new(PreviewKnowledgeTaskBriefHandler))
        .await;
    registry
        .register(Arc::new(GetKnowledgeTaskResultHandler))
        .await;
    registry
        .register(Arc::new(GetKnowledgeResearchWorkspaceHandler))
        .await;
    registry
        .register(Arc::new(GetKnowledgeResearchStageHandler))
        .await;
    registry
        .register(Arc::new(ExecuteKnowledgeTaskHandler))
        .await;
    registry
        .register(Arc::new(CancelKnowledgeTaskHandler))
        .await;
    registry
        .register(Arc::new(GetBookWikiSettingsHandler))
        .await;
    registry
        .register(Arc::new(ListKnowledgeBackupsHandler))
        .await;
    registry
        .register(Arc::new(CreateKnowledgeBackupHandler))
        .await;
    registry
        .register(Arc::new(RestoreKnowledgeBackupHandler))
        .await;
    registry.register(Arc::new(ListWikiSkillsHandler)).await;
    registry.register(Arc::new(GetWikiSkillDetailHandler)).await;
    registry
        .register(Arc::new(SaveCustomWikiSkillHandler))
        .await;
    registry
        .register(Arc::new(EvaluateWikiSkillVersionHandler))
        .await;
    registry
        .register(Arc::new(StartWikiSkillBenchmarkHandler))
        .await;
    registry
        .register(Arc::new(GetWikiSkillBenchmarkHandler))
        .await;
    registry
        .register(Arc::new(PublishWikiSkillVersionHandler))
        .await;
    registry
        .register(Arc::new(RollbackWikiSkillVersionHandler))
        .await;
    registry
        .register(Arc::new(SetWikiSkillBindingHandler))
        .await;
    registry.register(Arc::new(GetAgentRunEventsHandler)).await;
    registry
        .register(Arc::new(GetAgentRunCitationHandler))
        .await;
    registry
        .register(Arc::new(GetAgentRunInspectionHandler))
        .await;
    registry
        .register(Arc::new(GetKnowledgeTaskActivityHandler))
        .await;
    registry.register(Arc::new(GetAgentUsageStatsHandler)).await;
    registry
        .register(Arc::new(SaveBookWikiConfigDocumentHandler))
        .await;
    registry
        .register(Arc::new(SaveAgentRuntimeProfileHandler))
        .await;
    registry.register(Arc::new(VerifyAgentRuntimeHandler)).await;
    registry.register(Arc::new(ListModelProvidersHandler)).await;
    registry.register(Arc::new(SaveModelProviderHandler)).await;
    registry
        .register(Arc::new(DeleteModelProviderHandler))
        .await;

    // Reader (filesystem-scoped, powers the Markdown Reader UI)
    registry.register(Arc::new(ListLocalDirHandler)).await;
    registry.register(Arc::new(ReadLocalFileHandler)).await;
    registry.register(Arc::new(GetReaderHistoryHandler)).await;
    registry.register(Arc::new(SaveReaderHistoryHandler)).await;
    registry.register(Arc::new(GetReaderBooksHandler)).await;
    registry.register(Arc::new(SaveReaderBooksHandler)).await;
    registry.register(Arc::new(StatLocalPathHandler)).await;
}
