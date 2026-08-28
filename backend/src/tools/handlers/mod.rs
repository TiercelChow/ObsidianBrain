//! Tool handler implementations and centralized registration.
//!
//! This module contains all tool handlers organized by module:
//! - `search_handlers` — search_notes, get_note, list_recent_notes
//! - `memory_handlers` — get_memory_stats
//! - `reader_handlers` — list_local_dir, read_local_file, stat_local_path, get/save_reader_history, get/save_reader_books (Markdown Reader)
//! - `code_repo_handlers` — add_code_repo, list_code_repos, get_repo_detail, link_note_to_repo, get_linked_notes, open_in_vscode
//! - `timeline_handlers` — get_timeline
//! - `inspiration_handlers` — get_inspiration
//! - `radar_handlers` — get_radar, add_to_vault, dismiss_radar_item

pub mod code_repo_handlers;
pub mod config_handlers;
pub mod explore_handlers;
pub mod inspiration_handlers;
pub mod knowledge_handlers;
pub mod memory_handlers;
pub mod radar_handlers;
pub mod reader_handlers;
pub mod search_handlers;
pub mod task_handlers;
pub mod timeline_handlers;
pub mod wiki_handlers;

use std::sync::Arc;

use crate::tools::handlers::code_repo_handlers::*;
use crate::tools::handlers::config_handlers::{
    GetConfigHandler, SaveConfigHandler, VerifyLlmHandler,
};
use crate::tools::handlers::explore_handlers::*;
use crate::tools::handlers::inspiration_handlers::*;
use crate::tools::handlers::knowledge_handlers::GetKnowledgeInsightsHandler;
use crate::tools::handlers::memory_handlers::GetMemoryStatsHandler;
use crate::tools::handlers::radar_handlers::*;
use crate::tools::handlers::reader_handlers::*;
use crate::tools::handlers::search_handlers::*;
use crate::tools::handlers::task_handlers::*;
use crate::tools::handlers::timeline_handlers::*;
use crate::tools::handlers::wiki_handlers::*;
use crate::tools::registry::ToolRegistry;
use crate::AppContext;

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
    registry
        .register(Arc::new(GetKnowledgeInsightsHandler))
        .await;

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

    // Inspiration module
    registry.register(Arc::new(GetInspirationHandler)).await;

    // Radar module
    registry.register(Arc::new(GetRadarHandler)).await;
    registry.register(Arc::new(AddToVaultHandler)).await;
    registry.register(Arc::new(DismissRadarItemHandler)).await;

    // System config
    registry.register(Arc::new(GetConfigHandler)).await;
    registry.register(Arc::new(SaveConfigHandler)).await;
    registry.register(Arc::new(VerifyLlmHandler)).await;

    // Wiki module
    registry.register(Arc::new(IngestSourceHandler)).await;
    registry.register(Arc::new(QueryWikiHandler)).await;
    registry.register(Arc::new(LintWikiHandler)).await;
    registry.register(Arc::new(GetWikiStatusHandler)).await;

    // Knowledge exploration (Wiki-powered)
    registry.register(Arc::new(DiscoverGapsHandler)).await;
    registry.register(Arc::new(GenerateQuestionsHandler)).await;
    registry.register(Arc::new(ConceptCollisionHandler)).await;

    // Reader (filesystem-scoped, powers the Markdown Reader UI)
    registry.register(Arc::new(ListLocalDirHandler)).await;
    registry.register(Arc::new(ReadLocalFileHandler)).await;
    registry.register(Arc::new(GetReaderHistoryHandler)).await;
    registry.register(Arc::new(SaveReaderHistoryHandler)).await;
    registry.register(Arc::new(GetReaderBooksHandler)).await;
    registry.register(Arc::new(SaveReaderBooksHandler)).await;
    registry.register(Arc::new(StatLocalPathHandler)).await;
}
