use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::Utc;
use rusqlite::{params, Connection, Row};
use serde::Serialize;
use serde_json::{json, Value};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::error::BrainError;
use crate::infra::sqlite_store::SqliteStore;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BookWikiExportFormat {
    Json,
    Markdown,
}

impl BookWikiExportFormat {
    pub fn parse(value: &str) -> Result<Self, BrainError> {
        match value {
            "json" => Ok(Self::Json),
            "markdown" => Ok(Self::Markdown),
            _ => Err(BrainError::KnowledgeValidation(
                "导出格式仅支持 json 或 markdown".to_string(),
            )),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Markdown => "markdown",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ExportedBookWiki {
    pub filename: String,
    pub mime_type: String,
    pub size_bytes: u64,
    #[serde(skip)]
    pub path: PathBuf,
}

struct ExportManifest {
    base_id: String,
    book_name: String,
    book_path: String,
    lifecycle: String,
    compile_mode: String,
    exported_at: String,
}

pub fn export_book_wiki(
    db: &SqliteStore,
    base_id: &str,
    format: BookWikiExportFormat,
) -> Result<ExportedBookWiki, BrainError> {
    let parent = db
        .database_path()
        .parent()
        .unwrap_or_else(|| Path::new("."));
    let export_dir = parent.join("exports");
    fs::create_dir_all(&export_dir)?;
    let public_name = format!(
        "book-wiki-{}-{}.zip",
        format.label(),
        Utc::now().format("%Y%m%d-%H%M%S")
    );
    let path = export_dir.join(format!(
        "{}-{}-{}",
        uuid::Uuid::new_v4().simple(),
        base_id.replace(|character: char| !character.is_ascii_alphanumeric(), "-"),
        public_name
    ));

    db.with_connection(|conn| {
        let manifest = load_manifest(conn, base_id)?;
        let file = File::create(&path)?;
        let mut archive = ZipWriter::new(file);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        match format {
            BookWikiExportFormat::Json => {
                write_json_export(conn, &mut archive, options, &manifest)?
            }
            BookWikiExportFormat::Markdown => {
                write_markdown_export(conn, &mut archive, options, &manifest)?
            }
        }
        archive
            .finish()
            .map_err(|error| BrainError::Internal(format!("Wiki 导出 ZIP 收尾失败: {error}")))?;
        Ok(())
    })?;

    let size_bytes = fs::metadata(&path)?.len();
    prune_exports(&export_dir, 10, &path)?;
    Ok(ExportedBookWiki {
        filename: public_name,
        mime_type: "application/zip".to_string(),
        size_bytes,
        path,
    })
}

fn load_manifest(conn: &Connection, base_id: &str) -> Result<ExportManifest, BrainError> {
    conn.query_row(
        "SELECT kb.id, rb.name, rb.path, kb.lifecycle, kb.compile_mode
         FROM knowledge_bases kb
         JOIN reader_books rb ON rb.id = kb.book_id
         WHERE kb.id = ?1",
        params![base_id],
        |row| {
            Ok(ExportManifest {
                base_id: row.get(0)?,
                book_name: row.get(1)?,
                book_path: row.get(2)?,
                lifecycle: row.get(3)?,
                compile_mode: row.get(4)?,
                exported_at: Utc::now().to_rfc3339(),
            })
        },
    )
    .map_err(|error| match error {
        rusqlite::Error::QueryReturnedNoRows => BrainError::KnowledgeNotFound(base_id.to_string()),
        other => BrainError::Internal(format!("Wiki 导出信息读取失败: {other}")),
    })
}

fn start_file(
    archive: &mut ZipWriter<File>,
    name: &str,
    options: SimpleFileOptions,
) -> Result<(), BrainError> {
    archive
        .start_file(name, options)
        .map_err(|error| BrainError::Internal(format!("Wiki 导出文件创建失败: {error}")))
}

fn write_value(
    archive: &mut ZipWriter<File>,
    value: &Value,
    pretty: bool,
) -> Result<(), BrainError> {
    if pretty {
        serde_json::to_writer_pretty(&mut *archive, value)
    } else {
        serde_json::to_writer(&mut *archive, value)
    }
    .map_err(|error| BrainError::Internal(format!("Wiki 导出 JSON 写入失败: {error}")))?;
    if !pretty {
        archive.write_all(b"\n")?;
    }
    Ok(())
}

fn write_json_export(
    conn: &Connection,
    archive: &mut ZipWriter<File>,
    options: SimpleFileOptions,
    manifest: &ExportManifest,
) -> Result<(), BrainError> {
    start_file(archive, "manifest.json", options)?;
    write_value(
        archive,
        &json!({
            "schema": "obsidianbrain.book-wiki.export.v1",
            "format": "jsonl",
            "knowledge_base_id": manifest.base_id,
            "book_name": manifest.book_name,
            "book_path": manifest.book_path,
            "lifecycle": manifest.lifecycle,
            "compile_mode": manifest.compile_mode,
            "exported_at": manifest.exported_at,
        }),
        true,
    )?;

    write_jsonl_query(
        conn,
        archive,
        options,
        "sources.jsonl",
        "SELECT sd.id, sd.relative_path, sd.title, sd.sync_status, sd.extraction_status,
                sv.id, sv.content_hash, sv.size_bytes, sv.modified_at, sv.created_at
         FROM source_documents sd
         LEFT JOIN source_versions sv ON sv.id = sd.current_version_id
         WHERE sd.knowledge_base_id = ?1 ORDER BY sd.ordinal, sd.relative_path",
        &manifest.base_id,
        |row| {
            Ok(json!({
                "id": row.get::<_, String>(0)?, "relative_path": row.get::<_, String>(1)?,
                "title": row.get::<_, String>(2)?, "sync_status": row.get::<_, String>(3)?,
                "extraction_status": row.get::<_, String>(4)?, "version_id": row.get::<_, Option<String>>(5)?,
                "content_hash": row.get::<_, Option<String>>(6)?, "size_bytes": row.get::<_, Option<i64>>(7)?,
                "modified_at": row.get::<_, Option<String>>(8)?, "version_created_at": row.get::<_, Option<String>>(9)?
            }))
        },
    )?;
    write_jsonl_query(
        conn,
        archive,
        options,
        "source-spans.jsonl",
        "SELECT ss.id, sd.relative_path, ss.ordinal, ss.heading, ss.anchor, ss.line_start,
                ss.line_end, ss.content, ss.content_hash, ss.token_estimate
         FROM source_spans ss
         JOIN source_versions sv ON sv.id = ss.source_version_id
         JOIN source_documents sd ON sd.id = sv.source_document_id
         WHERE ss.knowledge_base_id = ?1 ORDER BY sd.ordinal, ss.ordinal",
        &manifest.base_id,
        |row| {
            Ok(json!({
                "id": row.get::<_, String>(0)?, "source_path": row.get::<_, String>(1)?,
                "ordinal": row.get::<_, i64>(2)?, "heading": row.get::<_, Option<String>>(3)?,
                "anchor": row.get::<_, Option<String>>(4)?, "line_start": row.get::<_, Option<i64>>(5)?,
                "line_end": row.get::<_, Option<i64>>(6)?, "content": row.get::<_, String>(7)?,
                "content_hash": row.get::<_, String>(8)?, "token_estimate": row.get::<_, i64>(9)?
            }))
        },
    )?;
    write_jsonl_query(
        conn,
        archive,
        options,
        "entities.jsonl",
        "SELECT id, entry_type, slug, title, aliases_json, summary, content_md, status,
                confidence, edit_policy, revision, created_at, updated_at
         FROM knowledge_entries WHERE knowledge_base_id = ?1 ORDER BY entry_type, title",
        &manifest.base_id,
        |row| {
            Ok(json!({
                "id": row.get::<_, String>(0)?, "entry_type": row.get::<_, String>(1)?,
                "slug": row.get::<_, String>(2)?, "title": row.get::<_, String>(3)?,
                "aliases": parse_json(row.get::<_, String>(4)?, json!([])),
                "summary": row.get::<_, String>(5)?, "content_md": row.get::<_, String>(6)?,
                "status": row.get::<_, String>(7)?, "confidence": row.get::<_, Option<f64>>(8)?,
                "edit_policy": row.get::<_, String>(9)?, "revision": row.get::<_, i64>(10)?,
                "created_at": row.get::<_, String>(11)?, "updated_at": row.get::<_, String>(12)?
            }))
        },
    )?;
    write_jsonl_query(
        conn,
        archive,
        options,
        "claims.jsonl",
        "SELECT id, entry_id, subject_entry_id, predicate, object_entry_id, object_text,
                claim_text, confidence, verification_status, revision, created_at, updated_at
         FROM knowledge_claims WHERE knowledge_base_id = ?1 ORDER BY entry_id, created_at",
        &manifest.base_id,
        |row| {
            Ok(json!({
                "id": row.get::<_, String>(0)?, "entry_id": row.get::<_, String>(1)?,
                "subject_entry_id": row.get::<_, Option<String>>(2)?, "predicate": row.get::<_, String>(3)?,
                "object_entry_id": row.get::<_, Option<String>>(4)?, "object_text": row.get::<_, Option<String>>(5)?,
                "claim_text": row.get::<_, String>(6)?, "confidence": row.get::<_, Option<f64>>(7)?,
                "verification_status": row.get::<_, String>(8)?, "revision": row.get::<_, i64>(9)?,
                "created_at": row.get::<_, String>(10)?, "updated_at": row.get::<_, String>(11)?
            }))
        },
    )?;
    write_jsonl_query(
        conn,
        archive,
        options,
        "relations.jsonl",
        "SELECT id, from_entry_id, to_entry_id, relation_type, strength, evidence, revision,
                created_at, updated_at FROM knowledge_relations
         WHERE knowledge_base_id = ?1 ORDER BY from_entry_id, relation_type, to_entry_id",
        &manifest.base_id,
        |row| {
            Ok(json!({
                "id": row.get::<_, String>(0)?, "from_entry_id": row.get::<_, String>(1)?,
                "to_entry_id": row.get::<_, String>(2)?, "relation_type": row.get::<_, String>(3)?,
                "strength": row.get::<_, Option<f64>>(4)?, "evidence": row.get::<_, Option<String>>(5)?,
                "revision": row.get::<_, i64>(6)?, "created_at": row.get::<_, String>(7)?,
                "updated_at": row.get::<_, String>(8)?
            }))
        },
    )?;
    write_jsonl_query(
        conn,
        archive,
        options,
        "citations.jsonl",
        "SELECT kc.id, kc.entry_id, kc.claim_id, sd.relative_path, ss.heading, ss.line_start,
                ss.line_end, kc.quote_text, ss.content_hash
         FROM knowledge_citations kc
         JOIN source_spans ss ON ss.id = kc.source_span_id
         JOIN source_versions sv ON sv.id = ss.source_version_id
         JOIN source_documents sd ON sd.id = sv.source_document_id
         WHERE kc.knowledge_base_id = ?1 ORDER BY kc.entry_id, kc.created_at",
        &manifest.base_id,
        |row| {
            Ok(json!({
                "id": row.get::<_, String>(0)?, "entry_id": row.get::<_, Option<String>>(1)?,
                "claim_id": row.get::<_, Option<String>>(2)?, "source_path": row.get::<_, String>(3)?,
                "heading": row.get::<_, Option<String>>(4)?, "line_start": row.get::<_, Option<i64>>(5)?,
                "line_end": row.get::<_, Option<i64>>(6)?, "quote_text": row.get::<_, Option<String>>(7)?,
                "source_content_hash": row.get::<_, String>(8)?
            }))
        },
    )?;
    write_jsonl_query(
        conn,
        archive,
        options,
        "conversations.jsonl",
        "SELECT kc.id, kc.title, km.id, km.ordinal, km.role, km.content, km.run_id, km.created_at
         FROM knowledge_conversation_scopes kcs
         JOIN knowledge_conversations kc ON kc.id = kcs.conversation_id
         JOIN knowledge_messages km ON km.conversation_id = kc.id
         WHERE kcs.knowledge_base_id = ?1 ORDER BY kc.updated_at DESC, km.ordinal",
        &manifest.base_id,
        |row| {
            Ok(json!({
                "conversation_id": row.get::<_, String>(0)?, "conversation_title": row.get::<_, String>(1)?,
                "message_id": row.get::<_, String>(2)?, "ordinal": row.get::<_, i64>(3)?,
                "role": row.get::<_, String>(4)?, "content": row.get::<_, String>(5)?,
                "run_id": row.get::<_, Option<String>>(6)?, "created_at": row.get::<_, String>(7)?
            }))
        },
    )?;
    write_jsonl_query(
        conn,
        archive,
        options,
        "research-tasks.jsonl",
        "SELECT id, title, description, task_type, status, result_summary, deliverable_type,
                artifact_state, knowledge_change_state, cancel_requested, created_at, updated_at
         FROM knowledge_tasks WHERE knowledge_base_id = ?1 ORDER BY updated_at DESC",
        &manifest.base_id,
        |row| {
            Ok(json!({
                "id": row.get::<_, String>(0)?, "title": row.get::<_, String>(1)?,
                "description": row.get::<_, String>(2)?, "task_type": row.get::<_, String>(3)?,
                "status": row.get::<_, String>(4)?, "result_summary": row.get::<_, String>(5)?,
                "deliverable_type": row.get::<_, String>(6)?, "artifact_state": row.get::<_, String>(7)?,
                "knowledge_change_state": row.get::<_, String>(8)?, "cancel_requested": row.get::<_, bool>(9)?,
                "created_at": row.get::<_, String>(10)?, "updated_at": row.get::<_, String>(11)?
            }))
        },
    )?;
    Ok(())
}

fn write_jsonl_query<F>(
    conn: &Connection,
    archive: &mut ZipWriter<File>,
    options: SimpleFileOptions,
    filename: &str,
    sql: &str,
    base_id: &str,
    mut mapper: F,
) -> Result<(), BrainError>
where
    F: FnMut(&Row<'_>) -> rusqlite::Result<Value>,
{
    start_file(archive, filename, options)?;
    let mut statement = conn.prepare(sql)?;
    let rows = statement.query_map(params![base_id], |row| mapper(row))?;
    for row in rows {
        write_value(archive, &row?, false)?;
    }
    Ok(())
}

fn write_markdown_export(
    conn: &Connection,
    archive: &mut ZipWriter<File>,
    options: SimpleFileOptions,
    manifest: &ExportManifest,
) -> Result<(), BrainError> {
    let counts = conn.query_row(
        "SELECT
            (SELECT COUNT(*) FROM source_documents WHERE knowledge_base_id = ?1),
            (SELECT COUNT(*) FROM knowledge_entries WHERE knowledge_base_id = ?1 AND status <> 'archived'),
            (SELECT COUNT(*) FROM knowledge_claims WHERE knowledge_base_id = ?1),
            (SELECT COUNT(*) FROM knowledge_relations WHERE knowledge_base_id = ?1)",
        params![manifest.base_id],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?, row.get::<_, i64>(3)?)),
    )?;
    start_file(archive, "README.md", options)?;
    writeln!(archive, "# {}\n", manifest.book_name)?;
    writeln!(
        archive,
        "> 由 ObsidianBrain 于 {} 导出。数据库仍是事实来源。\n",
        manifest.exported_at
    )?;
    writeln!(archive, "- 原书路径：`{}`", manifest.book_path)?;
    writeln!(archive, "- 知识库状态：{}", manifest.lifecycle)?;
    writeln!(archive, "- 建库模式：{}", manifest.compile_mode)?;
    writeln!(
        archive,
        "- 来源：{} · 实体：{} · 论断：{} · 关系：{}\n",
        counts.0, counts.1, counts.2, counts.3
    )?;
    writeln!(archive, "## 浏览\n\n- [知识实体](entities/INDEX.md)\n- [来源目录](sources.md)\n- [问答记录](reports/conversations.md)\n- [研究任务](reports/research-tasks.md)")?;

    let mut statement = conn.prepare(
        "SELECT id, entry_type, title, summary, content_md, status, confidence, revision
         FROM knowledge_entries WHERE knowledge_base_id = ?1 AND status <> 'archived'
         ORDER BY entry_type, title",
    )?;
    let entries = statement
        .query_map(params![manifest.base_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, Option<f64>>(6)?,
                row.get::<_, i64>(7)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut entity_index = String::from("# 知识实体\n\n");
    for (id, entry_type, title, ..) in &entries {
        let entity_file = format!("{}.md", safe_component(id));
        entity_index.push_str(&format!("- [{title}]({entity_file}) · `{entry_type}`\n"));
    }
    start_file(archive, "entities/INDEX.md", options)?;
    archive.write_all(entity_index.as_bytes())?;
    for (id, entry_type, title, summary, content, status, confidence, revision) in entries {
        let entity_file = format!("{}.md", safe_component(&id));
        start_file(archive, &format!("entities/{entity_file}"), options)?;
        writeln!(archive, "# {title}\n")?;
        writeln!(archive, "- 类型：`{entry_type}`")?;
        writeln!(archive, "- 状态：`{status}` · Revision {revision}")?;
        if let Some(confidence) = confidence {
            writeln!(archive, "- 置信度：{confidence:.2}")?;
        }
        if !summary.trim().is_empty() {
            writeln!(archive, "\n> {}\n", summary.replace('\n', " "))?;
        }
        writeln!(archive, "\n{content}\n")?;
        write_entry_evidence(conn, archive, &id)?;
    }

    write_sources_markdown(conn, archive, options, &manifest.base_id)?;
    write_conversations_markdown(conn, archive, options, &manifest.base_id)?;
    write_tasks_markdown(conn, archive, options, &manifest.base_id)?;
    Ok(())
}

fn write_entry_evidence(
    conn: &Connection,
    archive: &mut ZipWriter<File>,
    entry_id: &str,
) -> Result<(), BrainError> {
    let mut statement = conn.prepare(
        "SELECT sd.relative_path, ss.heading, ss.line_start, ss.line_end, kc.quote_text
         FROM knowledge_citations kc
         JOIN source_spans ss ON ss.id = kc.source_span_id
         JOIN source_versions sv ON sv.id = ss.source_version_id
         JOIN source_documents sd ON sd.id = sv.source_document_id
         WHERE kc.entry_id = ?1 ORDER BY kc.created_at",
    )?;
    let citations = statement
        .query_map(params![entry_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<i64>>(2)?,
                row.get::<_, Option<i64>>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    if !citations.is_empty() {
        writeln!(archive, "## 来源引用\n")?;
        for (path, heading, line_start, line_end, quote) in citations {
            let location = match (line_start, line_end) {
                (Some(start), Some(end)) => format!("L{start}–L{end}"),
                _ => heading.unwrap_or_default(),
            };
            writeln!(
                archive,
                "- `{path}`{}",
                if location.is_empty() {
                    String::new()
                } else {
                    format!(" · {location}")
                }
            )?;
            if let Some(quote) = quote.filter(|value| !value.trim().is_empty()) {
                writeln!(archive, "  > {}", quote.replace('\n', " "))?;
            }
        }
    }
    let mut relations_statement = conn.prepare(
        "SELECT kr.relation_type, ke.title, kr.evidence
         FROM knowledge_relations kr JOIN knowledge_entries ke ON ke.id = kr.to_entry_id
         WHERE kr.from_entry_id = ?1 ORDER BY kr.relation_type, ke.title",
    )?;
    let relations = relations_statement
        .query_map(params![entry_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    if !relations.is_empty() {
        writeln!(archive, "\n## 关系\n")?;
        for (relation_type, title, evidence) in relations {
            writeln!(
                archive,
                "- **{relation_type}** → {title}{}",
                evidence
                    .filter(|value| !value.is_empty())
                    .map(|value| format!("：{value}"))
                    .unwrap_or_default()
            )?;
        }
    }
    Ok(())
}

fn write_sources_markdown(
    conn: &Connection,
    archive: &mut ZipWriter<File>,
    options: SimpleFileOptions,
    base_id: &str,
) -> Result<(), BrainError> {
    start_file(archive, "sources.md", options)?;
    writeln!(archive, "# 来源目录\n")?;
    let mut statement = conn.prepare("SELECT relative_path, title, sync_status, extraction_status FROM source_documents WHERE knowledge_base_id = ?1 ORDER BY ordinal, relative_path")?;
    for row in statement.query_map(params![base_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
        ))
    })? {
        let (path, title, sync, extraction) = row?;
        writeln!(archive, "- **{title}** · `{path}` · {sync}/{extraction}")?;
    }
    Ok(())
}

fn write_conversations_markdown(
    conn: &Connection,
    archive: &mut ZipWriter<File>,
    options: SimpleFileOptions,
    base_id: &str,
) -> Result<(), BrainError> {
    start_file(archive, "reports/conversations.md", options)?;
    writeln!(archive, "# 问答记录\n")?;
    let mut statement = conn.prepare(
        "SELECT kc.id, kc.title, km.role, km.content, km.created_at
         FROM knowledge_conversation_scopes kcs
         JOIN knowledge_conversations kc ON kc.id = kcs.conversation_id
         LEFT JOIN knowledge_messages km ON km.conversation_id = kc.id
         WHERE kcs.knowledge_base_id = ?1 ORDER BY kc.updated_at DESC, km.ordinal",
    )?;
    let mut last_id = String::new();
    for row in statement.query_map(params![base_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
        ))
    })? {
        let (id, title, role, content, created_at) = row?;
        if id != last_id {
            writeln!(archive, "\n## {title}\n")?;
            last_id = id;
        }
        if let (Some(role), Some(content)) = (role, content) {
            writeln!(
                archive,
                "### {} · {}\n\n{}\n",
                if role == "user" { "提问" } else { "回答" },
                created_at.unwrap_or_default(),
                content
            )?;
        }
    }
    Ok(())
}

fn write_tasks_markdown(
    conn: &Connection,
    archive: &mut ZipWriter<File>,
    options: SimpleFileOptions,
    base_id: &str,
) -> Result<(), BrainError> {
    start_file(archive, "reports/research-tasks.md", options)?;
    writeln!(archive, "# 研究任务\n")?;
    let mut statement = conn.prepare("SELECT title, description, status, deliverable_type, result_summary, updated_at FROM knowledge_tasks WHERE knowledge_base_id = ?1 ORDER BY updated_at DESC")?;
    for row in statement.query_map(params![base_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, String>(5)?,
        ))
    })? {
        let (title, description, status, deliverable, result, updated_at) = row?;
        writeln!(
            archive,
            "## {title}\n\n- 状态：`{status}` · 交付物：`{deliverable}` · 更新：{updated_at}\n"
        )?;
        if !description.is_empty() {
            writeln!(archive, "{description}\n")?;
        }
        if !result.is_empty() {
            writeln!(archive, "### 结果\n\n{result}\n")?;
        }
    }
    Ok(())
}

fn safe_component(value: &str) -> String {
    let safe = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    if safe.is_empty() {
        "entity".to_string()
    } else {
        safe
    }
}

fn parse_json(raw: String, fallback: Value) -> Value {
    serde_json::from_str(&raw).unwrap_or(fallback)
}

fn prune_exports(directory: &Path, retention: usize, preserve: &Path) -> Result<(), BrainError> {
    let mut paths = fs::read_dir(directory)?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_file())
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    paths.sort_by(|left, right| right.file_name().cmp(&left.file_name()));
    for path in paths.into_iter().skip(retention.max(1)) {
        if path != preserve {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::sync::Arc;
    use tempfile::TempDir;
    use zip::ZipArchive;

    use crate::infra::book_wiki_store::BookWikiStore;
    use crate::models::book_wiki::{BookKind, ReaderBook};

    fn prepared_store() -> (TempDir, Arc<SqliteStore>, String) {
        let dir = TempDir::new().unwrap();
        let db = Arc::new(SqliteStore::new(&dir.path().join("export.db")).unwrap());
        let store = BookWikiStore::new(db.clone());
        store
            .save_reader_books(&[ReaderBook {
                id: "export-book".to_string(),
                path: dir.path().join("book").to_string_lossy().to_string(),
                kind: BookKind::Folder,
                name: "导出测试".to_string(),
                description: String::new(),
                category: String::new(),
                added_at: 1,
                progress: None,
            }])
            .unwrap();
        let base = store.initialize_base("export-book").unwrap();
        (dir, db, base.id)
    }

    #[test]
    fn test_json_export_contains_structured_knowledge_files() {
        let (_dir, db, base_id) = prepared_store();
        let exported = export_book_wiki(&db, &base_id, BookWikiExportFormat::Json).unwrap();
        let file = File::open(exported.path).unwrap();
        let mut archive = ZipArchive::new(file).unwrap();
        for expected in [
            "manifest.json",
            "sources.jsonl",
            "source-spans.jsonl",
            "entities.jsonl",
            "claims.jsonl",
            "relations.jsonl",
            "citations.jsonl",
            "conversations.jsonl",
            "research-tasks.jsonl",
        ] {
            assert!(archive.by_name(expected).is_ok(), "missing {expected}");
        }
    }

    #[test]
    fn test_markdown_export_is_browsable_and_includes_reports() {
        let (_dir, db, base_id) = prepared_store();
        let exported = export_book_wiki(&db, &base_id, BookWikiExportFormat::Markdown).unwrap();
        let file = File::open(exported.path).unwrap();
        let mut archive = ZipArchive::new(file).unwrap();
        for expected in [
            "README.md",
            "entities/INDEX.md",
            "sources.md",
            "reports/conversations.md",
            "reports/research-tasks.md",
        ] {
            assert!(archive.by_name(expected).is_ok(), "missing {expected}");
        }
        let mut readme = String::new();
        archive
            .by_name("README.md")
            .unwrap()
            .read_to_string(&mut readme)
            .unwrap();
        assert!(readme.contains("导出测试"));
    }
}
