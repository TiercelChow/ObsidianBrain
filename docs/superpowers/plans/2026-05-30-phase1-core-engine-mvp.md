# Phase 1: Core Engine MVP — Implementation Plan

## Overview

Phase 1 builds the Memory Engine MVP: parse Obsidian notes, chunk them intelligently, index into Tantivy (fulltext) and Qdrant (semantic), provide hybrid search via RRF fusion, and expose everything through an HTTP Tool API.

**Milestone:** Configure MCP Server in Claude Desktop to search Obsidian notes via natural language and get relevant results with source links.

**Timeline:** 2-3 weeks

**Prerequisites:** Phase 0 complete (config, SQLite, file watcher, Qdrant, embedding, LLM, Tantivy all integrated into AppContext).

---

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                   Tool API Layer                         │
│  GET  /v1/tools          → list all tool schemas        │
│  POST /v1/tools/call     → invoke a tool by name        │
│  GET  /v1/health         → health check (existing)      │
└──────────────────────┬──────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────┐
│                  Core Service Layer                      │
│  ┌──────────────────┐  ┌──────────────────────────┐     │
│  │  MemoryService   │  │  HybridSearchEngine      │     │
│  │  - index_file    │  │  - fulltext (Tantivy)    │     │
│  │  - add_memory    │  │  - semantic (Qdrant)     │     │
│  │  - update_memory │  │  - RRF fusion (k=60)     │     │
│  │  - forget_memory │  │  - degrade gracefully    │     │
│  └────────┬─────────┘  └──────────────────────────┘     │
│           │                                              │
│  ┌────────┴─────────────────────────────────────┐       │
│  │  MarkdownParser → SmartChunker → IndexMgr    │       │
│  │  (gray_matter)   (300-800 tok)  (Tantivy +   │       │
│  │                                   Qdrant +   │       │
│  │                                   Embedding) │       │
│  └──────────────────────────────────────────────┘       │
└─────────────────────────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────┐
│              Infrastructure Layer (Phase 0)              │
│  SqliteStore | TantivyIndex | QdrantStore               │
│  EmbeddingProvider | LlmProvider | FileWatcher          │
└─────────────────────────────────────────────────────────┘
```

---

## File Structure

```
src/
├── models/
│   ├── mod.rs                    # re-exports
│   ├── note.rs                   # Note, NoteSummary, ParsedDocument, Section
│   ├── memory.rs                 # Memory, MemoryChunk, MemoryStats
│   └── search.rs                 # SearchResult, HybridSearchResult
├── core/
│   ├── mod.rs                    # re-exports
│   ├── markdown_parser.rs        # MarkdownParser (gray_matter + pulldown-cmark)
│   ├── chunker.rs                # SmartChunker algorithm
│   ├── memory_service.rs         # MemoryService (index pipeline + CRUD)
│   └── search_engine.rs          # HybridSearchEngine (RRF fusion)
├── tools/
│   ├── mod.rs                    # re-exports
│   ├── traits.rs                 # ToolHandler trait, ToolDefinition
│   ├── registry.rs               # ToolRegistry
│   ├── definitions.rs            # JSON Schema definitions for all tools
│   └── handlers/
│       ├── mod.rs
│       ├── search_handlers.rs    # SearchNotesHandler, GetNoteHandler, ListRecentNotesHandler
│       └── memory_handlers.rs    # SearchMemoryHandler, AddMemoryHandler, UpdateMemoryHandler, ForgetMemoryHandler, GetMemoryStatsHandler
└── api/
    ├── router.rs                 # add /v1/tools, /v1/tools/call routes
    └── handlers/
        ├── health.rs             # (existing, keep as-is)
        └── tool_handler.rs       # list_tools, call_tool HTTP handlers
```

---

## Dependencies

Add to `Cargo.toml`:

```toml
pulldown-cmark = "0.10"
gray_matter = "0.2"
jsonschema = "0.18"
```

(`async-trait` and `futures` already present from Phase 0.)

---

## Task Breakdown

### Task 1: Data Models

Define all shared data types used across the system.

**Files:** `src/models/mod.rs`, `src/models/note.rs`, `src/models/memory.rs`, `src/models/search.rs`

**Models to define:**

```rust
// models/note.rs
pub struct ParsedDocument {
    pub path: PathBuf,
    pub frontmatter: HashMap<String, serde_json::Value>,
    pub title: String,
    pub tags: Vec<String>,
    pub sections: Vec<Section>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct Section {
    pub level: u8,              // 1-6
    pub heading: Option<String>,
    pub breadcrumb: Vec<String>,
    pub content: String,
    pub code_blocks: Vec<CodeBlock>,
    pub line_start: usize,
    pub line_end: usize,
}

pub struct CodeBlock {
    pub language: Option<String>,
    pub code: String,
    pub line_start: usize,
    pub line_end: usize,
}

pub struct NoteSummary {
    pub path: String,
    pub title: String,
    pub tags: Vec<String>,
    pub updated_at: DateTime<Utc>,
}

// models/memory.rs
pub struct MemoryChunk {
    pub id: Uuid,
    pub note_path: PathBuf,
    pub chunk_index: usize,
    pub content: String,
    pub breadcrumb: Vec<String>,
    pub tags: Vec<String>,
    pub note_title: String,
    pub token_count: usize,
    pub has_code_block: bool,
}

pub struct MemoryStats {
    pub total_chunks: usize,
    pub total_notes: usize,
    pub tags: Vec<String>,
}

// models/search.rs
pub struct HybridSearchResult {
    pub chunk_id: Uuid,
    pub note_path: PathBuf,
    pub note_title: String,
    pub content: String,
    pub breadcrumb: String,
    pub chunk_index: usize,
    pub rrf_score: f64,
    pub fulltext_rank: Option<usize>,
    pub fulltext_score: Option<f32>,
    pub semantic_rank: Option<usize>,
    pub semantic_score: Option<f32>,
    pub obsidian_uri: String,
}
```

**Tests:** Basic serialization/deserialization round-trips.

**Acceptance criteria:** `cargo check` passes, all model types compile.

---

### Task 2: Markdown Parser

Parse Obsidian Markdown files into structured `ParsedDocument` with frontmatter, sections, headings, code blocks, and tags.

**Files:** `src/core/markdown_parser.rs`

**Dependencies:** `gray_matter`, `pulldown-cmark`

**Key behaviors:**
- Extract YAML frontmatter via `gray_matter::Matter`
- Parse body with `pulldown-cmark` to identify headings (H1-H6), code blocks, text
- Build breadcrumb trail: `["# H1", "## H2", "### H3"]` for each section
- Extract tags from: (1) `frontmatter.tags` array, (2) inline `#tag` patterns in body
- Title resolution: `frontmatter.title` > first H1 heading > filename stem
- Code blocks preserved intact with language annotation

**Tests:**
- Parse a note with frontmatter + multiple heading levels
- Extract tags from both frontmatter and inline `#tags`
- Code block preservation (language + content)
- Title fallback chain (frontmatter > H1 > filename)
- Empty/minimal notes

**Acceptance criteria:** All tests pass, parser handles edge cases (empty files, no frontmatter, nested headings).

---

### Task 3: Smart Chunker

Split `ParsedDocument` sections into `MemoryChunk`s of 300-800 tokens, preserving code blocks and heading context.

**Files:** `src/core/chunker.rs`

**Algorithm (from design doc):**
1. Iterate through sections
2. If section fits in `max_tokens` (800), append to buffer
3. If buffer full, emit as chunk, retain last sentence as overlap
4. If section exceeds `max_tokens`, split by paragraph boundaries
5. Code blocks that exceed `max_tokens` become their own chunk (allowed to exceed)
6. Each chunk carries: breadcrumb, tags, note_path, chunk_index, token_count

**Token estimation:**
```rust
fn estimate_tokens(text: &str) -> usize {
    let chinese = text.chars().filter(|c| !c.is_ascii()).count();
    let english = text.split_whitespace()
        .filter(|w| w.chars().all(|c| c.is_ascii())).count();
    chinese + english
}
```

**Tests:**
- Single short section → 1 chunk
- Long section → multiple chunks, each ≤ 800 tokens
- Code block preserved intact even if > 800 tokens
- Breadcrumb context carried through chunks
- Overlap sentence retained between chunks
- Empty document → 0 chunks

**Acceptance criteria:** Chunker produces valid chunks for all test cases, no chunk exceeds max_tokens (except code blocks).

---

### Task 4: Tool Protocol Foundation

Build the tool registry, handler trait, JSON Schema definitions, and HTTP endpoints.

**Files:** `src/tools/traits.rs`, `src/tools/registry.rs`, `src/tools/definitions.rs`, `src/tools/mod.rs`, `src/api/handlers/tool_handler.rs`, `src/api/router.rs` (modify)

**Core types:**

```rust
// tools/traits.rs
#[async_trait]
pub trait ToolHandler: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> Value;  // JSON Schema
    async fn handle(&self, args: Value, ctx: &AppContext) -> Result<Value, BrainError>;
}

pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    pub module: String,
}

// tools/registry.rs
pub struct ToolRegistry {
    tools: RwLock<HashMap<String, Arc<dyn ToolHandler>>>,
}

impl ToolRegistry {
    pub fn new() -> Self;
    pub fn register(&self, handler: Arc<dyn ToolHandler>);
    pub fn get(&self, name: &str) -> Option<Arc<dyn ToolHandler>>;
    pub fn list(&self) -> Vec<ToolDefinition>;
}
```

**HTTP endpoints:**

```
GET  /v1/tools              → { "tools": [ToolDefinition, ...] }
POST /v1/tools/call         → { "tool": "name", "status": "success"|"error", "result": {...} }
```

**Request/Response:**

```rust
// POST /v1/tools/call request body
pub struct ToolCallRequest {
    pub tool: String,
    pub arguments: Value,
}

// POST /v1/tools/call response body
pub struct ToolCallResponse {
    pub tool: String,
    pub status: String,       // "success" | "error"
    pub result: Option<Value>,
    pub error: Option<ToolError>,
}

pub struct ToolError {
    pub code: String,
    pub message: String,
    pub suggestion: Option<String>,
}
```

**Router modification:** Add `/v1/tools` and `/v1/tools/call` routes to existing router, sharing the same `Arc<AppContext>` state. Add `ToolRegistry` to `AppContext`.

**Tests:**
- Register a mock tool, retrieve it
- Call a registered tool via HTTP, get success response
- Call unknown tool, get error response
- Validate JSON Schema format

**Acceptance criteria:** Tool registry works, HTTP endpoints return correct JSON, error handling works.

---

### Task 5: Hybrid Search Engine (RRF)

Combine Tantivy fulltext search and Qdrant semantic search using Reciprocal Rank Fusion.

**Files:** `src/core/search_engine.rs`

**Algorithm:**
```
RRFscore(d) = 1/(k + rank_fulltext) + 1/(k + rank_semantic)
where k = 60 (configurable via MemoryConfig.rrf_k)
```

**Implementation:**
1. Run Tantivy search → top 20 results with BM25 scores
2. Run Qdrant search → top 20 results with cosine scores (embed query first)
3. Use `tokio::join!` for parallel execution
4. Build HashMap<Uuid, (Option<ft_rank>, Option<sem_rank>, result_data)>
5. Compute RRF score for each doc present in either result set
6. Sort by RRF score descending, take top_k
7. Generate Obsidian URI for each result

**Degradation:**
- If Qdrant unavailable → fulltext-only search (RRF score = 1/(k + rank_ft))
- If Tantivy unavailable → semantic-only search
- If both fail → return error

**Tests:**
- RRF fusion with both sources → correct ranking
- Fulltext-only degradation → results still returned
- Empty results → empty vec
- Obsidian URI format

**Acceptance criteria:** Hybrid search returns ranked results, degrades gracefully when Qdrant is down.

---

### Task 6: Memory Service (Index Pipeline + CRUD)

Build the core indexing pipeline: file → parse → chunk → index (Tantivy + Qdrant + SQLite).

**Files:** `src/core/memory_service.rs`

**Index pipeline (`index_file`):**
1. Read file content
2. Parse with `MarkdownParser` → `ParsedDocument`
3. Chunk with `SmartChunker` → `Vec<MemoryChunk>`
4. Index chunks in Tantivy (delete old by path, add new)
5. Batch embed chunks via `EmbeddingProvider::embed_batch`
6. Upsert vectors to Qdrant with `ChunkPayload`
7. Commit Tantivy index

**CRUD operations:**
- `add_memory(note_path, content, tags)` → create new chunk, index it
- `update_memory(memory_id, content)` → re-embed, re-index
- `forget_memory(memory_id)` → remove from Tantivy + Qdrant + SQLite
- `get_memory_stats()` → count chunks, notes, unique tags

**Error handling:**
- Embedding failure → index in Tantivy only, log warning, continue
- Qdrant failure → index in Tantivy only, log warning, continue
- Parse failure → log error, skip file

**Tests:**
- Index a file → chunks created in Tantivy
- Search after indexing → results returned
- Add memory manually → searchable
- Delete memory → no longer searchable

**Acceptance criteria:** Full index pipeline works end-to-end, CRUD operations functional.

---

### Task 7: Core Tool Implementations

Implement the 8 Phase 1 tools as `ToolHandler` implementations.

**Files:** `src/tools/handlers/search_handlers.rs`, `src/tools/handlers/memory_handlers.rs`, `src/tools/definitions.rs`

**Tools:**

| # | Tool | Handler | Description |
|---|------|---------|-------------|
| 1 | `search_notes` | SearchNotesHandler | Hybrid search, group by note, return top matches |
| 2 | `get_note` | GetNoteHandler | Read file, parse, return full content |
| 3 | `list_recent_notes` | ListRecentNotesHandler | Scan vault, sort by mtime, return summaries |
| 4 | `search_memory` | SearchMemoryHandler | Hybrid search on chunks |
| 5 | `add_memory` | AddMemoryHandler | Create chunk, index it |
| 6 | `update_memory` | UpdateMemoryHandler | Re-embed and re-index |
| 7 | `forget_memory` | ForgetMemoryHandler | Remove from all indexes |
| 8 | `get_memory_stats` | GetMemoryStatsHandler | Return chunk/note/tag counts |

**JSON Schemas (examples):**

```json
// search_notes
{
  "type": "object",
  "properties": {
    "query": { "type": "string", "description": "Search query" },
    "top_k": { "type": "integer", "default": 5, "minimum": 1, "maximum": 50 },
    "tags": { "type": "array", "items": { "type": "string" } }
  },
  "required": ["query"],
  "additionalProperties": false
}
```

**Tests:**
- Each tool handler: valid input → success response
- Each tool handler: invalid input → error response
- search_notes with results → correct JSON structure
- get_note with missing file → NoteNotFound error

**Acceptance criteria:** All 8 tools callable via `POST /v1/tools/call`, return correct responses.

---

### Task 8: File Watcher Integration + Full Index

Wire the `FileWatcher` into the `MemoryService` for automatic re-indexing on file changes.

**Files:** `src/core/memory_service.rs` (modify), `src/main.rs` (modify)

**Implementation:**
1. In `main.rs`, after AppContext is built, start `FileWatcher` if `config.vault.watch_enabled`
2. Spawn a background task that receives `FileChangeEvent`s
3. On `Created`/`Modified` → call `memory_service.index_file(path)`
4. On `Deleted` → call `memory_service.remove_file_index(path)`
5. Add `full_index()` method: walk vault directory, index all `.md` files

**Tests:**
- Modify a file → re-indexed automatically
- Delete a file → removed from index
- Full index → all files indexed

**Acceptance criteria:** File changes trigger automatic re-indexing, full index works.

---

### Task 9: Integration Testing + Polish

End-to-end testing, error handling polish, documentation.

**Files:** Various modifications for bug fixes and polish.

**Tasks:**
- Write integration test: create temp vault → index files → search → verify results
- Test degradation: stop Qdrant → verify fulltext search still works
- Add `protocol` field to `ServerConfig` (for future MCP support)
- Update health check to include `tools_count` and `vault_path`
- Run `cargo clippy -- -D warnings`, fix all warnings
- Run `cargo test`, all tests pass
- Verify `cargo run` boots successfully, `curl /v1/health` returns OK
- Test `POST /v1/tools/call` with `search_notes` tool

**Acceptance criteria:** Full Phase 1 milestone achieved — Claude Desktop can search Obsidian notes via Tool API.

---

## Execution Strategy

Use **Subagent-Driven Development** with the following model selection:

| Task | Complexity | Model | Rationale |
|------|-----------|-------|-----------|
| 1. Data Models | Simple | sonnet | Straightforward struct definitions |
| 2. Markdown Parser | Medium | sonnet | Parsing logic with edge cases |
| 3. Smart Chunker | Medium | sonnet | Algorithm implementation |
| 4. Tool Protocol | Medium | sonnet | Trait + registry + HTTP wiring |
| 5. Hybrid Search | Complex | sonnet | RRF algorithm + degradation logic |
| 6. Memory Service | Complex | sonnet | Full pipeline orchestration |
| 7. Core Tools | Medium | sonnet | 8 handler implementations |
| 8. File Watcher | Simple | sonnet | Wiring existing components |
| 9. Integration | Complex | sonnet | End-to-end testing + polish |

Each task follows: **Implement → Spec Review → Quality Review → Fix → Next**

---

## Success Criteria

After Phase 1 completion:

1. ✅ `cargo clippy -- -D warnings` passes
2. ✅ `cargo test` — all tests pass
3. ✅ `cargo run` boots, `curl /v1/health` shows all components OK
4. ✅ `POST /v1/tools/call` with `search_notes` returns relevant results
5. ✅ File changes in vault trigger automatic re-indexing
6. ✅ Hybrid search degrades gracefully when Qdrant is unavailable
7. ✅ All 8 tools callable via HTTP API
8. ✅ Obsidian URIs in search results are correctly formatted
