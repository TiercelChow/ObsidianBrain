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
    models::book_wiki::ReaderBook,
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
use futures::StreamExt;
use rusqlite::{DatabaseName, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Read, Write},
    path::{Path as FsPath, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};
use tempfile::{NamedTempFile, TempDir, TempPath};
use tokio::sync::Semaphore;
use tokio_util::io::ReaderStream;

const TTL: Duration = Duration::from_secs(30 * 60);
const MAX_RECORD_BYTES: usize = 256 * 1024;
const MAX_TEXT_BYTES: usize = 128 * 1024 * 1024;
const MAX_MARKDOWN_BYTES: u64 = 4 * 1024 * 1024;
const LEGACY_DIRECTORY_DEPTH: usize = 10;
const MOBILE_DIRECTORY_DEPTH: usize = 15; // Matches the online list_local_dir limit.
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
    resource_prefix: String,
    snapshot_prefix: String,
    build: Semaphore,
    snapshots: Mutex<BTreeMap<String, Arc<Snapshot>>>,
    resources: Mutex<BTreeMap<String, Arc<FrozenResource>>>,
}
impl Exports {
    fn new(db: &SqliteStore) -> Self {
        let session = uuid::Uuid::new_v4();
        Self {
            root: db.database_path().with_extension("offline-exports"),
            resource_prefix: format!("resource-{session}-"),
            snapshot_prefix: format!("snapshot-{session}-"),
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
    mode: &'static str,
    versions: BTreeMap<String, String>,
    unchanged_scopes: Vec<String>,
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
    // Keep ownership of the pathname, without an open descriptor per cached asset.
    file: TempPath,
    expires: SystemTime,
    source_stamp: Option<SourceStamp>,
    thumbnail_only: bool,
}

#[derive(PartialEq, Eq)]
struct SourceStamp {
    path: PathBuf,
    device: u64,
    inode: u64,
    length: u64,
    modified: (i64, i64),
    changed: (i64, i64),
}

fn source_stamp(path: &FsPath) -> Option<SourceStamp> {
    metadata_stamp(path, &std::fs::metadata(path).ok()?)
}

#[cfg(unix)]
fn metadata_stamp(path: &FsPath, metadata: &std::fs::Metadata) -> Option<SourceStamp> {
    use std::os::unix::fs::MetadataExt;
    metadata.is_file().then(|| SourceStamp {
        path: path.into(),
        device: metadata.dev(),
        inode: metadata.ino(),
        length: metadata.len(),
        modified: (metadata.mtime(), metadata.mtime_nsec()),
        changed: (metadata.ctime(), metadata.ctime_nsec()),
    })
}

#[cfg(not(unix))]
fn metadata_stamp(_path: &FsPath, _metadata: &std::fs::Metadata) -> Option<SourceStamp> {
    None
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
    #[serde(default)]
    profile: SnapshotProfile,
    #[serde(default)]
    known_versions: BTreeMap<String, String>,
    #[serde(default)]
    reader_files: Vec<ReaderFile>,
    reader_depth: Option<usize>,
}

impl SnapshotOptions {
    fn directory_depth(&self) -> usize {
        if self.profile == SnapshotProfile::Mobile {
            self.reader_depth
                .unwrap_or(LEGACY_DIRECTORY_DEPTH)
                .clamp(LEGACY_DIRECTORY_DEPTH, MOBILE_DIRECTORY_DEPTH)
        } else {
            LEGACY_DIRECTORY_DEPTH
        }
    }
}

#[derive(Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum SnapshotProfile {
    #[default]
    Legacy,
    Mobile,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReaderFile {
    book_id: String,
    path: PathBuf,
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
            "mobile_incremental".into(),
            "mobile_reader_depth".into(),
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
    group: Option<PendingGroup>,
}

#[derive(Default)]
struct PendingGroup {
    records: Vec<Vec<u8>>,
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
            let (pending_count, pending_bytes) = self
                .group
                .as_ref()
                .map_or((0, 0), |group| (group.records.len(), group.bytes));
            if self.bytes + pending_bytes + bytes.len() > MAX_TEXT_BYTES
                || self.records.len() + pending_count >= MAX_OBJECTS
            {
                return Err(BrainError::Internal(
                    "离线文本导出超过容量上限，请缩小范围".into(),
                ));
            }
            if let Some(group) = &mut self.group {
                group.bytes += bytes.len();
                group.records.push(bytes);
            } else {
                self.persist_record(&bytes)?;
            }
        }
        Ok(())
    }

    fn persist_record(&mut self, bytes: &[u8]) -> Result<(), BrainError> {
        let path = self
            .root
            .path()
            .join(format!("record-{}.json", self.records.len()));
        std::fs::write(&path, bytes)?;
        self.bytes += bytes.len();
        self.records.push(path);
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
    for entry in std::fs::read_dir(&exports.root)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        let meta = entry.metadata()?;
        let old = meta.modified()?.elapsed().unwrap_or_default() > TTL;
        if !old {
            continue;
        }
        // Registered exports own their paths through RAII. Sweep only old
        // process leftovers, never an export currently owned by this process.
        if name.starts_with("resource-")
            && !name.starts_with(&exports.resource_prefix)
            && meta.is_file()
        {
            if let Err(error) = std::fs::remove_file(entry.path()) {
                tracing::warn!(%error,"清理过期离线资源失败");
            }
        } else if name.starts_with("snapshot-")
            && !name.starts_with(&exports.snapshot_prefix)
            && meta.is_dir()
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
    // Only registered snapshots occupy a slot. A failed or interrupted
    // export can leave a directory without a snapshot to release.
    let _permit = exports
        .build
        .try_acquire()
        .map_err(|_| failure(StatusCode::TOO_MANY_REQUESTS, "已有离线导出正在准备"))?;
    let mobile = options.profile == SnapshotProfile::Mobile;
    let reader_pack = !options.book_ids.is_empty() || !options.reader_files.is_empty();
    if options.book_ids.len() + options.reader_files.len() > 100 {
        return Err(failure(
            StatusCode::BAD_REQUEST,
            "一次最多选择100项阅读内容",
        ));
    }
    if (!mobile && !options.reader_files.is_empty())
        || (mobile && options.include_resources != reader_pack)
    {
        return Err(failure(
            StatusCode::BAD_REQUEST,
            "离线保存范围与资源选项不一致",
        ));
    }
    let original = ctx.clone();
    let export_root = exports.root.clone();
    let snapshot_prefix = exports.snapshot_prefix.clone();
    let (identity, root, copy) = tokio::task::spawn_blocking(move || -> Result<_, BrainError> {
        let identity = identity_for(&ctx.db)?;
        let root = tempfile::Builder::new()
            .prefix(&snapshot_prefix)
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
        group: None,
    };
    let mut resources = Vec::new();
    let mut versions = BTreeMap::new();
    let mut unchanged_scopes = Vec::new();
    let mode = if mobile && reader_pack {
        export_mobile_reader(
            &mut builder,
            &original,
            &exports,
            &options,
            &mut resources,
            &mut versions,
            &mut unchanged_scopes,
        )
        .await
        .map_err(internal)?;
        "mobile_reader"
    } else if mobile {
        export_mobile(
            &mut builder,
            &original,
            &exports,
            &options,
            &mut resources,
            &mut versions,
            &mut unchanged_scopes,
        )
        .await
        .map_err(internal)?;
        "mobile_incremental"
    } else {
        export_text(&mut builder).await.map_err(internal)?;
        export_reader(&mut builder, &original, &exports, &options, &mut resources).await;
        if options.include_resources && options.book_ids.is_empty() {
            export_attachments(&mut builder, &original, &exports, &mut resources).await;
        }
        "full_replacement"
    };
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
        mode,
        versions,
        unchanged_scopes,
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
    json!({"snapshot_id":id,"identity":snapshot.identity,"created_at":snapshot.created_at,"expires_at":chrono::DateTime::<Utc>::from(snapshot.expires).to_rfc3339(),"record_count":snapshot.records.len(),"resource_count":snapshot.resources.len(),"coverage":snapshot.coverage,"mode":snapshot.mode,"versions":snapshot.versions,"unchanged_scopes":snapshot.unchanged_scopes})
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

async fn export_home_and_shelf(
    builder: &mut ExportBuilder,
    mobile: bool,
) -> Result<(), BrainError> {
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
            if mobile {
                // Exclude wall-clock jitter while still updating on a new local day.
                result["generated_at"] = json!(format!("{today}T00:00:00Z"));
            }
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
    Ok(())
}

fn export_memos(builder: &mut ExportBuilder) -> Result<BTreeSet<String>, BrainError> {
    let mut images = BTreeSet::new();
    let memo_ids = builder.ids("SELECT id FROM memos ORDER BY timestamp DESC,id")?;
    for id in memo_ids {
        match builder.ctx.memo_manager.get_memo(&id) {
            Ok(memo) => {
                images.extend(memo.images.iter().cloned());
                builder.record(
                    "timeline",
                    "get_memo",
                    json!({"memo_id":id}),
                    serde_json::to_value(memo).map_err(|e| BrainError::Internal(e.to_string()))?,
                )?;
            }
            Err(error) => builder.error("timeline", error),
        }
    }
    Ok(images)
}

async fn export_text(builder: &mut ExportBuilder) -> Result<(), BrainError> {
    export_home_and_shelf(builder, false).await?;
    export_memos(builder)?;
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
    export_tasks(builder).await?;
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

async fn export_tasks(builder: &mut ExportBuilder) -> Result<(), BrainError> {
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
        if next.is_none() {
            break;
        }
        if next == cursor {
            builder.error("tasks", "任务分页没有前进");
            break;
        }
        cursor = next;
        if builder.records.len()
            + builder
                .group
                .as_ref()
                .map_or(0, |group| group.records.len())
            >= MAX_OBJECTS
        {
            builder.error("tasks", "任务分页超过上限");
            break;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn finish_mobile_group(
    builder: &mut ExportBuilder,
    name: &str,
    complete: bool,
    known_versions: &BTreeMap<String, String>,
    versions: &mut BTreeMap<String, String>,
    unchanged_scopes: &mut Vec<String>,
    resources: &[OfflineResource],
    selection: Option<&[u8]>,
) -> Result<bool, BrainError> {
    let group = builder
        .group
        .take()
        .ok_or_else(|| BrainError::Internal("离线分组不存在".into()))?;
    if complete {
        let mut digest = Sha256::new();
        digest.update(b"mobile-v1");
        digest.update(name.as_bytes());
        if let Some(selection) = selection {
            digest.update((selection.len() as u64).to_le_bytes());
            digest.update(selection);
        }
        for record in &group.records {
            digest.update((record.len() as u64).to_le_bytes());
            digest.update(record);
        }
        let mut resource_order: Vec<_> = resources.iter().collect();
        resource_order.sort_by_key(|r| &r.request_path);
        for resource in resource_order {
            digest.update((resource.request_path.len() as u64).to_le_bytes());
            digest.update(resource.request_path.as_bytes());
            digest.update(resource.sha256.as_bytes());
        }
        let version = hex::encode(digest.finalize());
        let unchanged = known_versions.get(name) == Some(&version);
        versions.insert(name.into(), version);
        if unchanged {
            unchanged_scopes.push(name.into());
            return Ok(true);
        }
    }
    for record in group.records {
        builder.persist_record(&record)?;
    }
    Ok(false)
}

async fn export_mobile(
    builder: &mut ExportBuilder,
    original: &Arc<AppContext>,
    exports: &Arc<Exports>,
    options: &SnapshotOptions,
    resources: &mut Vec<OfflineResource>,
    versions: &mut BTreeMap<String, String>,
    unchanged_scopes: &mut Vec<String>,
) -> Result<(), BrainError> {
    builder.group = Some(PendingGroup::default());
    export_home_and_shelf(builder, true).await?;
    builder
        .call("wiki", "list_book_knowledge_bases", json!({}))
        .await;
    builder
        .call("wiki", "list_knowledge_tasks", json!({}))
        .await;
    export_reader(builder, original, exports, options, resources).await;
    finish_mobile_group(
        builder,
        "overview",
        ["home", "reader", "wiki"].iter().all(|scope| {
            builder
                .coverage
                .get(*scope)
                .is_none_or(|item| item.complete)
        }),
        &options.known_versions,
        versions,
        unchanged_scopes,
        &[],
        None,
    )?;

    builder.group = Some(PendingGroup::default());
    let images = export_memos(builder)?;
    for path in images {
        let request = format!(
            "/v1/timeline/thumbnails/{}",
            path.split('/')
                .map(|part| urlencoding::encode(part).into_owned())
                .collect::<Vec<_>>()
                .join("/")
        );
        match freeze_resource_with_policy(original.clone(), exports.clone(), request, true).await {
            Ok(resource) => resources.push(resource.meta.clone()),
            Err((_, error)) => builder.error("timeline_assets", error.0),
        }
    }
    let timeline_complete = builder
        .coverage
        .get("timeline")
        .is_none_or(|item| item.complete);
    if finish_mobile_group(
        builder,
        "timeline",
        timeline_complete,
        &options.known_versions,
        versions,
        unchanged_scopes,
        resources,
        None,
    )? {
        resources.clear();
    }

    builder.group = Some(PendingGroup::default());
    export_tasks(builder).await?;
    finish_mobile_group(
        builder,
        "tasks",
        builder
            .coverage
            .get("tasks")
            .is_none_or(|item| item.complete),
        &options.known_versions,
        versions,
        unchanged_scopes,
        &[],
        None,
    )?;
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
    options: &SnapshotOptions,
    resources: &mut Vec<OfflineResource>,
) {
    let books = match builder.ctx.book_wiki_service.store().list_reader_books() {
        Ok(books) => books,
        Err(error) => {
            builder.error("reader", error);
            return;
        }
    };
    for id in &options.book_ids {
        if !books.iter().any(|book| &book.id == id) {
            builder.error(&format!("reader:{id}"), "文集不在当前书架中");
        }
    }
    for book in books {
        export_reader_book(
            builder,
            original,
            exports,
            &book,
            options.book_ids.contains(&book.id),
            &[],
            resources,
            options,
        )
        .await;
    }
}

async fn export_mobile_reader(
    builder: &mut ExportBuilder,
    original: &Arc<AppContext>,
    exports: &Arc<Exports>,
    options: &SnapshotOptions,
    resources: &mut Vec<OfflineResource>,
    versions: &mut BTreeMap<String, String>,
    unchanged_scopes: &mut Vec<String>,
) -> Result<(), BrainError> {
    let books = builder.ctx.book_wiki_service.store().list_reader_books()?;
    let mut selection: BTreeMap<&str, (bool, BTreeSet<&FsPath>)> = BTreeMap::new();
    for id in &options.book_ids {
        selection.entry(id.as_str()).or_default().0 = true;
    }
    for file in &options.reader_files {
        selection
            .entry(file.book_id.as_str())
            .or_default()
            .1
            .insert(&file.path);
    }
    let mut changed_books = Vec::new();
    for (id, (whole_book, paths)) in selection {
        let scope = format!("reader:{id}");
        let Some(book) = books.iter().find(|book| book.id == id) else {
            builder.error(&scope, "文集不在当前书架中");
            continue;
        };
        let paths: Vec<_> = paths.into_iter().collect();
        let mut stable_book = book.clone();
        // Reading position belongs to shelf sync and must not invalidate files.
        stable_book.progress = None;
        let selection_key = serde_json::to_vec(&(stable_book, whole_book, &paths))
            .map_err(|error| BrainError::Internal(error.to_string()))?;
        builder.group = Some(PendingGroup::default());
        let mut book_resources = Vec::new();
        export_reader_book(
            builder,
            original,
            exports,
            book,
            whole_book,
            &paths,
            &mut book_resources,
            options,
        )
        .await;
        let complete = builder
            .coverage
            .get(&scope)
            .is_some_and(|item| item.complete);
        if !finish_mobile_group(
            builder,
            &scope,
            complete,
            &options.known_versions,
            versions,
            unchanged_scopes,
            &book_resources,
            Some(&selection_key),
        )? {
            changed_books.push(book);
            resources.extend(book_resources);
        }
    }
    if !changed_books.is_empty() {
        builder.record(
            "reader",
            "get_reader_books",
            json!({}),
            json!({"books":changed_books}),
        )?;
    }
    resources.sort_by(|a, b| a.request_path.cmp(&b.request_path));
    resources.dedup_by(|a, b| a.request_path == b.request_path && a.sha256 == b.sha256);
    if resources
        .windows(2)
        .any(|pair| pair[0].request_path == pair[1].request_path)
    {
        return Err(BrainError::Internal(
            "共享阅读文件在导出期间改变，请重新下载".into(),
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn export_reader_book(
    builder: &mut ExportBuilder,
    original: &Arc<AppContext>,
    exports: &Arc<Exports>,
    book: &ReaderBook,
    is_selected: bool,
    chosen_files: &[&FsPath],
    resources: &mut Vec<OfflineResource>,
    options: &SnapshotOptions,
) {
    let mobile = options.profile == SnapshotProfile::Mobile;
    let scope = format!("reader_metadata:{}", book.id);
    let available = match book.kind {
        crate::models::book_wiki::BookKind::Folder => FsPath::new(&book.path).is_dir(),
        crate::models::book_wiki::BookKind::Pdf => FsPath::new(&book.path).is_file(),
        crate::models::book_wiki::BookKind::Epub => {
            builder.error(&scope, "EPUB 暂不支持离线快照，请在常规阅境轩阅读");
            return;
        }
    };
    if !available {
        builder.error(&scope, "文集路径不可用或文件类型已改变");
        if is_selected || !chosen_files.is_empty() {
            builder.error(
                &format!("reader:{}", book.id),
                "文集路径不可用或文件类型已改变",
            );
        }
        return;
    }
    if !chosen_files.is_empty() {
        let file_scope = format!("reader:{}", book.id);
        builder
            .coverage
            .entry(file_scope.clone())
            .or_insert_with(|| OfflineCoverage {
                scope: file_scope.clone(),
                complete: true,
                errors: vec![],
            });
        for file in chosen_files {
            match selected_reader_path(FsPath::new(&book.path), file) {
                Ok(()) if !is_selected => {
                    export_reader_file(
                        builder,
                        original,
                        exports,
                        &file_scope,
                        file,
                        resources,
                        mobile,
                    )
                    .await;
                }
                Ok(()) => {}
                Err(error) => builder.error(&file_scope, error),
            }
        }
        if !is_selected {
            return;
        }
    }
    let mut files = Vec::new();
    if FsPath::new(&book.path).is_dir() {
        let max_depth = options.directory_depth();
        let path = book.path.clone();
        let enumerated =
            tokio::task::spawn_blocking(move || reader_tree(FsPath::new(&path), 0, max_depth))
                .await;
        match enumerated {
            Ok(Ok((entries, listed))) => {
                let root = FsPath::new(&book.path);
                let result = match (is_selected, mobile) {
                    (true, false) => directory_records(builder, &scope, root, &entries),
                    (true, true) => {
                        file_stat_record(builder, &scope, root, 0, true).and_then(|()| {
                            directory_record(builder, &scope, root, &entries, max_depth)
                        })
                    }
                    (false, _) => directory_record(builder, &scope, root, &entries, max_depth),
                };
                if let Err(error) = result {
                    builder.error(&scope, error);
                }
                files = listed;
            }
            Ok(Err(error)) => builder.error(&scope, error),
            Err(error) => builder.error(&scope, error),
        }
    } else {
        let path = FsPath::new(&book.path);
        if mobile && !is_selected {
            match std::fs::metadata(path) {
                Ok(meta) => {
                    if let Err(error) = file_stat_record(builder, &scope, path, meta.len(), false) {
                        builder.error(&scope, error);
                    }
                }
                Err(error) => builder.error(&scope, error),
            }
        }
        files.push(path.to_path_buf());
    }
    if !is_selected {
        return;
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
        return;
    }
    files.sort();
    for path in files {
        export_reader_file(builder, original, exports, &scope, &path, resources, mobile).await;
    }
}

fn selected_reader_path(root: &FsPath, file: &FsPath) -> Result<(), BrainError> {
    if !file.is_absolute()
        || !matches!(
            file.extension()
                .and_then(|ext| ext.to_str())
                .map(str::to_ascii_lowercase)
                .as_deref(),
            Some("md" | "markdown" | "pdf")
        )
    {
        return Err(BrainError::Internal(
            "单文件离线下载仅支持文集内的 Markdown 和 PDF".into(),
        ));
    }
    let relative = file
        .strip_prefix(root)
        .map_err(|_| BrainError::Internal("所选文件不在对应文集内".into()))?;
    let mut checked = root.to_path_buf();
    if std::fs::symlink_metadata(&checked)?.is_symlink() {
        return Err(BrainError::Internal("离线保存不支持符号链接".into()));
    }
    for component in relative.components() {
        let std::path::Component::Normal(name) = component else {
            return Err(BrainError::Internal("所选文件不在对应文集内".into()));
        };
        checked.push(name);
        if std::fs::symlink_metadata(&checked)?.is_symlink() {
            return Err(BrainError::Internal("离线保存不支持符号链接".into()));
        }
    }
    if !checked.is_file() {
        return Err(BrainError::Internal("所选路径不是普通文件".into()));
    }
    Ok(())
}

async fn export_reader_file(
    builder: &mut ExportBuilder,
    original: &Arc<AppContext>,
    exports: &Arc<Exports>,
    scope: &str,
    path: &FsPath,
    resources: &mut Vec<OfflineResource>,
    mobile: bool,
) {
    let ext = path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if ![
        "md", "markdown", "pdf", "png", "jpg", "jpeg", "gif", "webp", "svg", "avif", "bmp", "ico",
    ]
    .contains(&ext.as_str())
    {
        return;
    }
    let assets_scope = scope
        .strip_prefix("reader:")
        .filter(|_| mobile)
        .map(|book_id| format!("reader_assets:{book_id}"));
    let assets_scope = assets_scope.as_deref().unwrap_or(scope);
    let primary_scope = if ["md", "markdown", "pdf"].contains(&ext.as_str()) {
        scope
    } else {
        assets_scope
    };
    let logical = format!(
        "/v1/reader/raw?path={}",
        urlencoding::encode(&path.to_string_lossy())
    );
    match freeze_resource(original.clone(), exports.clone(), logical).await {
        Ok(resource) => {
            if let Err(error) =
                file_stat_record(builder, primary_scope, path, resource.meta.size, false)
            {
                builder.error(primary_scope, error);
            }
            if ext == "md" || ext == "markdown" {
                if resource.meta.size > MAX_MARKDOWN_BYTES {
                    builder.error(scope, "单篇 Markdown 超过 4 MiB 离线保存上限");
                    return;
                }
                match std::fs::read_to_string(&resource.file) {
                    Ok(content) => {
                        for dependency in markdown_images(&content) {
                            if dependency.starts_with("data:") || dependency.starts_with('#') {
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
                                    builder.error(assets_scope, error);
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
                                    if !resources.iter().any(|r| r.request_path == request) {
                                        match freeze_resource(
                                            original.clone(),
                                            exports.clone(),
                                            request,
                                        )
                                        .await
                                        {
                                            Ok(resource) => resources.push(resource.meta.clone()),
                                            Err((_, error)) => builder.error(assets_scope, error.0),
                                        }
                                    }
                                }
                                Err(error) => {
                                    builder.error(assets_scope, format!("图片依赖不可用: {error}"))
                                }
                            }
                        }
                        if let Err(error) = builder.record(scope,"read_local_file",json!({"path":path.to_string_lossy()}),json!({"path":path.to_string_lossy(),"name":path.file_name().map(|s|s.to_string_lossy()),"size":resource.meta.size,"content":content})) { builder.error(scope,error); }
                    }
                    Err(error) => builder.error(scope, error),
                }
            }
            if !(mobile && (ext == "md" || ext == "markdown"))
                && !resources
                    .iter()
                    .any(|item| item.request_path == resource.meta.request_path)
            {
                resources.push(resource.meta.clone());
            }
        }
        Err((_, error)) => builder.error(primary_scope, error.0),
    }
}

fn reader_tree(
    path: &FsPath,
    depth: usize,
    max_depth: usize,
) -> Result<(Vec<Value>, Vec<PathBuf>), BrainError> {
    if depth > max_depth {
        return Err(BrainError::Internal(format!(
            "文集目录超过{max_depth}层，请分批保存"
        )));
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
            let (children, listed) = reader_tree(&path, depth + 1, max_depth)?;
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
            value["path"].as_str().unwrap_or("").to_owned(),
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
fn directory_record(
    builder: &mut ExportBuilder,
    scope: &str,
    path: &FsPath,
    entries: &[Value],
    max_depth: usize,
) -> Result<(), BrainError> {
    builder.record(
        scope,
        "list_local_dir",
        json!({"path":path.to_string_lossy(),"depth":max_depth}),
        json!({"root":path.to_string_lossy(),"total":entries.len(),"entries":entries}),
    )
}

fn directory_records(
    builder: &mut ExportBuilder,
    scope: &str,
    path: &FsPath,
    entries: &[Value],
) -> Result<(), BrainError> {
    file_stat_record(builder, scope, path, 0, true)?;
    directory_record(builder, scope, path, entries, LEGACY_DIRECTORY_DEPTH)?;
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
    freeze_resource_with_policy(ctx, exports, request_path, false).await
}

async fn freeze_resource_with_policy(
    ctx: Arc<AppContext>,
    exports: Arc<Exports>,
    request_path: String,
    thumbnail_only: bool,
) -> OfflineResult<Arc<FrozenResource>> {
    if request_path.len() > 8192 || !request_path.starts_with("/v1/") {
        return Err(failure(StatusCode::BAD_REQUEST, "资源路径无效"));
    }
    let identity = identity_for(&ctx.db).map_err(internal)?;
    let url = reqwest::Url::parse(&format!("http://offline{request_path}"))
        .map_err(|_| failure(StatusCode::BAD_REQUEST, "资源路径无效"))?;
    let path = url.path();
    let thumbnail_logical = path
        .strip_prefix("/v1/timeline/thumbnails/")
        .map(urlencoding::decode)
        .transpose()
        .map_err(internal)?
        .map(|path| path.into_owned());
    let source = if let Some(logical) = &thumbnail_logical {
        Some(
            ctx.memo_manager
                .images
                .asset(logical)
                .map_err(|error| failure(StatusCode::NOT_FOUND, error))?,
        )
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
    let stamp = source.as_ref().and_then(|(path, _)| source_stamp(path));
    if let Some(current) = &stamp {
        let store = exports.resources.lock().map_err(internal)?;
        if let Some(existing) = store.values().find(|item| {
            item.meta.request_path == request_path
                && item.meta.identity == identity
                && item.thumbnail_only == thumbnail_only
                && item.expires > SystemTime::now() + Duration::from_secs(60)
                && item.source_stamp.as_ref() == Some(current)
        }) {
            return Ok(existing.clone());
        }
    }
    prepare_exports(&exports).map_err(internal)?;
    // Strict mobile exports never substitute original photo bytes.
    let thumbnail = if let Some(logical) = thumbnail_logical {
        let image = if thumbnail_only {
            ctx.memo_manager.images.offline_thumbnail(&logical).await
        } else {
            ctx.memo_manager.images.thumbnail(&logical).await
        };
        Some(image.map_err(|error| failure(StatusCode::NOT_FOUND, error))?)
    } else {
        None
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
            .prefix(&exports.resource_prefix)
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
            if let Some(before) = &stamp {
                let opened = input.metadata().map_err(internal)?;
                if metadata_stamp(&file, &opened).as_ref() != Some(before) {
                    return Err(failure(
                        StatusCode::CONFLICT,
                        "文件在打开期间改变，请重新下载",
                    ));
                }
            }
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
            if let Some(before) = &stamp {
                let after = input.metadata().map_err(internal)?;
                if metadata_stamp(&file, &after).as_ref() != Some(before) {
                    return Err(failure(
                        StatusCode::CONFLICT,
                        "文件在保存期间改变，请重新下载",
                    ));
                }
            } else {
                // Without inode/mtime/ctime, confirm the source with a second pass.
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
            }
            mime
        };
        if let Some(before) = &stamp {
            if source_stamp(&before.path).as_ref() != Some(before) {
                return Err(failure(
                    StatusCode::CONFLICT,
                    "文件在保存期间改变，请重新下载",
                ));
            }
        }
        // The immutable download is a temporary file, not a crash-recovery copy.
        // File writes are unbuffered and are readable once the manifest is published.
        if identity_for(&source_ctx.db).map_err(internal)? != captured_identity {
            return Err(failure(StatusCode::CONFLICT, "数据集已改变，请重新下载"));
        }
        let digest = hex::encode(hash.finalize());
        if let Some(existing) = store.values().find(|item| {
            item.meta.request_path == request_path
                && item.meta.sha256 == digest
                && item.meta.identity == identity
                && item.thumbnail_only == thumbnail_only
                && item.source_stamp == stamp
                && item.expires > SystemTime::now() + Duration::from_secs(60)
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
            file: output.into_temp_path(),
            expires,
            source_stamp: stamp,
            thumbnail_only,
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
    // The resource stays owned until the stream opens the path. Expiry
    // removes it from the registry, never out from under an active open.
    let file = tokio::fs::File::open(&resource.file)
        .await
        .map_err(internal)?;
    // Hold the path until the body drops its read handle, including on Windows.
    let lease = resource.clone();
    let stream = ReaderStream::new(file).map(move |chunk| {
        let _lease = &lease;
        chunk
    });
    let mut response = Body::from_stream(stream).into_response();
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

    #[cfg(unix)]
    #[tokio::test]
    async fn test_offline_freezes_many_resources_under_low_descriptor_limit() {
        const CHILD_MARKER: &str = "OBRAIN_TEST_OFFLINE_LOW_DESCRIPTOR_LIMIT";
        if std::env::var_os(CHILD_MARKER).is_none() {
            // The soft limit must only affect this child; changing it in the
            // main harness would also affect unrelated concurrently running tests.
            let output = std::process::Command::new("/bin/sh")
                .args([
                    "-c",
                    "ulimit -n 64 && exec \"$@\"",
                    "offline-low-descriptor-test",
                ])
                .arg(std::env::current_exe().unwrap())
                .args([
                    "test_offline_freezes_many_resources_under_low_descriptor_limit",
                    "--nocapture",
                ])
                .env(CHILD_MARKER, "1")
                .output()
                .unwrap();
            assert!(
                output.status.success()
                    && String::from_utf8_lossy(&output.stdout).contains("1 passed"),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }

        let (ctx, dir, _) = AppContext::for_test();
        let app = crate::api::router::create_router(ctx);
        let mut first = None;
        for index in 0..80 {
            let path = dir.path().join(format!("chapter-{index}.pdf"));
            std::fs::write(&path, format!("chapter {index}")).unwrap();
            let source = format!(
                "/v1/reader/raw?path={}",
                urlencoding::encode(&path.to_string_lossy())
            );
            let url = format!("/v1/offline/resource?path={}", urlencoding::encode(&source));
            let (status, manifest) = request(app.clone(), "GET", &url, json!({})).await;
            assert_eq!(status, StatusCode::OK, "asset {index}: {manifest}");
            if first.is_none() {
                first = manifest["download_path"].as_str().map(str::to_owned);
            }
        }
        let response = app
            .oneshot(Request::get(first.unwrap()).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            to_bytes(response.into_body(), 1024).await.unwrap(),
            &b"chapter 0"[..]
        );
    }

    #[tokio::test]
    async fn test_offline_abandoned_snapshot_directories_do_not_block_new_exports() {
        let (ctx, _dir, _) = AppContext::for_test();
        crate::tools::handlers::register_all_tools(&ctx.tool_registry, ctx.clone()).await;
        let export_root = ctx.db.database_path().with_extension("offline-exports");
        for index in 0..3 {
            std::fs::create_dir_all(export_root.join(format!("snapshot-abandoned-{index}")))
                .unwrap();
        }
        let app = crate::api::router::create_router(ctx);
        let snapshot = mobile_snapshot(&app, json!({})).await;
        assert!(snapshot["versions"]["overview"].is_string(), "{snapshot}");
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
            group: None,
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
        assert!(first["capabilities"]
            .as_array()
            .unwrap()
            .contains(&json!("mobile_reader_depth")));
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

    async fn mobile_snapshot(app: &axum::Router, mut options: Value) -> Value {
        options["profile"] = json!("mobile");
        let (status, snapshot) =
            request(app.clone(), "POST", "/v1/offline/snapshots", options).await;
        assert_eq!(status, StatusCode::OK, "{snapshot}");
        snapshot
    }

    async fn snapshot_contents(app: &axum::Router, snapshot: &Value) -> (Value, Value) {
        let id = snapshot["snapshot_id"].as_str().unwrap();
        let path = format!("/v1/offline/snapshots/{id}");
        let (_, page) = request(app.clone(), "GET", &format!("{path}?limit=100"), json!({})).await;
        let (_, assets) = request(
            app.clone(),
            "GET",
            &format!("{path}/resources?limit=100"),
            json!({}),
        )
        .await;
        let (_, released) =
            request(app.clone(), "POST", &format!("{path}/release"), json!({})).await;
        assert_eq!(page["complete"], true);
        assert_eq!(assets["complete"], true);
        assert_eq!(released["released"], true);
        (page["records"].clone(), assets["resources"].clone())
    }

    #[tokio::test]
    async fn test_mobile_snapshot_sends_only_changed_scopes_and_available_thumbnails() {
        let (ctx, _dir, _) = AppContext::for_test();
        crate::tools::handlers::register_all_tools(&ctx.tool_registry, ctx.clone()).await;
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(10, 10)
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let photo = ctx
            .memo_manager
            .images
            .upload(&png.into_inner())
            .await
            .unwrap();
        let images = json!([photo, "Timeline/images/missing.png"]).to_string();
        let original_content = "A".repeat(250);
        // A previously imported memo can have an unavailable original attachment.
        ctx.db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO memos (id, content, timestamp, date, file_path, images) VALUES ('memo', ?1, '2026-01-01T00:00:00Z', '2026-01-01', '', ?2)",
                [&original_content, &images],
            )?;
            Ok(())
        }).unwrap();
        let app = crate::api::router::create_router(ctx.clone());
        let first = mobile_snapshot(&app, json!({})).await;
        let (records, assets) = snapshot_contents(&app, &first).await;
        assert_eq!(first["mode"], "mobile_incremental");
        assert_eq!(first["versions"].as_object().unwrap().len(), 3, "{first}");
        assert_eq!(assets.as_array().unwrap().len(), 1, "{assets}");
        assert!(assets[0]["request_path"]
            .as_str()
            .unwrap()
            .starts_with("/v1/timeline/thumbnails/"));
        assert_eq!(assets[0]["mime_type"], "image/jpeg");
        assert!(records.as_array().unwrap().iter().all(|r| {
            [
                "home_overview",
                "get_reader_books",
                "get_reader_history",
                "list_tasks",
                "list_book_knowledge_bases",
                "list_knowledge_tasks",
                "get_memo",
            ]
            .iter()
            .any(|tool| r["tool"] == *tool)
        }));

        let unchanged = mobile_snapshot(&app, json!({"known_versions":first["versions"]})).await;
        assert_eq!(
            unchanged["unchanged_scopes"],
            json!(["overview", "timeline", "tasks"])
        );
        assert_eq!(unchanged["record_count"], 0);
        assert_eq!(unchanged["resource_count"], 0);
        snapshot_contents(&app, &unchanged).await;

        let updated_content = format!("{original_content} 完整的小记正文");
        ctx.db
            .with_connection(|conn| {
                // Editing beyond the dashboard excerpt must only invalidate the timeline.
                conn.execute(
                    "UPDATE memos SET content=?1, revision=revision+1 WHERE id='memo'",
                    [&updated_content],
                )?;
                Ok(())
            })
            .unwrap();
        let changed = mobile_snapshot(&app, json!({"known_versions":first["versions"]})).await;
        let (records, assets) = snapshot_contents(&app, &changed).await;
        assert_eq!(changed["unchanged_scopes"], json!(["overview", "tasks"]));
        assert!(changed["versions"]["timeline"].is_string(), "{changed}");
        assert_ne!(
            changed["versions"]["timeline"],
            first["versions"]["timeline"]
        );
        assert_eq!(records.as_array().unwrap().len(), 1, "{records}");
        assert_eq!(records[0]["tool"], "get_memo");
        assert_eq!(records[0]["result"]["content"], updated_content);
        assert_eq!(assets.as_array().unwrap().len(), 1, "{assets}");
        assert!(assets[0]["request_path"]
            .as_str()
            .unwrap()
            .starts_with("/v1/timeline/thumbnails/"));
        assert_eq!(assets[0]["mime_type"], "image/jpeg");
        assert!(changed["coverage"]
            .as_array()
            .unwrap()
            .iter()
            .any(|scope| scope["scope"] == "timeline_assets" && scope["complete"] == false));
        let still_missing =
            mobile_snapshot(&app, json!({"known_versions":changed["versions"]})).await;
        assert_eq!(still_missing["record_count"], 0);
        assert_eq!(
            still_missing["unchanged_scopes"],
            json!(["overview", "timeline", "tasks"])
        );
        snapshot_contents(&app, &still_missing).await;
    }

    #[tokio::test]
    async fn test_mobile_reader_exports_one_complete_tree_through_fifteen_levels() {
        let (ctx, dir, _) = AppContext::for_test();
        crate::tools::handlers::register_all_tools(&ctx.tool_registry, ctx.clone()).await;
        let folder = dir.path().join("deep-book");
        let shallow = (1..=10).fold(folder.clone(), |path, level| {
            path.join(format!("part-{level}"))
        });
        std::fs::create_dir_all(&shallow).unwrap();
        std::fs::write(shallow.join("shallow.md"), "# Existing mobile article").unwrap();
        ctx.db.with_connection(|conn| {
            conn.execute("INSERT INTO reader_books(id,path,kind,name,added_at) VALUES('deep',?1,'folder','Deep book',1)", [folder.to_str().unwrap()])?;
            Ok(())
        }).unwrap();
        let app = crate::api::router::create_router(ctx);

        let old_mobile = mobile_snapshot(&app, json!({})).await;
        let (records, _) = snapshot_contents(&app, &old_mobile).await;
        assert!(records
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["tool"] == "list_local_dir"
                && r["arguments"] == json!({"path":folder,"depth":10})));

        let deepest = (11..=15).fold(shallow, |path, level| path.join(format!("part-{level}")));
        std::fs::create_dir_all(&deepest).unwrap();
        let article = deepest.join("article.md");
        std::fs::write(&article, "# Deep article").unwrap();

        let overview = mobile_snapshot(&app, json!({"reader_depth":15})).await;
        let (records, _) = snapshot_contents(&app, &overview).await;
        let directory = records
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["tool"] == "list_local_dir" && r["arguments"]["path"] == json!(folder))
            .expect("automatic snapshot contains the deep book directory");
        assert_eq!(directory["arguments"]["depth"], 15);
        let mut entries = &directory["result"]["entries"];
        for _ in 0..15 {
            entries = &entries[0]["children"];
        }
        assert_eq!(entries[0]["path"], json!(article));

        let selection = json!({"include_resources":true,"book_ids":["deep"],"reader_depth":15});
        let selected = mobile_snapshot(&app, selection.clone()).await;
        let (records, _) = snapshot_contents(&app, &selected).await;
        assert!(
            selected["versions"]["reader:deep"].is_string(),
            "{selected}"
        );
        let directories: Vec<_> = records
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["tool"] == "list_local_dir")
            .collect();
        assert_eq!(directories, vec![directory]);
        assert!(records
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["tool"] == "read_local_file" && r["arguments"]["path"] == json!(article)));

        let (_, legacy) = request(
            app.clone(),
            "POST",
            "/v1/offline/snapshots",
            json!({"book_ids":["deep"]}),
        )
        .await;
        assert!(legacy["coverage"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["scope"] == "reader_metadata:deep"
                && c["errors"][0]
                    .as_str()
                    .is_some_and(|message| message.contains("10层"))));
        snapshot_contents(&app, &legacy).await;

        std::fs::create_dir(deepest.join("part-16")).unwrap();
        let over_limit = mobile_snapshot(&app, selection).await;
        assert!(
            over_limit["versions"]["reader:deep"].is_null(),
            "{over_limit}"
        );
        assert!(over_limit["coverage"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["scope"] == "reader_metadata:deep"
                && c["errors"][0]
                    .as_str()
                    .is_some_and(|message| message.contains("15层"))));
        assert!(over_limit["coverage"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["scope"] == "reader:deep" && c["complete"] == false));
    }

    #[tokio::test]
    async fn test_mobile_reader_selection_versions_include_bodies_and_dependencies() {
        let (ctx, dir, _) = AppContext::for_test();
        crate::tools::handlers::register_all_tools(&ctx.tool_registry, ctx.clone()).await;
        let folder = dir.path().join("folder");
        std::fs::create_dir(&folder).unwrap();
        let chapter = folder.join("chapter.md");
        let image = folder.join("picture.png");
        std::fs::write(&chapter, "# Selected\n![picture](picture.png)").unwrap();
        std::fs::write(&image, b"first").unwrap();
        std::fs::write(folder.join("another.pdf"), b"not selected").unwrap();
        ctx.db.with_connection(|conn| {
            conn.execute("INSERT INTO reader_books(id,path,kind,name,added_at) VALUES('folder',?1,'folder','Book',1)", [folder.to_str().unwrap()])?;
            Ok(())
        }).unwrap();
        let app = crate::api::router::create_router(ctx);
        let mut options =
            json!({"include_resources":true,"reader_files":[{"book_id":"folder","path":chapter}]});
        let first = mobile_snapshot(&app, options.clone()).await;
        let (records, assets) = snapshot_contents(&app, &first).await;
        assert_eq!(first["mode"], "mobile_reader");
        assert!(first["versions"]["reader:folder"].is_string(), "{first}");
        assert!(records
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["tool"] == "read_local_file" && r["arguments"]["path"] == json!(chapter)));
        assert_eq!(assets.as_array().unwrap().len(), 1, "{assets}");
        assert!(assets[0]["request_path"]
            .as_str()
            .unwrap()
            .ends_with("picture.png"));

        options["known_versions"] = first["versions"].clone();
        let unchanged = mobile_snapshot(&app, options.clone()).await;
        assert_eq!(unchanged["unchanged_scopes"], json!(["reader:folder"]));
        assert_eq!(unchanged["record_count"], 0);
        assert_eq!(unchanged["resource_count"], 0);
        snapshot_contents(&app, &unchanged).await;

        std::fs::write(&image, b"other").unwrap();
        let changed = mobile_snapshot(&app, options.clone()).await;
        let (_, new_assets) = snapshot_contents(&app, &changed).await;
        assert_ne!(
            changed["versions"]["reader:folder"],
            first["versions"]["reader:folder"]
        );
        assert_ne!(assets[0]["sha256"], new_assets[0]["sha256"]);
        std::fs::remove_file(&image).unwrap();
        let missing_image = mobile_snapshot(&app, options.clone()).await;
        let (records, assets) = snapshot_contents(&app, &missing_image).await;
        assert_eq!(assets, json!([]));
        assert!(records
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["tool"] == "read_local_file" && r["arguments"]["path"] == json!(chapter)));
        assert_ne!(
            missing_image["versions"]["reader:folder"],
            changed["versions"]["reader:folder"]
        );
        assert!(
            missing_image["versions"]["reader:folder"].is_string(),
            "{missing_image}"
        );
        assert!(missing_image["coverage"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["scope"] == "reader_assets:folder" && c["complete"] == false));
        assert!(missing_image["coverage"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["scope"] == "reader:folder" && c["complete"] == true));

        options["known_versions"] = missing_image["versions"].clone();
        let unchanged_missing = mobile_snapshot(&app, options.clone()).await;
        assert_eq!(
            unchanged_missing["unchanged_scopes"],
            json!(["reader:folder"])
        );
        assert!(unchanged_missing["coverage"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["scope"] == "reader_assets:folder" && c["complete"] == false));
        snapshot_contents(&app, &unchanged_missing).await;

        std::fs::write(&image, b"other").unwrap();
        let repaired = mobile_snapshot(&app, options.clone()).await;
        let (_, repaired_assets) = snapshot_contents(&app, &repaired).await;
        assert_eq!(repaired_assets[0]["sha256"], new_assets[0]["sha256"]);
        assert_ne!(
            repaired["versions"]["reader:folder"],
            missing_image["versions"]["reader:folder"]
        );

        std::fs::remove_file(&chapter).unwrap();
        let missing_body = mobile_snapshot(&app, options).await;
        assert_eq!(missing_body["versions"], json!({}));
        assert!(missing_body["coverage"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["scope"] == "reader:folder" && c["complete"] == false));
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
