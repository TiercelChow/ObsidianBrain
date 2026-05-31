//! Tool handler implementations and centralized registration.
//!
//! This module contains all tool handlers organized by module:
//! - `search_handlers` — search_notes, get_note, list_recent_notes
//! - `memory_handlers` — get_memory_stats

pub mod memory_handlers;
pub mod search_handlers;

use std::sync::Arc;

use crate::tools::handlers::memory_handlers::GetMemoryStatsHandler;
use crate::tools::handlers::search_handlers::{
    GetNoteHandler, ListRecentNotesHandler, SearchNotesHandler,
};
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

    // Memory module
    registry.register(Arc::new(GetMemoryStatsHandler)).await;
}
