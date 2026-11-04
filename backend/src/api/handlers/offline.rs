//! Immutable, bounded read-only exports for the phone's local mirror.
//! SQLite is copied first; export handlers never execute model or mutation tools.
use crate::{
    core::{
        book_wiki::BookWikiService,
        tasks::TaskService,
        timeline::{store::TimelineStore, MemoManager, TimelineConfig, TimelineService},
    },
    error::BrainError,
    infra::{
        book_wiki_store::BookWikiStore, deepseek_harness::DeepSeekHarnessRuntime,
        sqlite_store::SqliteStore, task_index_store::SqliteTaskIndexStore,
    },
    AppContext,
};
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Extension, Json, Router,
};
use chrono::Utc;
use rusqlite::{DatabaseName, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    path::{Path as FsPath, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};
use tempfile::{NamedTempFile, TempDir};
use tokio::sync::Semaphore;
use tokio_util::io::ReaderStream;

const TTL: Duration = Duration::from_secs(30 * 60);
const MAX_RECORD_BYTES: usize = 256 * 1024;
const MAX_TEXT_BYTES: usize = 128 * 1024 * 1024;
const MAX_MARKDOWN_BYTES: u64 = 4 * 1024 * 1024;
const MAX_RESOURCE_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_EXPORT_BYTES: u64 = 2 * MAX_RESOURCE_BYTES;
const MAX_OBJECTS: usize = 100_000;
type OfflineResult<T> = Result<T, (StatusCode, Json<Value>)>;

fn failure(status: StatusCode, message: impl ToString) -> (StatusCode, Json<Value>) {
    (status, Json(json!({"error":message.to_string()})))
}
fn internal(error: impl ToString) -> (StatusCode, Json<Value>) {
    failure(StatusCode::INTERNAL_SERVER_ERROR, error)
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct OfflineIdentity {
    protocol: u32,
    instance_id: String,
    dataset_id: String,
    generation: String,
    capabilities: Vec<String>,
}

struct Exports {
    root: PathBuf,
    build: Semaphore,
    snapshots: Mutex<BTreeMap<String, Arc<Snapshot>>>,
    resources: Mutex<BTreeMap<String, Arc<FrozenResource>>>,
}
impl Exports {
    fn new(db: &SqliteStore) -> Self {
        Self {
            root: db.database_path().with_extension("offline-exports"),
            build: Semaphore::new(1),
            snapshots: Mutex::new(BTreeMap::new()),
            resources: Mutex::new(BTreeMap::new()),
        }
    }
}
struct Snapshot {
    _root: TempDir,
    identity: OfflineIdentity,
    records: Vec<PathBuf>,
    resources: Vec<OfflineResource>,
    coverage: Vec<OfflineCoverage>,
    created_at: String,
    expires: SystemTime,
}
#[derive(Clone, Serialize)]
struct OfflineResource {
    id: String,
    request_path: String,
    path: String,
    size: u64,
    size_bytes: u64,
    sha256: String,
    version: String,
    download_path: String,
    mime_type: String,
    identity: OfflineIdentity,
    expires_at: String,
}
struct FrozenResource {
    meta: OfflineResource,
    file: NamedTempFile,
    expires: SystemTime,
}
#[derive(Clone, Serialize)]
struct OfflineCoverage {
    scope: String,
    complete: bool,
    errors: Vec<String>,
}
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct SnapshotOptions {
    #[serde(default)]
    book_ids: Vec<String>,
    #[serde(default)]
    include_resources: bool,
}
#[derive(Deserialize, Default)]
struct PageQuery {
    #[serde(default)]
    cursor: usize,
    #[serde(default = "page_limit")]
    limit: usize,
}
fn page_limit() -> usize {
    20
}
#[derive(Deserialize)]
struct ResourceQuery {
    path: String,
}

pub fn router(ctx: Arc<AppContext>) -> Router {
    Router::new()
        .route("/identity", get(identity))
        .route("/snapshots", post(create_snapshot))
        .route(
            "/snapshots/:id",
            get(snapshot_page).delete(release_snapshot),
        )
        .route("/snapshots/:id/release", post(release_snapshot))
        .route("/snapshots/:id/resources", get(snapshot_resources))
        .route("/resource", get(resource_manifest))
        .route("/resources/:id", get(resource_download))
        .layer(Extension(Arc::new(Exports::new(&ctx.db))))
        .with_state(ctx)
}

fn identity_for(db: &SqliteStore) -> Result<OfflineIdentity, BrainError> {
    // The installation identity is outside SQLite so importing someone else's DB
    // cannot impersonate their computer. The dataset identity travels with the DB.
    let instance_path = db.database_path().with_extension("offline-instance");
    if !instance_path.exists() {
        let parent = instance_path.parent().unwrap_or(FsPath::new("."));
        let mut temporary = NamedTempFile::new_in(parent)?;
        write!(temporary, "{}", uuid::Uuid::new_v4())?;
        temporary.as_file().sync_all()?;
        if let Err(error) = temporary.persist_noclobber(&instance_path) {
            if error.error.kind() != std::io::ErrorKind::AlreadyExists {
                return Err(error.error.into());
            }
        }
    }
    let instance_id = std::fs::read_to_string(instance_path)?;
    uuid::Uuid::parse_str(&instance_id)
        .map_err(|_| BrainError::Internal("离线实例标识损坏，请检查数据目录".into()))?;
    let (dataset_id, generation) = db.transaction(|conn| {
        let mut values = Vec::new();
        for key in ["offline_dataset_id", "offline_generation"] {
            let existing: Option<String> = conn
                .query_row("SELECT value FROM app_state WHERE key=?1", [key], |row| {
                    row.get(0)
                })
                .optional()?;
            let value = existing.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            conn.execute(
                "INSERT OR IGNORE INTO app_state(key,value) VALUES(?1,?2)",
                [key, &value],
            )?;
            values.push(value);
        }
        Ok((values.remove(0), values.remove(0)))
    })?;
    for value in [&dataset_id, &generation] {
        uuid::Uuid::parse_str(value)
            .map_err(|_| BrainError::Internal("离线数据集标识损坏，请检查数据库".into()))?;
    }
    Ok(OfflineIdentity {
        protocol: 1,
        instance_id,
        dataset_id,
        generation,
        capabilities: vec![
            "text_snapshot".into(),
            "resource_manifest".into(),
            "full_replacement".into(),
            "chunked_records".into(),
        ],
    })
}
async fn identity(State(ctx): State<Arc<AppContext>>) -> OfflineResult<Json<OfflineIdentity>> {
    tokio::task::spawn_blocking(move || identity_for(&ctx.db))
        .await
        .map_err(internal)?
        .map(Json)
        .map_err(internal)
}

struct ExportClock {
    now: chrono::DateTime<Utc>,
    today: chrono::NaiveDate,
}
impl ExportClock {
    fn new() -> Self {
        let local = chrono::Local::now();
        Self {
            now: local.with_timezone(&Utc),
            today: local.date_naive(),
        }
    }
}
impl crate::core::tasks::service::TaskClock for ExportClock {
    fn now_utc(&self) -> chrono::DateTime<Utc> {
        self.now
    }
    fn today_local(&self) -> chrono::NaiveDate {
        self.today
    }
    fn storage_month(&self) -> String {
        self.today.format("%Y-%m").to_string()
    }
}

fn clone_snapshot_context(
    ctx: &Arc<AppContext>,
    directory: &FsPath,
) -> Result<Arc<AppContext>, BrainError> {
    let size = std::fs::metadata(ctx.db.database_path())?.len();
    if size > MAX_EXPORT_BYTES {
        return Err(BrainError::Internal("数据库超过离线导出容量上限".into()));
    }
    let target = directory.join("source.sqlite3");
    ctx.db.with_connection(|conn| {
        conn.backup(DatabaseName::Main, &target, None)?;
        Ok(())
    })?;
    let db = Arc::new(SqliteStore::new(&target)?);
    Ok(Arc::new(AppContext {
        config: ctx.config.clone(),
        components: ctx.components.clone(),
        tool_registry: ctx.tool_registry.clone(),
        db: db.clone(),
        repo_manager: ctx.repo_manager.clone(),
        note_linker: ctx.note_linker.clone(),
        timeline_service: Arc::new(TimelineService::new(
            Arc::new(TimelineStore::new(db.clone())),
            TimelineConfig::default(),
        )),
        memo_manager: Arc::new(MemoManager::new(
            db.clone(),
            ctx.memo_manager.images.clone(),
        )),
        task_service: Arc::new(TaskService::with_clock(
            Arc::new(SqliteTaskIndexStore::new(db.clone())),
            Arc::new(ExportClock::new()),
        )),
        book_wiki_service: Arc::new(BookWikiService::new(
            BookWikiStore::new(db),
            Arc::new(DeepSeekHarnessRuntime::default()),
        )),
        start_time: ctx.start_time,
    }))
}
struct ExportBuilder {
    root: TempDir,
    ctx: Arc<AppContext>,
    records: Vec<PathBuf>,
    coverage: BTreeMap<String, OfflineCoverage>,
    bytes: usize,
}
impl ExportBuilder {
    fn error(&mut self, scope: &str, message: impl ToString) {
        let coverage = self
            .coverage
            .entry(scope.into())
            .or_insert_with(|| OfflineCoverage {
                scope: scope.into(),
                complete: true,
                errors: vec![],
            });
        coverage.complete = false;
        if coverage.errors.len() < 20 {
            coverage.errors.push(message.to_string());
        }
    }
    fn record(
        &mut self,
        scope: &str,
        tool: &str,
        args: Value,
        mut result: Value,
    ) -> Result<(), BrainError> {
        self.coverage
            .entry(scope.into())
            .or_insert_with(|| OfflineCoverage {
                scope: scope.into(),
                complete: true,
                errors: vec![],
            });
        redact(&mut result);
        let serialized =
            serde_json::to_string(&result).map_err(|e| BrainError::Internal(e.to_string()))?;
        let parts = split_utf8(&serialized, MAX_RECORD_BYTES);
        for (index, part) in parts.iter().enumerate() {
            let value = if parts.len() == 1 {
                json!({"tool":tool,"arguments":args,"result":result})
            } else {
                json!({"tool":tool,"arguments":args,"result_json":part,"chunk_index":index,"chunk_count":parts.len()})
            };
            let bytes =
                serde_json::to_vec(&value).map_err(|e| BrainError::Internal(e.to_string()))?;
            if self.bytes + bytes.len() > MAX_TEXT_BYTES || self.records.len() >= MAX_OBJECTS {
                return Err(BrainError::Internal(
                    "离线文本导出超过容量上限，请缩小范围".into(),
                ));
            }
            let path = self
                .root
                .path()
                .join(format!("record-{}.json", self.records.len()));
            std::fs::write(&path, &bytes)?;
            self.bytes += bytes.len();
            self.records.push(path);
        }
        Ok(())
    }
    async fn call(&mut self, scope: &str, tool: &str, args: Value) -> Option<Value> {
        if !READ_EXPORT_TOOLS.contains(&tool) {
            self.error(scope, "不允许导出此操作");
            return None;
        }
        let outcome = match self.ctx.tool_registry.get(tool).await {
            Some(handler) => handler.handle(args.clone(), &self.ctx).await,
            None => Err(BrainError::Internal(format!("未注册的读取工具 {tool}"))),
        };
        match outcome {
            Ok(result) => {
                if let Err(error) = self.record(scope, tool, args, result.clone()) {
                    self.error(scope, error);
                }
                Some(result)
            }
            Err(error) => {
                self.error(scope, format!("{tool}: {error}"));
                None
            }
        }
    }
    fn ids(&self, sql: &str) -> Result<Vec<String>, BrainError> {
        self.ctx.db.with_connection(|conn| {
            let mut statement = conn.prepare(sql)?;
            let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
            let result: Vec<String> = rows.collect::<Result<_, _>>()?;
            if result.len() > MAX_OBJECTS {
                return Err(BrainError::Internal("离线对象数量超过上限".into()));
            }
            Ok(result)
        })
    }
}
// Explicit semantic allowlist: no create/save/recover/probe/compile/model tools.
const READ_EXPORT_TOOLS: &[&str] = &[
    "get_reader_books",
    "get_reader_history",
    "list_local_dir",
    "read_local_file",
    "list_tasks",
    "get_task",
    "browse_timeline",
    "list_book_knowledge_bases",
    "get_book_knowledge_base",
    "list_knowledge_entries",
    "get_knowledge_entry",
    "list_knowledge_conversations",
    "get_knowledge_conversation",
    "list_knowledge_tasks",
    "get_knowledge_task_result",
    "get_knowledge_research_workspace",
    "get_knowledge_research_stage",
    "get_knowledge_task_activity",
    "list_wiki_skills",
    "get_wiki_skill_detail",
    "list_knowledge_change_sets",
];

fn redact(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.retain(|key, _| {
                let normalized = key.to_ascii_lowercase().replace(['_', '-'], "");
                ![
                    "apikey",
                    "authorization",
                    "password",
                    "accesstoken",
                    "refreshtoken",
                    "secret",
                    "credential",
                    "capabilitytoken",
                ]
                .contains(&normalized.as_str())
            });
            for item in map.values_mut() {
                redact(item);
            }
        }
        Value::Array(items) => {
            for item in items {
                redact(item);
            }
        }
        _ => (),
    }
}
fn split_utf8(text: &str, cap: usize) -> Vec<&str> {
    let mut result = vec![];
    let mut remaining = text;
    while !remaining.is_empty() {
        let mut end = remaining.len().min(cap);
        while !remaining.is_char_boundary(end) {
            end -= 1;
        }
        result.push(&remaining[..end]);
        remaining = &remaining[end..];
    }
    if result.is_empty() {
        result.push("");
    }
    result
}

fn prepare_exports(exports: &Exports) -> Result<(), BrainError> {
    std::fs::create_dir_all(&exports.root)?;
    let protected: Vec<PathBuf> = exports
        .snapshots
        .lock()
        .map_err(|_| BrainError::Internal("离线快照锁不可用".into()))?
        .values()
        .filter(|snapshot| snapshot.expires > SystemTime::now())
        .map(|snapshot| snapshot._root.path().to_owned())
        .collect();
    for entry in std::fs::read_dir(&exports.root)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        let meta = entry.metadata()?;
        let old = meta.modified()?.elapsed().unwrap_or_default() > TTL;
        if !old {
            continue;
        }
        // Only remove our own exports. Aborted-process resources are included in
        // disk quota until expiration, rather than becoming invisible orphans.
        if name.starts_with("resource-") && meta.is_file() {
            if let Err(error) = std::fs::remove_file(entry.path()) {
                tracing::warn!(%error,"清理过期离线资源失败");
            }
        } else if name.starts_with("snapshot-")
            && meta.is_dir()
            && exports.build.available_permits() > 0
            && !protected.contains(&entry.path())
        {
            if let Err(error) = std::fs::remove_dir_all(entry.path()) {
                tracing::warn!(%error,"清理过期离线快照失败");
            }
        }
    }
    Ok(())
}

fn markdown_images(text: &str) -> Vec<String> {
    let mut result = Vec::new();
    for event in pulldown_cmark::Parser::new(text) {
        if let pulldown_cmark::Event::Start(pulldown_cmark::Tag::Image { dest_url, .. }) = event {
            result.push(dest_url.into_string());
        }
    }
    let mut remainder = text;
    while let Some(start) = remainder.find("![[") {
        remainder = &remainder[start + 3..];
        if let Some(end) = remainder.find("]]") {
            result.push(remainder[..end].split('|').next().unwrap_or("").to_owned());
            remainder = &remainder[end + 2..];
        } else {
            break;
        }
    }
    result
}

fn normalized_existing_path(path: &FsPath) -> Result<PathBuf, std::io::Error> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            std::path::Component::CurDir => {}
            other => normalized.push(other.as_os_str()),
        }
    }
    std::fs::metadata(&normalized)?;
    Ok(normalized)
}

async fn create_snapshot(
    State(ctx): State<Arc<AppContext>>,
    Extension(exports): Extension<Arc<Exports>>,
    Json(options): Json<SnapshotOptions>,
) -> OfflineResult<Json<Value>> {
    prepare_exports(&exports).map_err(internal)?;
    {
        let mut store = exports.snapshots.lock().map_err(internal)?;
        store.retain(|_, item| item.expires > SystemTime::now());
        if store.len() >= 3 {
            return Err(failure(
                StatusCode::TOO_MANY_REQUESTS,
                "离线快照过多，请稍后重试",
            ));
        }
    }
    let directories = std::fs::read_dir(&exports.root)
        .map_err(internal)?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("snapshot-"))
        .count();
    if directories >= 3 {
        return Err(failure(
            StatusCode::TOO_MANY_REQUESTS,
            "已有离线快照尚未过期，请稍后重试",
        ));
    }
    let _permit = exports
        .build
        .try_acquire()
        .map_err(|_| failure(StatusCode::TOO_MANY_REQUESTS, "已有离线导出正在准备"))?;
    if options.book_ids.len() > 100 {
        return Err(failure(StatusCode::BAD_REQUEST, "一次最多选择100本文集"));
    }
    let original = ctx.clone();
    let export_root = exports.root.clone();
    let (identity, root, copy) = tokio::task::spawn_blocking(move || -> Result<_, BrainError> {
        let identity = identity_for(&ctx.db)?;
        let root = tempfile::Builder::new()
            .prefix("snapshot-")
            .tempdir_in(&export_root)?;
        let copy = clone_snapshot_context(&ctx, root.path())?;
        Ok((identity, root, copy))
    })
    .await
    .map_err(internal)?
    .map_err(internal)?;
    let mut builder = ExportBuilder {
        root,
        ctx: copy,
        records: vec![],
        coverage: BTreeMap::new(),
        bytes: 0,
    };
    export_text(&mut builder).await.map_err(internal)?;
    let mut resources = Vec::new();
    export_reader(
        &mut builder,
        &original,
        &exports,
        &options.book_ids,
        &mut resources,
    )
    .await;
    if options.include_resources && options.book_ids.is_empty() {
        export_attachments(&mut builder, &original, &exports, &mut resources).await;
    }
    // A restore during export invalidates the generation, never publish mixed identity.
    if identity != identity_for(&original.db).map_err(internal)? {
        return Err(failure(
            StatusCode::CONFLICT,
            "数据集在导出期间改变，请重新同步",
        ));
    }
    let id = uuid::Uuid::new_v4().to_string();
    let created_at = Utc::now().to_rfc3339();
    let expires = SystemTime::now() + TTL;
    let snapshot = Arc::new(Snapshot {
        _root: builder.root,
        identity: identity.clone(),
        records: builder.records,
        coverage: builder.coverage.into_values().collect(),
        resources,
        created_at: created_at.clone(),
        expires,
    });
    let response = snapshot_summary(&id, &snapshot);
    let mut store = exports.snapshots.lock().map_err(internal)?;
    store.retain(|_, item| item.expires > SystemTime::now());
    if store.len() >= 3 {
        return Err(failure(
            StatusCode::TOO_MANY_REQUESTS,
            "离线快照过多，请稍后重试",
        ));
    }
    store.insert(id, snapshot);
    Ok(Json(response))
}
fn snapshot_summary(id: &str, snapshot: &Snapshot) -> Value {
    json!({"snapshot_id":id,"identity":snapshot.identity,"created_at":snapshot.created_at,"expires_at":chrono::DateTime::<Utc>::from(snapshot.expires).to_rfc3339(),"record_count":snapshot.records.len(),"resource_count":snapshot.resources.len(),"coverage":snapshot.coverage,"mode":"full_replacement"})
}
fn find_snapshot(exports: &Exports, id: &str) -> OfflineResult<Arc<Snapshot>> {
    let mut store = exports.snapshots.lock().map_err(internal)?;
    store.retain(|_, item| item.expires > SystemTime::now());
    store
        .get(id)
        .cloned()
        .ok_or_else(|| failure(StatusCode::NOT_FOUND, "离线快照不存在或已过期，请重新同步"))
}

async fn release_snapshot(
    Extension(exports): Extension<Arc<Exports>>,
    Path(id): Path<String>,
) -> OfflineResult<Json<Value>> {
    let released = exports
        .snapshots
        .lock()
        .map_err(internal)?
        .remove(&id)
        .is_some();
    Ok(Json(json!({"released":released})))
}
async fn snapshot_page(
    Extension(exports): Extension<Arc<Exports>>,
    Path(id): Path<String>,
    Query(page): Query<PageQuery>,
) -> OfflineResult<Json<Value>> {
    let snapshot = find_snapshot(&exports, &id)?;
    if page.cursor > snapshot.records.len() || page.limit == 0 || page.limit > 100 {
        return Err(failure(StatusCode::BAD_REQUEST, "分页范围无效"));
    }
    let mut records = Vec::new();
    let mut bytes = 0;
    for path in snapshot.records.iter().skip(page.cursor).take(page.limit) {
        let raw = tokio::fs::read(path).await.map_err(internal)?;
        if bytes + raw.len() > 1024 * 1024 && !records.is_empty() {
            break;
        }
        bytes += raw.len();
        records.push(serde_json::from_slice::<Value>(&raw).map_err(internal)?);
    }
    let cursor = page.cursor + records.len();
    Ok(Json(
        json!({"snapshot_id":id,"records":records,"next_cursor":if cursor<snapshot.records.len(){Some(cursor.to_string())}else{None},"complete":cursor==snapshot.records.len(),"coverage":snapshot.coverage}),
    ))
}
async fn snapshot_resources(
    Extension(exports): Extension<Arc<Exports>>,
    Path(id): Path<String>,
    Query(page): Query<PageQuery>,
) -> OfflineResult<Json<Value>> {
    let snapshot = find_snapshot(&exports, &id)?;
    if page.cursor > snapshot.resources.len() || page.limit == 0 || page.limit > 100 {
        return Err(failure(StatusCode::BAD_REQUEST, "分页范围无效"));
    }
    let end = (page.cursor + page.limit).min(snapshot.resources.len());
    Ok(Json(
        json!({"snapshot_id":id,"resources":&snapshot.resources[page.cursor..end],"next_cursor":if end<snapshot.resources.len(){Some(end.to_string())}else{None},"complete":end==snapshot.resources.len()}),
    ))
}

async fn export_text(builder: &mut ExportBuilder) -> Result<(), BrainError> {
    builder.call("reader", "get_reader_books", json!({})).await;
    builder
        .call("reader", "get_reader_history", json!({}))
        .await;
    // Keep original endpoint shapes so normal repositories can ingest this mirror.
    let today = chrono::Local::now().date_naive();
    match crate::infra::home_store::overview(&builder.ctx.db, today) {
        Ok(home) => {
            let mut result =
                serde_json::to_value(home).map_err(|e| BrainError::Internal(e.to_string()))?;
            result["system"] = json!({"version":env!("CARGO_PKG_VERSION"),"uptime_seconds":0,"components":{"server":"snapshot","sqlite":"snapshot"}});
            builder.record(
                "home",
                "home_overview",
                json!({"today":today.to_string()}),
                result,
            )?;
        }
        Err(error) => builder.error("home", error),
    }
    let memo_ids = builder.ids("SELECT id FROM memos ORDER BY timestamp DESC,id")?;
    for id in memo_ids {
        match builder.ctx.memo_manager.get_memo(&id) {
            Ok(memo) => builder.record(
                "timeline",
                "get_memo",
                json!({"memo_id":id}),
                serde_json::to_value(memo).map_err(|e| BrainError::Internal(e.to_string()))?,
            )?,
            Err(error) => builder.error("timeline", error),
        }
    }
    let mut offset = 0;
    loop {
        let result = builder
            .call(
                "timeline",
                "browse_timeline",
                json!({"limit":100,"offset":offset}),
            )
            .await;
        let count = result
            .as_ref()
            .and_then(|v| v["memos"].as_array())
            .map(Vec::len)
            .unwrap_or(0);
        offset += count;
        if count < 100 || offset >= MAX_OBJECTS {
            break;
        }
    }
    let ids = builder.ids("SELECT id FROM task_nodes WHERE role='root' ORDER BY id")?;
    for id in ids {
        builder
            .call("tasks", "get_task", json!({"task_id":id}))
            .await;
    }
    let mut cursor: Option<String> = None;
    loop {
        let mut args = json!({"limit":100,"include_archived":true,"include_subtasks":true});
        if let Some(ref value) = cursor {
            args["cursor"] = json!(value);
        }
        let result = builder.call("tasks", "list_tasks", args).await;
        let next = result.and_then(|r| r["next_cursor"].as_str().map(str::to_owned));
        if next.is_none() || next == cursor {
            break;
        }
        cursor = next;
        if builder.records.len() >= MAX_OBJECTS {
            builder.error("tasks", "任务分页超过上限");
            break;
        }
    }
    builder
        .call("wiki", "list_book_knowledge_bases", json!({}))
        .await;
    let store = builder.ctx.book_wiki_service.store();
    let health: Vec<Value> = store.list_runtime_profiles()?.into_iter().map(|profile| json!({"profile":profile,"available":false,"version":null,"message":"离线快照未运行环境检查"})).collect();
    let settings = json!({"runtime_profiles":health,"model_providers":store.list_model_provider_profiles()?,"documents":store.list_config_documents(None)?});
    builder.record("wiki", "get_book_wiki_settings", json!({}), settings)?;
    let bases = builder.ids("SELECT id FROM knowledge_bases ORDER BY id")?;
    for base in bases {
        let scope = format!("wiki:{base}");
        builder
            .call(
                &scope,
                "get_book_knowledge_base",
                json!({"knowledge_base_id":base}),
            )
            .await;
        builder
            .call(
                &scope,
                "list_knowledge_tasks",
                json!({"knowledge_base_id":base}),
            )
            .await;
        builder
            .call(
                &scope,
                "list_knowledge_change_sets",
                json!({"knowledge_base_id":base}),
            )
            .await;
        builder
            .call(
                &scope,
                "list_knowledge_conversations",
                json!({"knowledge_base_id":base,"limit":100}),
            )
            .await;
        let mut offset = 0;
        loop {
            let result = builder.call(&scope, "list_knowledge_entries", json!({"knowledge_base_id":base,"offset":offset,"limit":100,"include_stale":true})).await;
            let count = result
                .as_ref()
                .and_then(|r| r["entries"].as_array())
                .map(Vec::len)
                .unwrap_or(0);
            offset += count;
            if count < 100 || offset >= MAX_OBJECTS {
                break;
            }
        }
        // Profiles are metadata only. Unlike get_book_wiki_settings this does not
        // run runtime health/probe checks while producing a read-only snapshot.
        let store = builder.ctx.book_wiki_service.store();
        let health: Vec<Value> = store.list_runtime_profiles()?.into_iter().map(|profile| json!({"profile":profile,"available":false,"version":null,"message":"离线快照未运行环境检查"})).collect();
        let settings = json!({"runtime_profiles":health,"model_providers":store.list_model_provider_profiles()?,"documents":store.list_config_documents(Some(&base))?});
        builder.record(
            &scope,
            "get_book_wiki_settings",
            json!({"knowledge_base_id":base}),
            settings,
        )?;
        builder
            .call(
                &scope,
                "list_wiki_skills",
                json!({"knowledge_base_id":base}),
            )
            .await;
    }
    for id in builder.ids("SELECT id FROM knowledge_entries ORDER BY id")? {
        builder
            .call("wiki", "get_knowledge_entry", json!({"entry_id":id}))
            .await;
    }
    for id in builder.ids("SELECT id FROM knowledge_conversations ORDER BY id")? {
        builder
            .call(
                "wiki",
                "get_knowledge_conversation",
                json!({"conversation_id":id}),
            )
            .await;
    }
    for id in builder.ids("SELECT id FROM knowledge_tasks ORDER BY id")? {
        builder
            .call(
                "wiki",
                "get_knowledge_research_workspace",
                json!({"task_id":id}),
            )
            .await;
        builder
            .call("wiki", "get_knowledge_task_activity", json!({"task_id":id}))
            .await;
    }
    for id in builder.ids("SELECT id FROM knowledge_tasks WHERE status='completed' ORDER BY id")? {
        builder
            .call(
                "wiki_reports",
                "get_knowledge_task_result",
                json!({"task_id":id}),
            )
            .await;
    }
    let stages = builder.ctx.db.with_connection(|conn| {
        let mut stmt = conn.prepare("SELECT task_id,stage_key FROM knowledge_research_stages WHERE content_md!='' ORDER BY task_id,ordinal")?;
        let rows = stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?;
        Ok(rows.collect::<Result<Vec<_>,_>>()?)
    })?;
    for (task, stage) in stages {
        builder
            .call(
                "wiki",
                "get_knowledge_research_stage",
                json!({"task_id":task,"stage_key":stage}),
            )
            .await;
    }
    builder.call("wiki", "list_wiki_skills", json!({})).await;
    // Empty modules still have a positive, fully covered snapshot.
    for name in ["home", "reader", "timeline", "tasks", "wiki"] {
        builder
            .coverage
            .entry(name.into())
            .or_insert_with(|| OfflineCoverage {
                scope: name.into(),
                complete: true,
                errors: vec![],
            });
    }
    Ok(())
}

async fn export_reader(
    builder: &mut ExportBuilder,
    original: &Arc<AppContext>,
    exports: &Arc<Exports>,
    selected: &[String],
    resources: &mut Vec<OfflineResource>,
) {
    let books = match builder.ctx.book_wiki_service.store().list_reader_books() {
        Ok(books) => books,
        Err(error) => {
            builder.error("reader", error);
            return;
        }
    };
    for id in selected {
        if !books.iter().any(|book| &book.id == id) {
            builder.error(&format!("reader:{id}"), "文集不在当前书架中");
        }
    }
    for book in books {
        let scope = format!("reader_metadata:{}", book.id);
        let is_selected = selected.contains(&book.id);
        let available = match book.kind {
            crate::models::book_wiki::BookKind::Folder => FsPath::new(&book.path).is_dir(),
            crate::models::book_wiki::BookKind::Pdf => FsPath::new(&book.path).is_file(),
        };
        if !available {
            builder.error(&scope, "文集路径不可用或文件类型已改变");
            if is_selected {
                builder.error(
                    &format!("reader:{}", book.id),
                    "文集路径不可用或文件类型已改变",
                );
            }
            continue;
        }
        let mut files = Vec::new();
        if FsPath::new(&book.path).is_dir() {
            let path = book.path.clone();
            let enumerated =
                tokio::task::spawn_blocking(move || reader_tree(FsPath::new(&path), 0)).await;
            match enumerated {
                Ok(Ok((entries, listed))) => {
                    if is_selected {
                        if let Err(error) =
                            directory_records(builder, &scope, FsPath::new(&book.path), &entries)
                        {
                            builder.error(&scope, error);
                        }
                    }
                    if !is_selected {
                        if let Err(error) = builder.record(
                            &scope,
                            "list_local_dir",
                            json!({"path":book.path,"depth":10}),
                            json!({"root":book.path,"total":entries.len(),"entries":entries}),
                        ) {
                            builder.error(&scope, error);
                        }
                    }
                    files = listed;
                }
                Ok(Err(error)) => builder.error(&scope, error),
                Err(error) => builder.error(&scope, error),
            }
        } else {
            files.push(PathBuf::from(&book.path));
        }
        if !is_selected {
            continue;
        }
        let metadata_complete = builder
            .coverage
            .get(&scope)
            .is_none_or(|coverage| coverage.complete);
        let scope = format!("reader:{}", book.id);
        builder
            .coverage
            .entry(scope.clone())
            .or_insert_with(|| OfflineCoverage {
                scope: scope.clone(),
                complete: true,
                errors: vec![],
            });
        if !metadata_complete {
            builder.error(&scope, "文集目录尚未完整读取，请恢复访问后重试");
            continue;
        }
        for path in files {
            let ext = path
                .extension()
                .and_then(|v| v.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if ![
                "md", "markdown", "pdf", "png", "jpg", "jpeg", "gif", "webp", "svg", "avif", "bmp",
                "ico",
            ]
            .contains(&ext.as_str())
            {
                continue;
            }
            let logical = format!(
                "/v1/reader/raw?path={}",
                urlencoding::encode(&path.to_string_lossy())
            );
            match freeze_resource(original.clone(), exports.clone(), logical).await {
                Ok(resource) => {
                    if let Err(error) =
                        file_stat_record(builder, &scope, &path, resource.meta.size, false)
                    {
                        builder.error(&scope, error);
                    }
                    if ext == "md" || ext == "markdown" {
                        if resource.meta.size > MAX_MARKDOWN_BYTES {
                            builder.error(&scope, "单篇 Markdown 超过 4 MiB 离线保存上限");
                            continue;
                        }
                        match std::fs::read_to_string(resource.file.path()) {
                            Ok(content) => {
                                for dependency in markdown_images(&content) {
                                    if dependency.starts_with("data:")
                                        || dependency.starts_with('#')
                                    {
                                        continue;
                                    }
                                    if dependency.contains("://") || dependency.starts_with("//") {
                                        // The pack covers local dependencies; external sites remain links.
                                        continue;
                                    }
                                    let without_fragment =
                                        dependency.split(['#', '?']).next().unwrap_or("");
                                    let decoded = match urlencoding::decode(without_fragment) {
                                        Ok(decoded) => decoded.into_owned(),
                                        Err(error) => {
                                            builder.error(&scope, error);
                                            continue;
                                        }
                                    };
                                    let dependency_path =
                                        path.parent().unwrap_or(FsPath::new("/")).join(decoded);
                                    match normalized_existing_path(&dependency_path) {
                                        Ok(canonical) => {
                                            let request = format!(
                                                "/v1/reader/raw?path={}",
                                                urlencoding::encode(&canonical.to_string_lossy())
                                            );
                                            if !resources.iter().any(|r| r.request_path == request)
                                            {
                                                match freeze_resource(
                                                    original.clone(),
                                                    exports.clone(),
                                                    request,
                                                )
                                                .await
                                                {
                                                    Ok(resource) => {
                                                        resources.push(resource.meta.clone())
                                                    }
                                                    Err((_, error)) => {
                                                        builder.error(&scope, error.0)
                                                    }
                                                }
                                            }
                                        }
                                        Err(error) => builder
                                            .error(&scope, format!("图片依赖不可用: {error}")),
                                    }
                                }
                                if let Err(error) = builder.record(&scope,"read_local_file",json!({"path":path.to_string_lossy()}),json!({"path":path.to_string_lossy(),"name":path.file_name().map(|s|s.to_string_lossy()),"size":resource.meta.size,"content":content})) { builder.error(&scope,error); }
                            }
                            Err(error) => builder.error(&scope, error),
                        }
                    }
                    resources.push(resource.meta.clone());
                }
                Err((_, error)) => builder.error(&scope, error.0),
            }
        }
    }
}
fn reader_tree(path: &FsPath, depth: usize) -> Result<(Vec<Value>, Vec<PathBuf>), BrainError> {
    if depth > 10 {
        return Err(BrainError::Internal("文集目录超过10层，请分批保存".into()));
    }
    let mut entries = vec![];
    let mut files = vec![];
    for item in std::fs::read_dir(path)? {
        let item = item?;
        let name = item.file_name().to_string_lossy().to_string();
        if name.starts_with('.')
            || [
                "node_modules",
                "target",
                "dist",
                "build",
                "__pycache__",
                "venv",
                ".venv",
                "out",
                "coverage",
            ]
            .contains(&name.as_str())
        {
            continue;
        }
        let kind = item.file_type()?;
        if kind.is_symlink() {
            return Err(BrainError::Internal(
                "文集含有符号链接，请直接添加链接目标后保存离线副本".into(),
            ));
        }
        let path = item.path();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let mut entry = json!({"name":name,"path":path.to_string_lossy(),"is_dir":kind.is_dir(),"is_markdown":(["md","markdown"].contains(&ext.as_str())),"is_pdf":ext=="pdf"});
        if kind.is_dir() {
            let (children, listed) = reader_tree(&path, depth + 1)?;
            entry["children"] = json!(children);
            files.extend(listed);
        } else {
            files.push(path);
        }
        entries.push(entry);
        if files.len() + entries.len() > MAX_OBJECTS {
            return Err(BrainError::Internal("文集文件数量超过上限".into()));
        }
    }
    entries.sort_by_key(|value| {
        (
            value["is_dir"] != true,
            value["name"].as_str().unwrap_or("").to_lowercase(),
        )
    });
    Ok((entries, files))
}

fn file_stat_record(
    builder: &mut ExportBuilder,
    scope: &str,
    path: &FsPath,
    size: u64,
    directory: bool,
) -> Result<(), BrainError> {
    builder.record(scope,"stat_local_path",json!({"path":path.to_string_lossy()}),json!({"exists":true,"is_dir":directory,"is_file":!directory,"path":path.to_string_lossy(),"name":path.file_name().map(|s|s.to_string_lossy()).unwrap_or_default(),"size":size,"ext":path.extension().map(|s|s.to_string_lossy()).unwrap_or_default()}))
}
fn directory_records(
    builder: &mut ExportBuilder,
    scope: &str,
    path: &FsPath,
    entries: &[Value],
) -> Result<(), BrainError> {
    file_stat_record(builder, scope, path, 0, true)?;
    builder.record(
        scope,
        "list_local_dir",
        json!({"path":path.to_string_lossy(),"depth":10}),
        json!({"root":path.to_string_lossy(),"total":entries.len(),"entries":entries}),
    )?;
    for entry in entries {
        if entry["is_dir"] == true {
            if let (Some(path), Some(children)) =
                (entry["path"].as_str(), entry["children"].as_array())
            {
                directory_records(builder, scope, FsPath::new(path), children)?;
            }
        }
    }
    Ok(())
}
async fn export_attachments(
    builder: &mut ExportBuilder,
    original: &Arc<AppContext>,
    exports: &Arc<Exports>,
    resources: &mut Vec<OfflineResource>,
) {
    let candidates = builder.ctx.db.with_connection(|conn| {
        let mut stmt = conn
            .prepare("SELECT DISTINCT path FROM timeline_images WHERE pending=0 ORDER BY path")?;
        let values = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(values)
    });
    match candidates {
        Ok(paths) => {
            for path in paths {
                for prefix in ["images", "thumbnails"] {
                    let request = format!(
                        "/v1/timeline/{prefix}/{}",
                        path.split('/')
                            .map(|part| urlencoding::encode(part).into_owned())
                            .collect::<Vec<_>>()
                            .join("/")
                    );
                    match freeze_resource(original.clone(), exports.clone(), request).await {
                        Ok(resource) => resources.push(resource.meta.clone()),
                        Err((_, error)) => builder.error("timeline_assets", error.0),
                    }
                }
            }
        }
        Err(error) => builder.error("timeline_assets", error),
    }
    let ids = builder.ids("SELECT id FROM knowledge_artifacts ORDER BY id");
    match ids {
        Ok(ids) => {
            for id in ids {
                match freeze_resource(
                    original.clone(),
                    exports.clone(),
                    format!("/v1/knowledge/artifacts/{id}"),
                )
                .await
                {
                    Ok(resource) => resources.push(resource.meta.clone()),
                    Err((_, error)) => builder.error("wiki_assets", error.0),
                }
            }
        }
        Err(error) => builder.error("wiki_assets", error),
    }
}

async fn resource_manifest(
    State(ctx): State<Arc<AppContext>>,
    Extension(exports): Extension<Arc<Exports>>,
    Query(query): Query<ResourceQuery>,
) -> OfflineResult<Json<OfflineResource>> {
    freeze_resource(ctx, exports, query.path)
        .await
        .map(|resource| Json(resource.meta.clone()))
}
async fn freeze_resource(
    ctx: Arc<AppContext>,
    exports: Arc<Exports>,
    request_path: String,
) -> OfflineResult<Arc<FrozenResource>> {
    if request_path.len() > 8192 || !request_path.starts_with("/v1/") {
        return Err(failure(StatusCode::BAD_REQUEST, "资源路径无效"));
    }
    prepare_exports(&exports).map_err(internal)?;
    let identity = identity_for(&ctx.db).map_err(internal)?;
    let url = reqwest::Url::parse(&format!("http://offline{request_path}"))
        .map_err(|_| failure(StatusCode::BAD_REQUEST, "资源路径无效"))?;
    let path = url.path();
    let thumbnail_prefix = "/v1/timeline/thumbnails/";
    // Generating a thumbnail only affects the derived image cache, never memos.
    let thumbnail = if let Some(raw) = path.strip_prefix(thumbnail_prefix) {
        let logical = urlencoding::decode(raw).map_err(internal)?.into_owned();
        Some(
            ctx.memo_manager
                .images
                .thumbnail(&logical)
                .await
                .map_err(|e| failure(StatusCode::NOT_FOUND, e))?,
        )
    } else {
        None
    };
    let source = if thumbnail.is_some() {
        None
    } else if path == "/v1/reader/raw" {
        let raw = url
            .query_pairs()
            .find(|(key, _)| key == "path")
            .map(|(_, value)| value.into_owned())
            .ok_or_else(|| failure(StatusCode::BAD_REQUEST, "缺少文件路径"))?;
        let file = PathBuf::from(raw);
        if !file.is_absolute()
            || file
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(failure(StatusCode::BAD_REQUEST, "文件路径无效"));
        }
        let ext = file
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if ![
            "md", "markdown", "pdf", "png", "jpg", "jpeg", "gif", "webp", "svg", "avif", "bmp",
            "ico",
        ]
        .contains(&ext.as_str())
        {
            return Err(failure(StatusCode::BAD_REQUEST, "离线阅读不支持此文件类型"));
        }
        Some((
            file.clone(),
            mime_guess::from_path(file)
                .first_or_octet_stream()
                .to_string(),
        ))
    } else if let Some(raw) = path.strip_prefix("/v1/timeline/images/") {
        let logical = urlencoding::decode(raw).map_err(internal)?.into_owned();
        Some(
            ctx.memo_manager
                .images
                .asset(&logical)
                .map_err(|e| failure(StatusCode::NOT_FOUND, e))?,
        )
    } else if let Some(id) = path.strip_prefix("/v1/knowledge/artifacts/") {
        let (file, mime, _) = ctx
            .book_wiki_service
            .artifact_path(id)
            .map_err(|e| failure(StatusCode::NOT_FOUND, e))?;
        Some((file, mime))
    } else {
        return Err(failure(StatusCode::BAD_REQUEST, "不支持的离线资源地址"));
    };
    let captured_identity = identity.clone();
    let source_ctx = ctx.clone();
    tokio::task::spawn_blocking(move || -> OfflineResult<Arc<FrozenResource>> {
        let mut store = exports.resources.lock().map_err(internal)?;
        store.retain(|_, item| item.expires > SystemTime::now());
        let used: u64 = std::fs::read_dir(&exports.root)
            .map_err(internal)?
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().starts_with("resource-"))
            .filter_map(|entry| entry.metadata().ok().map(|meta| meta.len()))
            .sum();
        if store.len() >= 10_000 {
            return Err(failure(
                StatusCode::TOO_MANY_REQUESTS,
                "离线资源数量达到上限",
            ));
        }
        let mut output = tempfile::Builder::new()
            .prefix("resource-")
            .tempfile_in(&exports.root)
            .map_err(internal)?;
        let mut hash = Sha256::new();
        let mut size = 0;
        let mime = if let Some((bytes, mime)) = thumbnail {
            size = bytes.len() as u64;
            if size > MAX_RESOURCE_BYTES || used + size > MAX_EXPORT_BYTES {
                return Err(failure(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "离线资源导出空间不足",
                ));
            }
            hash.update(&bytes);
            output.write_all(&bytes).map_err(internal)?;
            mime
        } else {
            let (file, mime) =
                source.ok_or_else(|| failure(StatusCode::BAD_REQUEST, "资源不存在"))?;
            let mut input =
                std::fs::File::open(&file).map_err(|e| failure(StatusCode::NOT_FOUND, e))?;
            let mut buffer = [0u8; 64 * 1024];
            loop {
                let count = input.read(&mut buffer).map_err(internal)?;
                if count == 0 {
                    break;
                }
                size += count as u64;
                if size > MAX_RESOURCE_BYTES || used + size > MAX_EXPORT_BYTES {
                    return Err(failure(
                        StatusCode::PAYLOAD_TOO_LARGE,
                        "离线文件超过导出容量上限",
                    ));
                }
                hash.update(&buffer[..count]);
                output.write_all(&buffer[..count]).map_err(internal)?;
            }
            let expected = hash.clone().finalize();
            let mut second = std::fs::File::open(file).map_err(internal)?;
            let mut check = Sha256::new();
            let mut checked = 0u64;
            loop {
                let count = second.read(&mut buffer).map_err(internal)?;
                if count == 0 {
                    break;
                }
                checked += count as u64;
                if checked > MAX_RESOURCE_BYTES {
                    return Err(failure(
                        StatusCode::CONFLICT,
                        "文件在校验期间增大，请重新下载",
                    ));
                }
                check.update(&buffer[..count]);
            }
            if expected != check.finalize() {
                return Err(failure(
                    StatusCode::CONFLICT,
                    "文件在保存期间改变，请重新下载",
                ));
            }
            mime
        };
        output.as_file().sync_all().map_err(internal)?;
        if identity_for(&source_ctx.db).map_err(internal)? != captured_identity {
            return Err(failure(StatusCode::CONFLICT, "数据集已改变，请重新下载"));
        }
        let digest = hex::encode(hash.finalize());
        if let Some(existing) = store.values().find(|item| {
            item.meta.request_path == request_path
                && item.meta.sha256 == digest
                && item.meta.identity == identity
        }) {
            return Ok(existing.clone());
        }
        let id = uuid::Uuid::new_v4().to_string();
        let expires = SystemTime::now() + TTL;
        let resource = Arc::new(FrozenResource {
            meta: OfflineResource {
                id: id.clone(),
                request_path: request_path.clone(),
                path: request_path,
                size,
                size_bytes: size,
                sha256: digest.clone(),
                version: digest,
                download_path: format!("/v1/offline/resources/{id}"),
                mime_type: mime,
                identity,
                expires_at: chrono::DateTime::<Utc>::from(expires).to_rfc3339(),
            },
            file: output,
            expires,
        });
        store.insert(id, resource.clone());
        Ok(resource)
    })
    .await
    .map_err(internal)?
}
async fn resource_download(
    Extension(exports): Extension<Arc<Exports>>,
    Path(id): Path<String>,
) -> OfflineResult<Response> {
    let resource = {
        let mut store = exports.resources.lock().map_err(internal)?;
        store.retain(|_, item| item.expires > SystemTime::now());
        store
            .get(&id)
            .cloned()
            .ok_or_else(|| failure(StatusCode::NOT_FOUND, "离线资源已过期，请重新获取清单"))?
    };
    let file = tokio::fs::File::open(resource.file.path())
        .await
        .map_err(internal)?;
    // The file handle remains valid through the stream even if expiry removes its pathname.
    let mut response = Body::from_stream(ReaderStream::new(file)).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        resource.meta.mime_type.parse().map_err(internal)?,
    );
    response.headers_mut().insert(
        header::CONTENT_LENGTH,
        resource.meta.size.to_string().parse().map_err(internal)?,
    );
    response.headers_mut().insert(
        header::ETAG,
        format!("\"{}\"", resource.meta.sha256)
            .parse()
            .map_err(internal)?,
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, "no-store".parse().map_err(internal)?);
    Ok(response)
}
#[cfg(test)]
mod tests {
    use crate::AppContext;
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use serde_json::{json, Value};
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_offline_resource_versions_are_immutable_after_source_changes() {
        let (ctx, dir, _) = AppContext::for_test();
        let path = dir.path().join("source.pdf");
        std::fs::write(&path, b"first-version").unwrap();
        let app = crate::api::router::create_router(ctx);
        let source = format!(
            "/v1/reader/raw?path={}",
            urlencoding::encode(&path.to_string_lossy())
        );
        let url = format!("/v1/offline/resource?path={}", urlencoding::encode(&source));
        let (status, first) = request(app.clone(), "GET", &url, json!({})).await;
        assert_eq!(status, StatusCode::OK);
        std::fs::write(&path, b"other-version").unwrap();
        let (_, second) = request(app.clone(), "GET", &url, json!({})).await;
        assert_ne!(first["sha256"], second["sha256"]);
        let response = app
            .clone()
            .oneshot(
                Request::get(first["download_path"].as_str().unwrap())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            to_bytes(response.into_body(), 1024).await.unwrap(),
            &b"first-version"[..]
        );
        let (_, third) = request(app.clone(), "GET", &url, json!({})).await;
        assert_eq!(
            second["id"], third["id"],
            "unchanged resources reuse frozen storage"
        );
        let (status, _) = request(
            app.clone(),
            "GET",
            "/v1/offline/resource?path=%2Fv1%2Ftools",
            json!({}),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let (status, _) = request(app, "GET", "/v1/offline/resources/expired", json!({})).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_offline_record_chunking_preserves_multibyte_json_and_redacts_credentials() {
        let (ctx, root, _) = AppContext::for_test();
        let mut builder = super::ExportBuilder {
            root,
            ctx,
            records: vec![],
            coverage: Default::default(),
            bytes: 0,
        };
        let text = "离线内容与公式".repeat(50_000);
        builder
            .record(
                "wiki",
                "get_knowledge_entry",
                json!({"entry_id":"entry"}),
                json!({"content":text,"nested":{"api_key":"secret","api_key_configured":true}}),
            )
            .unwrap();
        assert!(builder.records.len() > 1);
        let mut rebuilt = String::new();
        for (index, path) in builder.records.iter().enumerate() {
            let raw = std::fs::read(path).unwrap();
            assert!(raw.len() < super::MAX_RECORD_BYTES + 1024);
            let part: Value = serde_json::from_slice(&raw).unwrap();
            assert_eq!(part["chunk_index"], index);
            rebuilt.push_str(part["result_json"].as_str().unwrap());
        }
        let result: Value = serde_json::from_str(&rebuilt).unwrap();
        assert_eq!(result["content"], text);
        assert!(result["nested"].get("api_key").is_none());
        assert_eq!(result["nested"]["api_key_configured"], true);
        assert!(!super::READ_EXPORT_TOOLS.contains(&"recover_completed_qa_answer"));
        assert!(!super::READ_EXPORT_TOOLS.contains(&"verify_agent_runtime"));
    }

    #[test]
    fn test_offline_generation_rotates_after_database_restore() {
        let (ctx, _dir, _) = AppContext::for_test();
        let before = super::identity_for(&ctx.db).unwrap();
        let backup = ctx.db.create_managed_backup("offline-test", 3).unwrap();
        ctx.db
            .restore_database_file(&backup.path, "RESTORE", 3)
            .unwrap();
        let after = super::identity_for(&ctx.db).unwrap();
        assert_eq!(before.instance_id, after.instance_id);
        assert_eq!(before.dataset_id, after.dataset_id);
        assert_ne!(before.generation, after.generation);
    }

    async fn request(
        app: axum::Router,
        method: &str,
        path: &str,
        body: Value,
    ) -> (StatusCode, Value) {
        let response = app
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), 2 * 1024 * 1024)
            .await
            .unwrap();
        (
            status,
            serde_json::from_slice(&bytes)
                .unwrap_or_else(|_| json!({"raw":String::from_utf8_lossy(&bytes)})),
        )
    }

    #[tokio::test]
    async fn test_offline_identity_survives_router_restart_and_dataset_reset_is_isolated() {
        let (ctx, _dir, _) = AppContext::for_test();
        let (_, first) = request(
            crate::api::router::create_router(ctx.clone()),
            "GET",
            "/v1/offline/identity",
            json!({}),
        )
        .await;
        let (_, second) = request(
            crate::api::router::create_router(ctx),
            "GET",
            "/v1/offline/identity",
            json!({}),
        )
        .await;
        assert_eq!(first, second);
        assert_eq!(first["protocol"], 1);
        assert!(!first["instance_id"].as_str().unwrap().is_empty());
        let (other, _other_dir, _) = AppContext::for_test();
        let (_, identity) = request(
            crate::api::router::create_router(other),
            "GET",
            "/v1/offline/identity",
            json!({}),
        )
        .await;
        assert_ne!(first["dataset_id"], identity["dataset_id"]);
    }

    #[tokio::test]
    async fn test_offline_snapshot_is_immutable_paginated_and_does_not_export_secrets() {
        let (ctx, _dir, _) = AppContext::for_test();
        crate::tools::handlers::register_all_tools(&ctx.tool_registry, ctx.clone()).await;
        ctx.db
            .set_state("system_config", r#"{"api_key":"NEVER_EXPORT_ME"}"#)
            .unwrap();
        ctx.db.with_connection(|conn| {
            conn.execute("INSERT INTO reader_books(id,path,kind,name,added_at) VALUES('book','/fixture/book.pdf','pdf','Original',1)", [])?;
            Ok(())
        }).unwrap();
        let app = crate::api::router::create_router(ctx.clone());
        let (status, snapshot) =
            request(app.clone(), "POST", "/v1/offline/snapshots", json!({})).await;
        assert_eq!(status, StatusCode::OK);
        let id = snapshot["snapshot_id"].as_str().unwrap();
        let path = format!("/v1/offline/snapshots/{id}?limit=1");
        let (_, first) = request(app.clone(), "GET", &path, json!({})).await;
        ctx.db
            .with_connection(|conn| {
                conn.execute("UPDATE reader_books SET name='Changed'", [])?;
                Ok(())
            })
            .unwrap();
        let (_, second) = request(app.clone(), "GET", &path, json!({})).await;
        assert_eq!(first, second);
        assert_eq!(
            first["records"][0]["result"]["books"][0]["name"],
            "Original"
        );
        assert_eq!(first["records"].as_array().unwrap().len(), 1);
        assert!(
            snapshot["coverage"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|v| !v["scope"].as_str().unwrap().starts_with("reader_metadata:"))
                .all(|v| v["complete"] == true),
            "{snapshot}"
        );
        assert!(!first.to_string().contains("NEVER_EXPORT_ME"));
        let (status, _) = request(
            app.clone(),
            "GET",
            &format!("/v1/offline/snapshots/{id}?cursor=bad"),
            json!({}),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let (status, _) = request(
            app.clone(),
            "GET",
            "/v1/offline/snapshots/not-a-token",
            json!({}),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (status, released) = request(
            app.clone(),
            "POST",
            &format!("/v1/offline/snapshots/{id}/release"),
            json!({}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(released["released"], true);
        let (status, _) = request(app, "GET", &path, json!({})).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_offline_reader_pack_collects_dependencies_and_marks_missing_resources_partial() {
        let (ctx, dir, _) = AppContext::for_test();
        crate::tools::handlers::register_all_tools(&ctx.tool_registry, ctx.clone()).await;
        let folder = dir.path().join("book");
        std::fs::create_dir(&folder).unwrap();
        std::fs::write(
            folder.join("chapter.md"),
            "# Chapter\n![outside](../outside.png)\n![missing](missing.png)",
        )
        .unwrap();
        std::fs::write(dir.path().join("outside.png"), b"outside-image-bytes").unwrap();
        ctx.db.with_connection(|conn| {
            conn.execute("INSERT INTO reader_books(id,path,kind,name,added_at) VALUES('folder',?1,'folder','Book',1)",[folder.to_string_lossy().as_ref()])?;
            conn.execute("INSERT INTO reader_books(id,path,kind,name,added_at) VALUES('missing','/nonexistent/offline-missing-directory','folder','Missing',2)",[])?;
            Ok(())
        }).unwrap();
        let app = crate::api::router::create_router(ctx);
        let (status, snapshot) = request(
            app.clone(),
            "POST",
            "/v1/offline/snapshots",
            json!({"book_ids":["folder","missing"]}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let cover = snapshot["coverage"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["scope"] == "reader:folder")
            .unwrap();
        assert_eq!(cover["complete"], false);
        let missing = snapshot["coverage"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["scope"] == "reader:missing")
            .unwrap();
        assert_eq!(
            missing["complete"], false,
            "missing directory must not claim a complete empty pack"
        );
        let id = snapshot["snapshot_id"].as_str().unwrap();
        let (_, assets) = request(
            app.clone(),
            "GET",
            &format!("/v1/offline/snapshots/{id}/resources"),
            json!({}),
        )
        .await;
        assert!(assets["resources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["request_path"].as_str().unwrap().contains("outside.png")));
        let (_, page) = request(
            app,
            "GET",
            &format!("/v1/offline/snapshots/{id}?limit=100"),
            json!({}),
        )
        .await;
        assert!(page["records"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["tool"] == "read_local_file"));
    }
}
