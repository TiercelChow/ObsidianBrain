// DB functions naturally take many parameters and return complex rusqlite types.
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, DatabaseName, OpenFlags, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::error::BrainError;

/// SQLite metadata store with WAL mode and versioned migrations.
pub struct SqliteStore {
    conn: Arc<Mutex<Connection>>,
    db_path: PathBuf,
    backup_retention: usize,
}

const DEFAULT_BACKUP_RETENTION: usize = 7;
const MANAGED_BACKUP_PREFIX: &str = "brain-";
const MANAGED_BACKUP_EXTENSION: &str = "sqlite3";

/// A consistent SQLite snapshot managed alongside the live database.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DatabaseBackupSummary {
    pub filename: String,
    pub reason: String,
    pub created_at: String,
    pub size_bytes: u64,
    #[serde(skip)]
    pub path: PathBuf,
}

/// Results from opening and checking a database before or after restoration.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DatabaseValidationReport {
    pub integrity_ok: bool,
    pub integrity_message: String,
    pub foreign_key_violations: u64,
    pub migration_version: u32,
}

#[allow(dead_code)] // Internal helper for migrations
struct Migration {
    version: u32,
    description: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        description: "code_repos + note_repo_links",
        sql: include_str!("../../migrations/001_code_repos.sql"),
    },
    Migration {
        version: 2,
        description: "radar_items",
        sql: include_str!("../../migrations/002_radar_items.sql"),
    },
    Migration {
        version: 3,
        description: "inspiration_history",
        sql: include_str!("../../migrations/003_inspiration.sql"),
    },
    Migration {
        version: 4,
        description: "timeline_events",
        sql: include_str!("../../migrations/004_timeline.sql"),
    },
    Migration {
        version: 5,
        description: "app_state",
        sql: include_str!("../../migrations/005_app_state.sql"),
    },
    Migration {
        version: 6,
        description: "inspiration_history (Phase 3)",
        sql: include_str!("../../migrations/006_inspiration_history.sql"),
    },
    Migration {
        version: 7,
        description: "radar_items (Phase 3)",
        sql: include_str!("../../migrations/007_radar_items.sql"),
    },
    Migration {
        version: 8,
        description: "memos (Time Machine)",
        sql: include_str!("../../migrations/008_memos.sql"),
    },
    Migration {
        version: 9,
        description: "personal task management",
        sql: include_str!("../../migrations/009_tasks.sql"),
    },
    Migration {
        version: 10,
        description: "remove task sync queue and sync_error marker",
        sql: include_str!("../../migrations/010_remove_task_sync.sql"),
    },
    Migration {
        version: 11,
        description: "rebuild radar_items with source_name",
        sql: include_str!("../../migrations/011_radar_items_source_name.sql"),
    },
    Migration {
        version: 12,
        description: "database-native per-book knowledge bases",
        sql: include_str!("../../migrations/012_book_wiki.sql"),
    },
    Migration {
        version: 13,
        description: "DeepSeek Harness ACP runtime command",
        sql: include_str!("../../migrations/013_deepseek_harness_acp.sql"),
    },
    Migration {
        version: 14,
        description: "persistent knowledge conversations and messages",
        sql: include_str!("../../migrations/014_knowledge_conversations.sql"),
    },
    Migration {
        version: 15,
        description: "auditable agent token usage",
        sql: include_str!("../../migrations/015_agent_token_usage.sql"),
    },
    Migration {
        version: 16,
        description: "safe source versions and separate wiki compilation state",
        sql: include_str!("../../migrations/016_book_wiki_source_safety.sql"),
    },
    Migration {
        version: 17,
        description: "versioned wiki skills and durable agent run events",
        sql: include_str!("../../migrations/017_wiki_skills_and_run_events.sql"),
    },
    Migration {
        version: 18,
        description: "semantic wiki change sets, entry versions, and artifacts",
        sql: include_str!("../../migrations/018_semantic_wiki_changes_and_artifacts.sql"),
    },
    Migration {
        version: 19,
        description: "incremental semantic compile checkpoints and ranked FTS",
        sql: include_str!("../../migrations/019_incremental_compile_and_fts.sql"),
    },
    Migration {
        version: 20,
        description: "short-lived scoped capabilities for agent knowledge tools",
        sql: include_str!("../../migrations/020_agent_run_capabilities.sql"),
    },
    Migration {
        version: 21,
        description: "leased knowledge task queue with retry backoff",
        sql: include_str!("../../migrations/021_knowledge_task_leases.sql"),
    },
    Migration {
        version: 22,
        description: "per-task external research grants and safe built-in skills",
        sql: include_str!("../../migrations/022_external_research_and_safe_skills.sql"),
    },
    Migration {
        version: 23,
        description: "background semantic compilation progress and recovery",
        sql: include_str!("../../migrations/023_background_compile_progress.sql"),
    },
    Migration {
        version: 24,
        description: "auditable agent prompts and skill snapshots",
        sql: include_str!("../../migrations/024_agent_run_inspection.sql"),
    },
    Migration {
        version: 25,
        description: "skill-aware semantic compile fingerprints",
        sql: include_str!("../../migrations/025_compile_skill_fingerprint.sql"),
    },
    Migration {
        version: 26,
        description: "skill provenance, evaluation, publishing, and rollback",
        sql: include_str!("../../migrations/026_skill_quality_release.sql"),
    },
    Migration {
        version: 27,
        description: "semantic change classification, citation audit, and impact summaries",
        sql: include_str!("../../migrations/027_semantic_change_audit.sql"),
    },
    Migration {
        version: 28,
        description: "real-model skill benchmark cases and comparison runs",
        sql: include_str!("../../migrations/028_skill_model_benchmarks.sql"),
    },
    Migration {
        version: 29,
        description: "detailed, reviewable semantic ingest skill candidate",
        sql: include_str!("../../migrations/029_detailed_ingest_skill.sql"),
    },
    Migration {
        version: 30,
        description: "publish detailed built-in knowledge skills",
        sql: include_str!("../../migrations/030_publish_detailed_builtin_skills.sql"),
    },
    Migration {
        version: 31,
        description: "strengthen wiki output contracts and evidence quality skills",
        sql: include_str!("../../migrations/031_wiki_prompt_contract_skills.sql"),
    },
    Migration {
        version: 32,
        description: "retain only the active version of each wiki skill",
        sql: include_str!("../../migrations/032_compact_wiki_skill_versions.sql"),
    },
    Migration {
        version: 33,
        description: "structured presentation planning skill and provenance",
        sql: include_str!("../../migrations/033_structured_presentation_skill.sql"),
    },
    Migration {
        version: 34,
        description: "editable chart and relationship presentation layouts",
        sql: include_str!("../../migrations/034_rich_presentation_layouts.sql"),
    },
    Migration {
        version: 35,
        description: "structured artifact validation details",
        sql: include_str!("../../migrations/035_artifact_validation_details.sql"),
    },
    Migration {
        version: 36,
        description: "multiple model providers and secure credential references",
        sql: include_str!("../../migrations/036_multiple_llm_providers.sql"),
    },
    Migration {
        version: 37,
        description: "run-scoped evidence actually read by agents",
        sql: include_str!("../../migrations/037_agent_run_evidence.sql"),
    },
    Migration {
        version: 38,
        description: "native Harness query and research skill contracts",
        sql: include_str!("../../migrations/038_harness_wiki_skill_contract.sql"),
    },
    Migration {
        version: 39,
        description: "bounded agent knowledge tool calls",
        sql: include_str!("../../migrations/039_agent_tool_call_budget.sql"),
    },
    Migration {
        version: 40,
        description: "stable numbered run citations with immutable visible evidence",
        sql: include_str!("../../migrations/040_numbered_run_citations.sql"),
    },
    Migration {
        version: 41,
        description: "explicit model context, output and reasoning capabilities",
        sql: include_str!("../../migrations/041_model_provider_capabilities.sql"),
    },
    Migration {
        version: 42,
        description: "numbered citation contract for stock Harness skills",
        sql: include_str!("../../migrations/042_numbered_citation_skill_contract.sql"),
    },
];

fn seed_detailed_ingest_skill(conn: &Connection) -> Result<(), BrainError> {
    seed_builtin_skill_file(
        conn,
        "skill-version-book-ingest-v3",
        "builtin-book-ingest-v3-pending",
        include_str!("../../skills/book-ingest/SKILL.md"),
    )
}

fn seed_detailed_builtin_skills(conn: &Connection) -> Result<(), BrainError> {
    for (version_id, placeholder, content) in [
        (
            "skill-version-book-query-v3",
            "builtin-book-query-v3-pending",
            include_str!("../../skills/book-query/SKILL.md"),
        ),
        (
            "skill-version-book-research-v3",
            "builtin-book-research-v3-pending",
            include_str!("../../skills/book-research/SKILL.md"),
        ),
        (
            "skill-version-book-presentation-v3",
            "builtin-book-presentation-v3-pending",
            include_str!("../../skills/book-presentation/SKILL.md"),
        ),
        (
            "skill-version-book-lint-v2",
            "builtin-book-lint-v2-pending",
            include_str!("../../skills/book-lint/SKILL.md"),
        ),
        (
            "skill-version-book-synthesis-v2",
            "builtin-book-synthesis-v2-pending",
            include_str!("../../skills/book-synthesis/SKILL.md"),
        ),
        (
            "skill-version-markdown-collection-v2",
            "builtin-markdown-collection-v2-pending",
            include_str!("../../skills/markdown-collection/SKILL.md"),
        ),
    ] {
        seed_builtin_skill_file(conn, version_id, placeholder, content)?;
    }
    Ok(())
}

fn seed_builtin_skill_file(
    conn: &Connection,
    version_id: &str,
    placeholder: &str,
    content: &str,
) -> Result<(), BrainError> {
    let content_hash = hex::encode(Sha256::digest(content.as_bytes()));
    let size_bytes = i64::try_from(content.len())
        .map_err(|_| BrainError::Internal("内置 Skill 大小超出限制".to_string()))?;
    let updated = conn.execute(
        "UPDATE skill_versions SET content_hash = ?1
         WHERE id = ?2 AND content_hash = ?3",
        params![&content_hash, version_id, placeholder],
    )?;
    if updated != 1 {
        return Err(BrainError::Internal(format!(
            "内置 Skill 版本 {version_id} 缺失或重复"
        )));
    }
    conn.execute(
        "INSERT INTO skill_files
            (skill_version_id, relative_path, media_type, content_text, content_hash, size_bytes)
         VALUES (?1, 'SKILL.md', 'text/markdown', ?2, ?3, ?4)",
        params![version_id, content, content_hash, size_bytes],
    )?;
    Ok(())
}

fn overwrite_wiki_prompt_contract_skills(conn: &Connection) -> Result<(), BrainError> {
    // The v3 identities are deliberately reused. Any score that evaluated the
    // previous body (or compared a candidate against it) is therefore stale.
    // Remove those gates before replacing the body so candidates must be
    // evaluated again against the new baseline. Benchmark case results cascade;
    // audited agent runs remain intact.
    conn.execute(
        "DELETE FROM skill_evaluation_runs
         WHERE skill_version_id IN (
             'skill-version-book-ingest-v3',
             'skill-version-book-query-v3',
             'skill-version-book-research-v3'
         ) OR baseline_version_id IN (
             'skill-version-book-ingest-v3',
             'skill-version-book-query-v3',
             'skill-version-book-research-v3'
         )",
        [],
    )?;
    conn.execute(
        "DELETE FROM skill_benchmark_runs
         WHERE skill_version_id IN (
             'skill-version-book-ingest-v3',
             'skill-version-book-query-v3',
             'skill-version-book-research-v3'
         ) OR baseline_version_id IN (
             'skill-version-book-ingest-v3',
             'skill-version-book-query-v3',
             'skill-version-book-research-v3'
         )",
        [],
    )?;
    for (skill_id, version_id, changelog, content) in [
        (
            "skill-book-ingest",
            "skill-version-book-ingest-v3",
            "完善语义编译输出契约、JSON 合法性、空结果原因、证据边界与逐项自检规则。",
            include_str!("../../skills/book-ingest/SKILL.md"),
        ),
        (
            "skill-book-query",
            "skill-version-book-query-v3",
            "完善问题分流、证据覆盖、条件与版本保留、冲突核验和可追溯回答规则。",
            include_str!("../../skills/book-query/SKILL.md"),
        ),
        (
            "skill-book-research",
            "skill-version-book-research-v3",
            "完善研究证据矩阵、反例与竞争解释、任务类型分流和可核验结论规则。",
            include_str!("../../skills/book-research/SKILL.md"),
        ),
    ] {
        let content_hash = hex::encode(Sha256::digest(content.as_bytes()));
        let size_bytes = i64::try_from(content.len())
            .map_err(|_| BrainError::Internal("内置 Skill 大小超出限制".to_string()))?;
        let updated = conn.execute(
            "UPDATE skill_versions
             SET content_hash = ?1, release_state = 'published', changelog = ?2
             WHERE id = ?3 AND skill_id = ?4",
            params![&content_hash, changelog, version_id, skill_id],
        )?;
        if updated != 1 {
            return Err(BrainError::Internal(format!(
                "内置 Skill 版本 {version_id} 缺失或重复"
            )));
        }
        let updated = conn.execute(
            "UPDATE skill_files
             SET media_type = 'text/markdown', content_text = ?1,
                 content_hash = ?2, size_bytes = ?3
             WHERE skill_version_id = ?4 AND relative_path = 'SKILL.md'",
            params![content, &content_hash, size_bytes, version_id],
        )?;
        if updated != 1 {
            return Err(BrainError::Internal(format!(
                "内置 Skill 文件 {version_id}/SKILL.md 缺失或重复"
            )));
        }
        let updated = conn.execute(
            "UPDATE skills
             SET current_version_id = ?1, updated_at = CURRENT_TIMESTAMP
             WHERE id = ?2 AND source_type = 'builtin'",
            params![version_id, skill_id],
        )?;
        if updated != 1 {
            return Err(BrainError::Internal(format!(
                "内置 Skill {skill_id} 缺失或类型错误"
            )));
        }
    }
    Ok(())
}

fn overwrite_harness_wiki_skills(conn: &Connection) -> Result<(), BrainError> {
    for (skill_id, version_id, content) in [
        (
            "skill-book-query",
            "skill-version-book-query-v3",
            include_str!("../../skills/book-query/SKILL.md"),
        ),
        (
            "skill-book-research",
            "skill-version-book-research-v3",
            include_str!("../../skills/book-research/SKILL.md"),
        ),
    ] {
        let current: Option<(String, String, String, i64)> = conn
            .query_row(
                "SELECT s.current_version_id, s.source_type, v.release_state,
                        EXISTS (SELECT 1 FROM skill_files f
                                WHERE f.skill_version_id = v.id
                                  AND f.relative_path = 'SKILL.md')
                 FROM skills s
                 JOIN skill_versions v ON v.id = s.current_version_id AND v.skill_id = s.id
                 WHERE s.id = ?1",
                params![skill_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?;
        let (current_version_id, source_type, release_state, has_skill_file) =
            current.ok_or_else(|| {
                BrainError::Internal(format!("内置 Skill {skill_id} 缺少有效的当前版本"))
            })?;
        if source_type != "builtin" || release_state != "published" || has_skill_file != 1 {
            return Err(BrainError::Internal(format!(
                "内置 Skill {skill_id} 当前版本不完整，不能执行迁移"
            )));
        }
        if current_version_id != version_id {
            tracing::info!(
                skill_id,
                current_version_id,
                "保留已发布的后续 Skill 版本，不覆盖为仓库默认内容"
            );
            continue;
        }
        // The stock v3 body is being replaced in place, so quality results
        // calculated against its former content are no longer valid. A later
        // published revision belongs to the user and must be left untouched.
        conn.execute(
            "DELETE FROM skill_evaluation_runs
             WHERE skill_version_id = ?1 OR baseline_version_id = ?1",
            params![version_id],
        )?;
        conn.execute(
            "DELETE FROM skill_benchmark_runs
             WHERE skill_version_id = ?1 OR baseline_version_id = ?1",
            params![version_id],
        )?;
        let content_hash = hex::encode(Sha256::digest(content.as_bytes()));
        let size_bytes = i64::try_from(content.len())
            .map_err(|_| BrainError::Internal("内置 Skill 大小超出限制".to_string()))?;
        let version_count = conn.execute(
            "UPDATE skill_versions SET content_hash = ?1, release_state = 'published',
                    changelog = '适配 Harness 原生 Skill、分层检索与实际已读稳定引用'
             WHERE id = ?2 AND skill_id = ?3",
            params![&content_hash, version_id, skill_id],
        )?;
        let file_count = conn.execute(
            "UPDATE skill_files SET content_text = ?1, content_hash = ?2, size_bytes = ?3
             WHERE skill_version_id = ?4 AND relative_path = 'SKILL.md'",
            params![content, &content_hash, size_bytes, version_id],
        )?;
        if version_count != 1 || file_count != 1 {
            return Err(BrainError::Internal(format!(
                "内置 Skill 当前版本 {version_id} 缺失或重复"
            )));
        }
        conn.execute(
            "UPDATE skills SET updated_at = ?2 WHERE id = ?1",
            params![skill_id, Utc::now().to_rfc3339()],
        )?;
    }
    Ok(())
}

fn compact_wiki_skill_versions(conn: &Connection) -> Result<(), BrainError> {
    let invalid_active_versions: i64 = conn.query_row(
        "SELECT COUNT(*)
         FROM skills s
         LEFT JOIN skill_versions v
                ON v.id = s.current_version_id AND v.skill_id = s.id
         WHERE s.current_version_id IS NULL OR v.id IS NULL",
        [],
        |row| row.get(0),
    )?;
    if invalid_active_versions != 0 {
        return Err(BrainError::Internal(format!(
            "有 {invalid_active_versions} 个 Skill 缺少有效的当前版本，无法清理历史正文"
        )));
    }

    // Scores for a discarded candidate or an obsolete baseline cannot be
    // reused after compaction. Case results cascade with benchmark runs.
    conn.execute(
        "DELETE FROM skill_evaluation_runs
         WHERE NOT EXISTS (
             SELECT 1 FROM skills s
             WHERE s.current_version_id = skill_evaluation_runs.skill_version_id
         ) OR (
             baseline_version_id IS NOT NULL AND NOT EXISTS (
                 SELECT 1 FROM skills s
                 WHERE s.current_version_id = skill_evaluation_runs.baseline_version_id
             )
         )",
        [],
    )?;
    conn.execute(
        "DELETE FROM skill_benchmark_runs
         WHERE NOT EXISTS (
             SELECT 1 FROM skills s
             WHERE s.current_version_id = skill_benchmark_runs.skill_version_id
         ) OR NOT EXISTS (
             SELECT 1 FROM skills s
             WHERE s.current_version_id = skill_benchmark_runs.baseline_version_id
         )",
        [],
    )?;
    conn.execute(
        "DELETE FROM skill_versions
         WHERE NOT EXISTS (
             SELECT 1 FROM skills s WHERE s.current_version_id = skill_versions.id
         )",
        [],
    )?;

    let remaining_versions: i64 =
        conn.query_row("SELECT COUNT(*) FROM skill_versions", [], |row| row.get(0))?;
    let skills: i64 = conn.query_row("SELECT COUNT(*) FROM skills", [], |row| row.get(0))?;
    if remaining_versions != skills {
        return Err(BrainError::Internal(format!(
            "Skill 版本清理结果异常：{skills} 个 Skill，{remaining_versions} 个当前版本"
        )));
    }
    Ok(())
}

fn overwrite_structured_presentation_skill(conn: &Connection) -> Result<(), BrainError> {
    let skill_id = "skill-book-presentation";
    let version_id = "skill-version-book-presentation-v3";
    let content = include_str!("../../skills/book-presentation/SKILL.md");
    let content_hash = hex::encode(Sha256::digest(content.as_bytes()));
    let size_bytes = i64::try_from(content.len())
        .map_err(|_| BrainError::Internal("内置演示 Skill 大小超出限制".to_string()))?;

    conn.execute(
        "DELETE FROM skill_evaluation_runs
         WHERE skill_version_id = ?1 OR baseline_version_id = ?1",
        params![version_id],
    )?;
    conn.execute(
        "DELETE FROM skill_benchmark_runs
         WHERE skill_version_id = ?1 OR baseline_version_id = ?1",
        params![version_id],
    )?;
    let updated = conn.execute(
        "UPDATE skill_versions
         SET content_hash = ?1, release_state = 'published',
             changelog = '重构为受众导向的结构化演示策划，增加叙事、证据、受限数据图、关系图和交付自检。'
         WHERE id = ?2 AND skill_id = ?3",
        params![&content_hash, version_id, skill_id],
    )?;
    if updated != 1 {
        return Err(BrainError::Internal(
            "当前演示 Skill 版本缺失或重复".to_string(),
        ));
    }
    let updated = conn.execute(
        "UPDATE skill_files
         SET media_type = 'text/markdown', content_text = ?1,
             content_hash = ?2, size_bytes = ?3
         WHERE skill_version_id = ?4 AND relative_path = 'SKILL.md'",
        params![content, &content_hash, size_bytes, version_id],
    )?;
    if updated != 1 {
        return Err(BrainError::Internal(
            "当前演示 Skill 正文缺失或重复".to_string(),
        ));
    }
    conn.execute(
        "UPDATE skills SET current_version_id = ?1, updated_at = CURRENT_TIMESTAMP
         WHERE id = ?2 AND source_type = 'builtin'",
        params![version_id, skill_id],
    )?;
    conn.execute(
        "INSERT INTO skill_version_origins
            (skill_version_id, repository_url, source_path, source_ref, license_spdx,
             attribution, adaptation_notes, reviewed_at)
         VALUES (?1, 'https://github.com/hugohe3/ppt-master',
                 'skills/ppt-master/SKILL.md', 'main', 'MIT',
                 'hugohe3/ppt-master',
                 '独立改写；借鉴策略、中间规格、质量门禁和可编辑产物思路，改造为数据库证据与受控 Rust 渲染流程，不复制其脚本或引入 Shell 执行。',
                 CURRENT_TIMESTAMP)
         ON CONFLICT(skill_version_id) DO UPDATE SET
             repository_url = excluded.repository_url,
             source_path = excluded.source_path,
             source_ref = excluded.source_ref,
             license_spdx = excluded.license_spdx,
             attribution = excluded.attribution,
             adaptation_notes = excluded.adaptation_notes,
             reviewed_at = excluded.reviewed_at",
        params![version_id],
    )?;
    Ok(())
}

impl SqliteStore {
    /// Open (or create) the database at `db_path`, enable WAL, run pending migrations.
    pub fn new(db_path: &Path) -> Result<Self, BrainError> {
        Self::new_with_backup_retention(db_path, DEFAULT_BACKUP_RETENTION)
    }

    pub fn new_with_backup_retention(
        db_path: &Path,
        backup_retention: usize,
    ) -> Result<Self, BrainError> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(db_path)
            .map_err(|e| BrainError::Internal(format!("SQLite 打开失败: {e}")))?;

        conn.execute_batch("PRAGMA journal_mode=WAL;")
            .map_err(|e| BrainError::Internal(format!("WAL 设置失败: {e}")))?;

        conn.execute_batch("PRAGMA busy_timeout=5000;")
            .map_err(|e| BrainError::Internal(format!("busy_timeout 设置失败: {e}")))?;

        conn.execute_batch("PRAGMA foreign_keys=ON;")
            .map_err(|e| BrainError::Internal(format!("foreign_keys 设置失败: {e}")))?;

        let store = SqliteStore {
            conn: Arc::new(Mutex::new(conn)),
            db_path: db_path.to_path_buf(),
            backup_retention: backup_retention.max(1),
        };
        store.run_migrations()?;
        Ok(store)
    }

    fn run_migrations(&self) -> Result<(), BrainError> {
        let current_version: u32 = {
            let conn = self.conn.lock().unwrap();
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS _migrations (
                    version INTEGER PRIMARY KEY,
                    description TEXT NOT NULL,
                    applied_at DATETIME DEFAULT CURRENT_TIMESTAMP
                );",
            )
            .map_err(|e| BrainError::Internal(format!("迁移表创建失败: {e}")))?;

            conn.query_row(
                "SELECT COALESCE(MAX(version), 0) FROM _migrations",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0)
        };

        let pending: Vec<&Migration> = MIGRATIONS
            .iter()
            .filter(|m| m.version > current_version)
            .collect();

        if pending.is_empty() {
            return Ok(());
        }

        // Existing user data gets a consistent snapshot before schema changes.
        if current_version > 0 {
            self.create_managed_backup("pre-migration", self.backup_retention)?;
        }

        let conn = self.conn.lock().unwrap();

        // Wrap all pending migrations in a single transaction
        conn.execute_batch("BEGIN IMMEDIATE;")
            .map_err(|e| BrainError::Internal(format!("迁移事务开始失败: {e}")))?;

        for migration in &pending {
            tracing::info!("执行迁移 v{}: {}", migration.version, migration.description);
            if let Err(e) = conn.execute_batch(migration.sql) {
                let _ = conn.execute_batch("ROLLBACK;");
                return Err(BrainError::Internal(format!(
                    "迁移 v{} 执行失败: {e}",
                    migration.version
                )));
            }
            let seed_result = match migration.version {
                29 => seed_detailed_ingest_skill(&conn),
                30 => seed_detailed_builtin_skills(&conn),
                31 => overwrite_wiki_prompt_contract_skills(&conn),
                32 => compact_wiki_skill_versions(&conn),
                33 => overwrite_structured_presentation_skill(&conn),
                34 => overwrite_structured_presentation_skill(&conn),
                38 | 42 => overwrite_harness_wiki_skills(&conn),
                _ => Ok(()),
            };
            if let Err(error) = seed_result {
                let _ = conn.execute_batch("ROLLBACK;");
                return Err(BrainError::Internal(format!(
                    "迁移 v{} Skill 内容写入失败: {error}",
                    migration.version
                )));
            }
            if let Err(e) = conn.execute(
                "INSERT INTO _migrations (version, description) VALUES (?1, ?2)",
                params![migration.version, migration.description],
            ) {
                let _ = conn.execute_batch("ROLLBACK;");
                return Err(BrainError::Internal(format!(
                    "迁移 v{} 记录失败: {e}",
                    migration.version
                )));
            }
        }

        conn.execute_batch("COMMIT;")
            .map_err(|e| BrainError::Internal(format!("迁移事务提交失败: {e}")))?;

        Ok(())
    }

    fn backups_dir(&self) -> PathBuf {
        self.db_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("backups")
    }

    pub fn backup_retention(&self) -> usize {
        self.backup_retention
    }

    pub fn database_path(&self) -> &Path {
        &self.db_path
    }

    fn sanitize_backup_reason(reason: &str) -> String {
        let sanitized = reason
            .trim()
            .chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                    character
                } else {
                    '-'
                }
            })
            .collect::<String>()
            .trim_matches('-')
            .to_string();
        if sanitized.is_empty() {
            "manual".to_string()
        } else {
            sanitized
        }
    }

    fn backup_reason_from_filename(filename: &str) -> Option<String> {
        let stem = filename.strip_suffix(".sqlite3")?;
        let mut parts = stem.splitn(4, '-');
        if parts.next()? != "brain" {
            return None;
        }
        parts.next()?.parse::<i64>().ok()?;
        let nonce = parts.next()?;
        if nonce.len() != 32 || !nonce.chars().all(|character| character.is_ascii_hexdigit()) {
            return None;
        }
        let reason = parts.next()?.trim();
        (!reason.is_empty()).then(|| reason.to_string())
    }

    fn summary_for_backup(path: PathBuf) -> Result<DatabaseBackupSummary, BrainError> {
        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| BrainError::Internal("备份文件名无效".to_string()))?
            .to_string();
        let reason = Self::backup_reason_from_filename(&filename)
            .ok_or_else(|| BrainError::Internal("备份文件不属于当前应用".to_string()))?;
        let metadata = fs::metadata(&path)?;
        let created_at = metadata
            .modified()
            .ok()
            .map(DateTime::<Utc>::from)
            .unwrap_or_else(Utc::now)
            .to_rfc3339();
        Ok(DatabaseBackupSummary {
            filename,
            reason,
            created_at,
            size_bytes: metadata.len(),
            path,
        })
    }

    /// Create a transactionally consistent snapshot using SQLite's Online Backup API.
    pub fn create_managed_backup(
        &self,
        reason: &str,
        retention: usize,
    ) -> Result<DatabaseBackupSummary, BrainError> {
        let backup_dir = self.backups_dir();
        fs::create_dir_all(&backup_dir)?;
        let filename = format!(
            "{MANAGED_BACKUP_PREFIX}{}-{}-{}.{}",
            Utc::now().timestamp_millis(),
            uuid::Uuid::new_v4().simple(),
            Self::sanitize_backup_reason(reason),
            MANAGED_BACKUP_EXTENSION
        );
        let path = backup_dir.join(&filename);

        {
            let conn = self.conn.lock().unwrap();
            conn.backup(DatabaseName::Main, &path, None)
                .map_err(|error| BrainError::Internal(format!("SQLite 在线备份失败: {error}")))?;
        }

        let report = Self::validate_database_file(&path)?;
        if !report.integrity_ok || report.foreign_key_violations > 0 {
            let _ = fs::remove_file(&path);
            return Err(BrainError::Internal(format!(
                "备份校验失败: {}, 外键异常 {} 条",
                report.integrity_message, report.foreign_key_violations
            )));
        }

        let summary = Self::summary_for_backup(path)?;
        self.prune_managed_backups(retention.max(1), Some(&summary.filename))?;
        Ok(summary)
    }

    /// List only snapshots created by this application, newest first.
    pub fn list_managed_backups(&self) -> Result<Vec<DatabaseBackupSummary>, BrainError> {
        let backup_dir = self.backups_dir();
        if !backup_dir.exists() {
            return Ok(Vec::new());
        }
        let mut backups = fs::read_dir(backup_dir)?
            .filter_map(Result::ok)
            .filter_map(|entry| Self::summary_for_backup(entry.path()).ok())
            .collect::<Vec<_>>();
        backups.sort_by(|left, right| right.filename.cmp(&left.filename));
        Ok(backups)
    }

    /// Resolve a public backup identifier without accepting arbitrary paths.
    pub fn managed_backup_path(&self, filename: &str) -> Result<PathBuf, BrainError> {
        if Self::backup_reason_from_filename(filename).is_none()
            || Path::new(filename).components().count() != 1
        {
            return Err(BrainError::KnowledgeValidation(
                "备份文件标识无效".to_string(),
            ));
        }
        let path = self.backups_dir().join(filename);
        if !path.is_file() {
            return Err(BrainError::KnowledgeNotFound(filename.to_string()));
        }
        Ok(path)
    }

    fn prune_managed_backups(
        &self,
        retention: usize,
        preserve: Option<&str>,
    ) -> Result<(), BrainError> {
        let backups = self.list_managed_backups()?;
        let keep = retention.max(1);
        for backup in backups.iter().skip(keep) {
            if preserve == Some(backup.filename.as_str()) {
                continue;
            }
            fs::remove_file(&backup.path)?;
        }
        Ok(())
    }

    /// Open a database read-only and check integrity, foreign keys and schema level.
    pub fn validate_database_file(path: &Path) -> Result<DatabaseValidationReport, BrainError> {
        // FTS5's integrity hook may need a transient write lock even though the
        // check itself does not change user data, so validate a private upload
        // or managed snapshot with read/write access.
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)
            .map_err(|error| BrainError::Internal(format!("备份数据库无法打开: {error}")))?;
        let integrity_message: String = conn
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .map_err(|error| BrainError::Internal(format!("数据库完整性检查失败: {error}")))?;
        let foreign_key_violations: u64 = conn
            .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
                row.get(0)
            })
            .map_err(|error| BrainError::Internal(format!("数据库外键检查失败: {error}")))?;
        let migration_version: u32 = conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM _migrations",
                [],
                |row| row.get(0),
            )
            .map_err(|error| BrainError::Internal(format!("数据库迁移版本读取失败: {error}")))?;

        Ok(DatabaseValidationReport {
            integrity_ok: integrity_message.eq_ignore_ascii_case("ok"),
            integrity_message,
            foreign_key_violations,
            migration_version,
        })
    }

    /// Replace the live database with a checked snapshot after explicit confirmation.
    pub fn restore_database_file(
        &self,
        source_path: &Path,
        confirmation: &str,
        retention: usize,
    ) -> Result<DatabaseValidationReport, BrainError> {
        if confirmation != "RESTORE" {
            return Err(BrainError::KnowledgeValidation(
                "恢复数据库前请输入 RESTORE 确认".to_string(),
            ));
        }
        let source_report = Self::validate_database_file(source_path)?;
        let latest_version = MIGRATIONS
            .last()
            .map(|migration| migration.version)
            .unwrap_or(0);
        if !source_report.integrity_ok || source_report.foreign_key_violations > 0 {
            return Err(BrainError::KnowledgeValidation(format!(
                "恢复文件校验失败: {}, 外键异常 {} 条",
                source_report.integrity_message, source_report.foreign_key_violations
            )));
        }
        if source_report.migration_version > latest_version {
            return Err(BrainError::KnowledgeValidation(format!(
                "恢复文件版本 v{} 高于当前应用支持的 v{}",
                source_report.migration_version, latest_version
            )));
        }

        let active_work: u64 = {
            let conn = self.conn.lock().unwrap();
            conn.query_row(
                "SELECT
                    (SELECT COUNT(*) FROM agent_runs WHERE status IN ('queued', 'running')) +
                    (SELECT COUNT(*) FROM knowledge_tasks WHERE status IN ('queued', 'running')) +
                    (SELECT COUNT(*) FROM knowledge_bases
                       WHERE compile_state = 'compiling' AND compile_phase != 'waiting_review')",
                [],
                |row| row.get(0),
            )
            .map_err(|error| BrainError::Internal(format!("运行任务检查失败: {error}")))?
        };
        if active_work > 0 {
            return Err(BrainError::KnowledgeValidation(format!(
                "仍有 {active_work} 项智能编译、知识任务或 Agent 运行中，请结束后再恢复"
            )));
        }

        // Do not let retention pruning remove a selected managed snapshot before
        // the restore has consumed it. Normal retention is applied afterwards.
        let temporary_retention = self
            .list_managed_backups()?
            .len()
            .saturating_add(1)
            .max(retention.max(1));
        let safety_backup = self.create_managed_backup("pre-restore", temporary_retention)?;
        let restoration = (|| {
            self.replace_database_from_path(source_path)?;
            self.run_migrations()?;
            let report = Self::validate_database_file(&self.db_path)?;
            if !report.integrity_ok || report.foreign_key_violations > 0 {
                return Err(BrainError::Internal(format!(
                    "数据库恢复后校验失败: {}, 外键异常 {} 条",
                    report.integrity_message, report.foreign_key_violations
                )));
            }
            Ok(report)
        })();
        let report = match restoration {
            Ok(report) => report,
            Err(error) => {
                let rollback = self
                    .replace_database_from_path(&safety_backup.path)
                    .and_then(|_| self.run_migrations());
                return match rollback {
                    Ok(()) => Err(BrainError::Internal(format!(
                        "数据库恢复失败，已回滚到操作前快照: {error}"
                    ))),
                    Err(rollback_error) => Err(BrainError::Internal(format!(
                        "数据库恢复失败且自动回滚未完成: {error}; 回滚错误: {rollback_error}"
                    ))),
                };
            }
        };
        self.prune_managed_backups(retention.max(1), None)?;
        Ok(report)
    }

    fn replace_database_from_path(&self, source_path: &Path) -> Result<(), BrainError> {
        let mut conn = self.conn.lock().unwrap();
        conn.restore(
            DatabaseName::Main,
            source_path,
            None::<fn(rusqlite::backup::Progress)>,
        )
        .map_err(|error| BrainError::Internal(format!("SQLite 恢复失败: {error}")))?;
        conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA busy_timeout=5000;")
            .map_err(|error| BrainError::Internal(format!("恢复后数据库初始化失败: {error}")))?;
        Ok(())
    }

    /// Execute a closure inside a transaction.
    pub fn transaction<F, T>(&self, f: F) -> Result<T, BrainError>
    where
        F: FnOnce(&Connection) -> Result<T, BrainError>,
    {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch("BEGIN IMMEDIATE;")
            .map_err(|e| BrainError::Internal(format!("事务开始失败: {e}")))?;

        match f(&conn) {
            Ok(result) => {
                conn.execute_batch("COMMIT;")
                    .map_err(|e| BrainError::Internal(format!("事务提交失败: {e}")))?;
                Ok(result)
            }
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK;");
                Err(e)
            }
        }
    }

    /// Execute a read-only closure with the shared SQLite connection.
    pub fn with_connection<F, T>(&self, f: F) -> Result<T, BrainError>
    where
        F: FnOnce(&Connection) -> Result<T, BrainError>,
    {
        let conn = self
            .conn
            .lock()
            .map_err(|error| BrainError::Internal(format!("SQLite 锁已损坏: {error}")))?;
        f(&conn)
    }

    // ── App state helpers ──

    pub fn get_state(&self, key: &str) -> Result<Option<String>, BrainError> {
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row(
            "SELECT value FROM app_state WHERE key = ?1",
            params![key],
            |row| row.get::<_, String>(0),
        );
        match result {
            Ok(val) => Ok(Some(val)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(BrainError::Internal(format!("状态查询失败: {e}"))),
        }
    }

    pub fn set_state(&self, key: &str, value: &str) -> Result<(), BrainError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO app_state (key, value, updated_at)
             VALUES (?1, ?2, CURRENT_TIMESTAMP)
             ON CONFLICT(key) DO UPDATE SET value = ?2, updated_at = CURRENT_TIMESTAMP",
            params![key, value],
        )
        .map_err(|e| BrainError::Internal(format!("状态写入失败: {e}")))?;
        Ok(())
    }

    /// Check if the store is healthy (can execute a simple query).
    pub fn health_check(&self) -> bool {
        let conn = self.conn.lock().unwrap();
        conn.query_row("SELECT 1", [], |_| Ok(true))
            .unwrap_or(false)
    }

    // ── Code Repos ──

    pub fn insert_code_repo(
        &self,
        name: &str,
        path: &str,
        metadata: &str,
    ) -> Result<(), BrainError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO code_repos (name, path, metadata) VALUES (?1, ?2, ?3)",
            params![name, path, metadata],
        )
        .map_err(|e| BrainError::Internal(format!("插入代码仓失败: {e}")))?;
        Ok(())
    }

    pub fn get_code_repo_by_name(
        &self,
        name: &str,
    ) -> Result<Option<(String, String, String)>, BrainError> {
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row(
            "SELECT name, path, metadata FROM code_repos WHERE name = ?1",
            params![name],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        );
        match result {
            Ok(val) => Ok(Some(val)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(BrainError::Internal(format!("查询代码仓失败: {e}"))),
        }
    }

    pub fn list_code_repos(&self) -> Result<Vec<(String, String, String)>, BrainError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT name, path, metadata FROM code_repos")
            .map_err(|e| BrainError::Internal(format!("准备查询失败: {e}")))?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|e| BrainError::Internal(format!("查询代码仓失败: {e}")))?;
        let mut repos = Vec::new();
        for row in rows {
            repos.push(row.map_err(|e| BrainError::Internal(format!("读取行失败: {e}")))?);
        }
        Ok(repos)
    }

    pub fn delete_code_repo(&self, name: &str) -> Result<bool, BrainError> {
        let conn = self.conn.lock().unwrap();
        let rows_changed = conn
            .execute("DELETE FROM code_repos WHERE name = ?1", params![name])
            .map_err(|e| BrainError::Internal(format!("删除代码仓失败: {e}")))?;
        Ok(rows_changed > 0)
    }

    pub fn update_repo_metadata(&self, name: &str, metadata: &str) -> Result<(), BrainError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE code_repos SET metadata = ?1 WHERE name = ?2",
            params![metadata, name],
        )
        .map_err(|e| BrainError::Internal(format!("更新代码仓元数据失败: {e}")))?;
        Ok(())
    }

    // ── Note-Repo Links ──

    pub fn insert_note_repo_link(
        &self,
        note_path: &str,
        repo_name: &str,
    ) -> Result<(), BrainError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO note_repo_links (note_path, repo_name) VALUES (?1, ?2)",
            params![note_path, repo_name],
        )
        .map_err(|e| BrainError::Internal(format!("插入笔记-仓库关联失败: {e}")))?;
        Ok(())
    }

    pub fn get_linked_notes(&self, repo_name: &str) -> Result<Vec<String>, BrainError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT note_path FROM note_repo_links WHERE repo_name = ?1")
            .map_err(|e| BrainError::Internal(format!("准备查询失败: {e}")))?;
        let rows = stmt
            .query_map(params![repo_name], |row| row.get::<_, String>(0))
            .map_err(|e| BrainError::Internal(format!("查询关联笔记失败: {e}")))?;
        let mut notes = Vec::new();
        for row in rows {
            notes.push(row.map_err(|e| BrainError::Internal(format!("读取行失败: {e}")))?);
        }
        Ok(notes)
    }

    pub fn count_note_links(&self, repo_name: &str) -> Result<usize, BrainError> {
        let conn = self.conn.lock().unwrap();
        let count: usize = conn
            .query_row(
                "SELECT COUNT(*) FROM note_repo_links WHERE repo_name = ?1",
                params![repo_name],
                |row| row.get(0),
            )
            .map_err(|e| BrainError::Internal(format!("统计关联笔记失败: {e}")))?;
        Ok(count)
    }

    // ── Timeline Events ──

    pub fn insert_timeline_event(
        &self,
        id: &str,
        date: &str,
        event_type: &str,
        title: &str,
        summary: &str,
        tags: &str,
        related_paths: &str,
        source: &str,
    ) -> Result<(), BrainError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO timeline_events (id, date, event_type, title, summary, tags, related_paths, source_path) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![id, date, event_type, title, summary, tags, related_paths, source],
        )
        .map_err(|e| BrainError::Internal(format!("插入时间线事件失败: {e}")))?;
        Ok(())
    }

    pub fn get_timeline_events(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> Result<
        Vec<(
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            String,
        )>,
        BrainError,
    > {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT id, date, event_type, title, summary, tags, related_paths, source_path
                 FROM timeline_events
                 WHERE date >= ?1 AND date <= ?2
                 ORDER BY date",
            )
            .map_err(|e| BrainError::Internal(format!("准备查询失败: {e}")))?;
        let rows = stmt
            .query_map(params![start_date, end_date], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                ))
            })
            .map_err(|e| BrainError::Internal(format!("查询时间线事件失败: {e}")))?;
        let mut events = Vec::new();
        for row in rows {
            events.push(row.map_err(|e| BrainError::Internal(format!("读取行失败: {e}")))?);
        }
        Ok(events)
    }

    pub fn delete_timeline_events_before(&self, before_date: &str) -> Result<usize, BrainError> {
        let conn = self.conn.lock().unwrap();
        let rows_deleted = conn
            .execute(
                "DELETE FROM timeline_events WHERE date < ?1",
                params![before_date],
            )
            .map_err(|e| BrainError::Internal(format!("删除时间线事件失败: {e}")))?;
        Ok(rows_deleted)
    }

    // ── Inspiration History ──

    pub fn insert_inspiration(
        &self,
        id: &str,
        insp_type: &str,
        input_refs: &str,
        output: &str,
    ) -> Result<(), BrainError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO inspiration_history (id, type, input_refs, output) VALUES (?1, ?2, ?3, ?4)",
            params![id, insp_type, input_refs, output],
        )
        .map_err(|e| BrainError::Internal(format!("插入灵感记录失败: {e}")))?;
        Ok(())
    }

    pub fn get_recent_inspirations(
        &self,
        insp_type: &str,
        limit: i64,
    ) -> Result<Vec<(String, String, String, String)>, BrainError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT id, type, input_refs, output FROM inspiration_history
                 WHERE type = ?1 ORDER BY created_at DESC LIMIT ?2",
            )
            .map_err(|e| BrainError::Internal(format!("准备查询失败: {e}")))?;
        let rows = stmt
            .query_map(params![insp_type, limit], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(|e| BrainError::Internal(format!("查询灵感记录失败: {e}")))?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(|e| BrainError::Internal(format!("读取行失败: {e}")))?);
        }
        Ok(results)
    }

    // ── Radar Items ──

    pub fn insert_radar_item(
        &self,
        id: &str,
        title: &str,
        summary: &str,
        source_name: &str,
        url: &str,
    ) -> Result<(), BrainError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO radar_items (id, title, summary, source_name, url) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, title, summary, source_name, url],
        )
        .map_err(|e| BrainError::Internal(format!("插入雷达条目失败: {e}")))?;
        Ok(())
    }

    pub fn get_radar_items(
        &self,
        status: &str,
        limit: i64,
    ) -> Result<Vec<(String, String, String, String, String, String)>, BrainError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT id, title, summary, source_name, url, status FROM radar_items
                 WHERE status = ?1 ORDER BY fetched_at DESC LIMIT ?2",
            )
            .map_err(|e| BrainError::Internal(format!("准备查询失败: {e}")))?;
        let rows = stmt
            .query_map(params![status, limit], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                ))
            })
            .map_err(|e| BrainError::Internal(format!("查询雷达条目失败: {e}")))?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(|e| BrainError::Internal(format!("读取行失败: {e}")))?);
        }
        Ok(results)
    }

    // ── Memos (Time Machine) ──

    pub fn insert_memo(
        &self,
        id: &str,
        timestamp: &str,
        date: &str,
        content: &str,
        images: &str,
        tags: &str,
        file_path: &str,
    ) -> Result<(), BrainError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO memos (id, timestamp, date, content, images, tags, file_path) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, timestamp, date, content, images, tags, file_path],
        )
        .map_err(|e| BrainError::Internal(format!("插入小记失败: {e}")))?;
        Ok(())
    }

    /// Count total memos in the database.
    pub fn count_memos(&self) -> Result<u32, BrainError> {
        let conn = self.conn.lock().unwrap();
        let count: u32 = conn
            .query_row("SELECT COUNT(*) FROM memos", [], |row| row.get(0))
            .map_err(|e| BrainError::Internal(format!("统计小记数量失败: {e}")))?;
        Ok(count)
    }

    /// Insert or update a memo (for sync from Obsidian files).
    pub fn upsert_memo(
        &self,
        id: &str,
        timestamp: &str,
        date: &str,
        content: &str,
        images: &str,
        tags: &str,
        file_path: &str,
    ) -> Result<(), BrainError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO memos (id, timestamp, date, content, images, tags, file_path)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(id) DO UPDATE SET
               timestamp = excluded.timestamp,
               date = excluded.date,
               content = excluded.content,
               images = excluded.images,
               tags = excluded.tags,
               file_path = excluded.file_path",
            params![id, timestamp, date, content, images, tags, file_path],
        )
        .map_err(|e| BrainError::Internal(format!("同步小记失败: {e}")))?;
        Ok(())
    }

    /// Find an existing memo ID by timestamp (for dedup during sync).
    /// Compares only the date+time portion (first 19 chars: YYYY-MM-DDTHH:MM:SS)
    /// to handle different timezone offsets and microsecond precision.
    pub fn find_memo_id_by_timestamp(&self, timestamp: &str) -> Result<Option<String>, BrainError> {
        let conn = self.conn.lock().unwrap();
        // Normalize: take first 19 chars (YYYY-MM-DDTHH:MM:SS)
        let normalized = if timestamp.len() >= 19 {
            &timestamp[..19]
        } else {
            timestamp
        };
        let pattern = format!("{}%", normalized);
        let result = conn.query_row(
            "SELECT id FROM memos WHERE timestamp LIKE ?1 LIMIT 1",
            params![pattern],
            |row| row.get::<_, String>(0),
        );
        match result {
            Ok(id) => Ok(Some(id)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(BrainError::Internal(format!("查询小记 ID 失败: {e}"))),
        }
    }

    /// Delete memos whose date is in synced_dates but whose ID is NOT in keep_ids.
    /// Used during sync to remove memos deleted from Obsidian.
    pub fn delete_memos_not_by_ids(
        &self,
        synced_dates: &std::collections::HashSet<String>,
        keep_ids: &[String],
    ) -> Result<u32, BrainError> {
        if synced_dates.is_empty() {
            return Ok(0);
        }

        let conn = self.conn.lock().unwrap();

        // Build date IN clause
        let date_placeholders: Vec<String> = synced_dates
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect();
        let date_in = date_placeholders.join(", ");

        // Build ID NOT IN clause
        let id_placeholders: Vec<String> = keep_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + synced_dates.len() + 1))
            .collect();
        let id_not_in = if id_placeholders.is_empty() {
            "''".to_string()
        } else {
            id_placeholders.join(", ")
        };

        let sql = format!(
            "DELETE FROM memos WHERE date IN ({}) AND id NOT IN ({})",
            date_in, id_not_in
        );

        // Collect all params: dates first, then IDs
        let mut all_params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        for d in synced_dates {
            all_params.push(Box::new(d.clone()));
        }
        for id in keep_ids {
            all_params.push(Box::new(id.clone()));
        }
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            all_params.iter().map(|p| p.as_ref()).collect();

        let deleted = conn
            .execute(&sql, param_refs.as_slice())
            .map_err(|e| BrainError::Internal(format!("删除过期小记失败: {e}")))?;

        Ok(deleted as u32)
    }

    pub fn query_memos(
        &self,
        sql: &str,
        params: &[String],
    ) -> Result<
        Vec<(
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            String,
        )>,
        BrainError,
    > {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(sql)
            .map_err(|e| BrainError::Internal(format!("准备查询失败: {e}")))?;
        let params_refs: Vec<&dyn rusqlite::types::ToSql> = params
            .iter()
            .map(|s| s as &dyn rusqlite::types::ToSql)
            .collect();
        let rows = stmt
            .query_map(params_refs.as_slice(), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                ))
            })
            .map_err(|e| BrainError::Internal(format!("查询小记失败: {e}")))?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(|e| BrainError::Internal(format!("读取行失败: {e}")))?);
        }
        Ok(results)
    }

    pub fn update_radar_status(&self, id: &str, status: &str) -> Result<bool, BrainError> {
        let conn = self.conn.lock().unwrap();
        let rows_changed = conn
            .execute(
                "UPDATE radar_items SET status = ?1 WHERE id = ?2",
                params![status, id],
            )
            .map_err(|e| BrainError::Internal(format!("更新雷达状态失败: {e}")))?;
        Ok(rows_changed > 0)
    }

    pub fn radar_url_exists(&self, url: &str) -> Result<bool, BrainError> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM radar_items WHERE url = ?1",
                params![url],
                |row| row.get(0),
            )
            .map_err(|e| BrainError::Internal(format!("查询雷达URL失败: {e}")))?;
        Ok(count > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_new_creates_db_and_runs_migrations() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let store = SqliteStore::new(&db_path).unwrap();

        let conn = store.conn.lock().unwrap();
        let count: u32 = conn
            .query_row("SELECT COUNT(*) FROM _migrations", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, MIGRATIONS.len() as u32);

        let max_version: u32 = conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM _migrations",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            max_version,
            MIGRATIONS
                .last()
                .map(|migration| migration.version)
                .unwrap_or(0)
        );
    }

    #[test]
    fn test_migration_010_removes_task_sync_objects() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let store = SqliteStore::new(&db_path).unwrap();
        let conn = store.conn.lock().unwrap();

        let queue_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='task_sync_queue'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!queue_exists);

        let sync_error_columns: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('task_documents') WHERE name='sync_error'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(sync_error_columns, 0);
    }

    #[test]
    fn test_migrations_are_idempotent() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");

        let _store1 = SqliteStore::new(&db_path).unwrap();
        let store2 = SqliteStore::new(&db_path).unwrap();
        assert!(store2.health_check());
    }

    #[test]
    fn test_migration_038_preserves_published_builtin_v4_after_single_version_cleanup() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("published-skill-v4.db");
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "PRAGMA foreign_keys=ON;
             CREATE TABLE _migrations (
                 version INTEGER PRIMARY KEY, description TEXT NOT NULL,
                 applied_at DATETIME DEFAULT CURRENT_TIMESTAMP
             );",
        )
        .unwrap();
        for migration in MIGRATIONS
            .iter()
            .filter(|migration| migration.version <= 36)
        {
            conn.execute_batch(migration.sql).unwrap();
            match migration.version {
                29 => seed_detailed_ingest_skill(&conn).unwrap(),
                30 => seed_detailed_builtin_skills(&conn).unwrap(),
                31 => overwrite_wiki_prompt_contract_skills(&conn).unwrap(),
                32 => compact_wiki_skill_versions(&conn).unwrap(),
                33 | 34 => overwrite_structured_presentation_skill(&conn).unwrap(),
                _ => {}
            }
            conn.execute(
                "INSERT INTO _migrations (version, description) VALUES (?1, ?2)",
                params![migration.version, migration.description],
            )
            .unwrap();
        }
        for (skill_id, version_id, content) in [
            (
                "skill-book-query",
                "skill-version-book-query-v4",
                "已发布的问答 v4 规则",
            ),
            (
                "skill-book-research",
                "skill-version-book-research-v4",
                "已发布的研究 v4 规则",
            ),
        ] {
            let hash = hex::encode(Sha256::digest(content.as_bytes()));
            conn.execute(
                "INSERT INTO skill_versions
                     (id, skill_id, revision, content_hash, release_state, created_at)
                 VALUES (?1, ?2, 4, ?3, 'published', CURRENT_TIMESTAMP)",
                params![version_id, skill_id, hash],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO skill_files
                     (skill_version_id, relative_path, media_type, content_text,
                      content_hash, size_bytes)
                 VALUES (?1, 'SKILL.md', 'text/markdown', ?2, ?3, ?4)",
                params![version_id, content, hash, content.len() as i64],
            )
            .unwrap();
            conn.execute(
                "UPDATE skills SET current_version_id = ?2 WHERE id = ?1",
                params![skill_id, version_id],
            )
            .unwrap();
        }
        compact_wiki_skill_versions(&conn).unwrap();
        conn.execute(
            "INSERT INTO app_state (key, value) VALUES ('migration-v38-user-state', 'preserved')",
            [],
        )
        .unwrap();
        drop(conn);

        let store = SqliteStore::new(&db_path).unwrap();
        store
            .with_connection(|conn| {
                let latest: i64 =
                    conn.query_row("SELECT MAX(version) FROM _migrations", [], |row| row.get(0))?;
                assert_eq!(latest as u32, MIGRATIONS.last().unwrap().version);
                for (skill_id, version_id, content) in [
                    (
                        "skill-book-query",
                        "skill-version-book-query-v4",
                        "已发布的问答 v4 规则",
                    ),
                    (
                        "skill-book-research",
                        "skill-version-book-research-v4",
                        "已发布的研究 v4 规则",
                    ),
                ] {
                    let actual: (String, String) = conn.query_row(
                        "SELECT s.current_version_id, f.content_text
                         FROM skills s JOIN skill_files f
                           ON f.skill_version_id = s.current_version_id
                         WHERE s.id = ?1 AND f.relative_path = 'SKILL.md'",
                        params![skill_id],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )?;
                    assert_eq!(actual, (version_id.to_string(), content.to_string()));
                }
                let state: String = conn.query_row(
                    "SELECT value FROM app_state WHERE key = 'migration-v38-user-state'",
                    [],
                    |row| row.get(0),
                )?;
                assert_eq!(state, "preserved");
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn test_migrations_031_through_036_keep_only_current_skill_bodies() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("upgrade-from-030.db");
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "PRAGMA foreign_keys=ON;
             CREATE TABLE _migrations (
                 version INTEGER PRIMARY KEY, description TEXT NOT NULL,
                 applied_at DATETIME DEFAULT CURRENT_TIMESTAMP
             );",
        )
        .unwrap();
        for migration in MIGRATIONS
            .iter()
            .filter(|migration| migration.version <= 30)
        {
            conn.execute_batch(migration.sql).unwrap();
            match migration.version {
                29 => seed_detailed_ingest_skill(&conn).unwrap(),
                30 => seed_detailed_builtin_skills(&conn).unwrap(),
                _ => {}
            }
            conn.execute(
                "INSERT INTO _migrations (version, description) VALUES (?1, ?2)",
                params![migration.version, migration.description],
            )
            .unwrap();
        }
        for (version_id, legacy_hash) in [
            ("skill-version-book-ingest-v3", "legacy-ingest-v3"),
            ("skill-version-book-query-v3", "legacy-query-v3"),
            ("skill-version-book-research-v3", "legacy-research-v3"),
        ] {
            conn.execute(
                "UPDATE skill_versions SET content_hash = ?1 WHERE id = ?2",
                params![legacy_hash, version_id],
            )
            .unwrap();
            conn.execute(
                "UPDATE skill_files
                 SET content_text = 'legacy placeholder', content_hash = ?1, size_bytes = 18
                 WHERE skill_version_id = ?2 AND relative_path = 'SKILL.md'",
                params![legacy_hash, version_id],
            )
            .unwrap();
        }
        conn.execute(
            "INSERT INTO reader_books (id, path, kind, name, added_at)
             VALUES ('migration-031-book', '/migration-031-book', 'folder', '迁移测试', 1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO knowledge_bases
                (id, book_id, created_at, updated_at)
             VALUES ('migration-031-base', 'migration-031-book', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO skill_evaluation_runs
                (id, skill_version_id, suite_id, baseline_version_id, score,
                 baseline_score, passed, findings_json, created_at)
             VALUES ('stale-evaluation', 'skill-version-book-query-v2',
                     'skill-suite-query', 'skill-version-book-query-v3',
                     1, 1, 1, '[]', CURRENT_TIMESTAMP)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO skill_benchmark_runs
                (id, skill_id, skill_version_id, baseline_version_id, suite_id,
                 knowledge_base_id, runtime_profile_id, status, total_cases,
                 completed_cases, candidate_score, baseline_score, score_delta,
                 passed, created_at)
             VALUES ('stale-benchmark', 'skill-book-query',
                     'skill-version-book-query-v2', 'skill-version-book-query-v3',
                     'skill-suite-query', 'migration-031-base',
                     'runtime-deepseek-harness', 'completed', 1, 1, 1, 0, 1, 1,
                     CURRENT_TIMESTAMP)",
            [],
        )
        .unwrap();
        conn.execute(
            "UPDATE agent_runtime_profiles
             SET model = 'glm-5.2',
                 config_json = json_object(
                     'provider', json_object(
                         'provider_id', 'aliyun-bailian',
                         'display_name', '阿里云百炼',
                         'api_protocol', 'openai-completions',
                         'base_url', 'https://dashscope.aliyuncs.com/compatible-mode/v1',
                         'api_key_env', 'CUSTOM_LLM_API_KEY'
                     )
                 )
             WHERE id = 'runtime-deepseek-harness'",
            [],
        )
        .unwrap();
        let counts: (i64, i64) = conn
            .query_row(
                "SELECT
                    (SELECT COUNT(*) FROM skill_versions),
                    (SELECT COUNT(*) FROM skill_files)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(counts, (18, 18));
        drop(conn);

        // Opening twice verifies both the upgrade and its idempotency.
        for _ in 0..2 {
            let store = SqliteStore::new(&db_path).unwrap();
            store
                .with_connection(|conn| {
                    let latest: u32 =
                        conn.query_row("SELECT MAX(version) FROM _migrations", [], |row| {
                            row.get(0)
                        })?;
                    assert_eq!(latest, MIGRATIONS.last().unwrap().version);
                    for (skill_id, version_id, expected_content) in [
                        (
                            "skill-book-ingest",
                            "skill-version-book-ingest-v3",
                            include_str!("../../skills/book-ingest/SKILL.md"),
                        ),
                        (
                            "skill-book-query",
                            "skill-version-book-query-v3",
                            include_str!("../../skills/book-query/SKILL.md"),
                        ),
                        (
                            "skill-book-research",
                            "skill-version-book-research-v3",
                            include_str!("../../skills/book-research/SKILL.md"),
                        ),
                        (
                            "skill-book-presentation",
                            "skill-version-book-presentation-v3",
                            include_str!("../../skills/book-presentation/SKILL.md"),
                        ),
                    ] {
                        let (
                            current,
                            revision,
                            release_state,
                            version_hash,
                            file_hash,
                            content,
                            size,
                        ): (
                            String,
                            i64,
                            String,
                            String,
                            String,
                            String,
                            i64,
                        ) = conn.query_row(
                            "SELECT s.current_version_id, v.revision, v.release_state,
                                    v.content_hash, f.content_hash, f.content_text, f.size_bytes
                             FROM skills s
                             JOIN skill_versions v ON v.id = ?2 AND v.skill_id = s.id
                             JOIN skill_files f ON f.skill_version_id = v.id
                             WHERE s.id = ?1 AND f.relative_path = 'SKILL.md'",
                            params![skill_id, version_id],
                            |row| {
                                Ok((
                                    row.get(0)?,
                                    row.get(1)?,
                                    row.get(2)?,
                                    row.get(3)?,
                                    row.get(4)?,
                                    row.get(5)?,
                                    row.get(6)?,
                                ))
                            },
                        )?;
                        let expected_hash =
                            hex::encode(Sha256::digest(expected_content.as_bytes()));
                        assert_eq!(current, version_id);
                        assert_eq!(revision, 3);
                        assert_eq!(release_state, "published");
                        assert_eq!(version_hash, expected_hash);
                        assert_eq!(file_hash, expected_hash);
                        assert_eq!(content, expected_content);
                        assert_eq!(size, expected_content.len() as i64);
                    }
                    let counts: (i64, i64) = conn.query_row(
                        "SELECT
                            (SELECT COUNT(*) FROM skill_versions),
                            (SELECT COUNT(*) FROM skill_files)",
                        [],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )?;
                    assert_eq!(counts, (7, 7));
                    let stale_quality_runs: i64 = conn.query_row(
                        "SELECT
                            (SELECT COUNT(*) FROM skill_evaluation_runs
                             WHERE id = 'stale-evaluation') +
                            (SELECT COUNT(*) FROM skill_benchmark_runs
                             WHERE id = 'stale-benchmark')",
                        [],
                        |row| row.get(0),
                    )?;
                    assert_eq!(stale_quality_runs, 0);
                    let migrated_provider: (String, String, String, String, String, String) = conn
                        .query_row(
                            "SELECT p.id, p.display_name, p.model, p.credential_source,
                                    p.api_key_env, r.provider_id
                             FROM llm_provider_profiles p
                             JOIN agent_runtime_profiles r ON r.provider_id = p.id
                             WHERE r.id = 'runtime-deepseek-harness'",
                            [],
                            |row| {
                                Ok((
                                    row.get(0)?,
                                    row.get(1)?,
                                    row.get(2)?,
                                    row.get(3)?,
                                    row.get(4)?,
                                    row.get(5)?,
                                ))
                            },
                        )?;
                    assert_eq!(
                        migrated_provider,
                        (
                            "aliyun-bailian".to_string(),
                            "阿里云百炼".to_string(),
                            "glm-5.2".to_string(),
                            "environment".to_string(),
                            "CUSTOM_LLM_API_KEY".to_string(),
                            "aliyun-bailian".to_string(),
                        )
                    );
                    Ok(())
                })
                .unwrap();
        }
    }

    #[test]
    fn test_migration_042_refreshes_stock_citation_contract_without_changing_bookshelf() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("citation-skill-upgrade.db");
        let store = SqliteStore::new(&db_path).unwrap();
        store.with_connection(|conn| {
            conn.execute("DELETE FROM _migrations WHERE version >= 42", [])?;
            conn.execute("UPDATE skill_files SET content_text='old stock citation rules', content_hash='old' WHERE skill_version_id='skill-version-book-query-v3'", [])?;
            conn.execute("UPDATE skill_versions SET content_hash='old' WHERE id='skill-version-book-query-v3'", [])?;
            conn.execute("INSERT INTO app_state (key,value) VALUES ('citation-upgrade-books','original')", [])?;
            Ok(())
        }).unwrap();
        drop(store);
        let reopened = SqliteStore::new(&db_path).unwrap();
        reopened.with_connection(|conn| {
            let body: String = conn.query_row("SELECT content_text FROM skill_files WHERE skill_version_id='skill-version-book-query-v3' AND relative_path='SKILL.md'", [], |row|row.get(0))?;
            assert!(body.contains("citation.label"));
            let state: String = conn.query_row("SELECT value FROM app_state WHERE key='citation-upgrade-books'", [], |row|row.get(0))?;
            assert_eq!(state, "original");
            Ok(())
        }).unwrap();
    }

    #[test]
    fn test_migrations_seed_versioned_safe_instruction_and_artifact_skills() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("skills.db");
        let store = SqliteStore::new(&db_path).unwrap();

        store
            .with_connection(|conn| {
                let counts: (i64, i64, i64) = conn.query_row(
                    "SELECT
                        (SELECT COUNT(*) FROM skills WHERE source_type = 'builtin'),
                        (SELECT COUNT(*) FROM skill_versions),
                        (SELECT COUNT(*) FROM skill_files WHERE relative_path = 'SKILL.md')",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )?;
                assert_eq!(counts, (7, 7, 7));
                let candidates: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM skill_versions WHERE release_state = 'candidate'",
                    [],
                    |row| row.get(0),
                )?;
                assert_eq!(candidates, 0);
                let benchmark_counts: (i64, i64, i64, i64) = conn.query_row(
                    "SELECT
                        (SELECT COUNT(*) FROM skill_benchmark_cases WHERE suite_id = 'skill-suite-ingest'),
                        (SELECT COUNT(*) FROM skill_benchmark_cases WHERE suite_id = 'skill-suite-query'),
                        (SELECT COUNT(*) FROM skill_benchmark_cases WHERE suite_id = 'skill-suite-research'),
                        (SELECT COUNT(*) FROM skill_benchmark_cases WHERE suite_id = 'skill-suite-presentation')",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )?;
                assert_eq!(benchmark_counts, (12, 8, 6, 6));
                let current: String = conn.query_row(
                    "SELECT current_version_id FROM skills WHERE id = 'skill-book-query'",
                    [],
                    |row| row.get(0),
                )?;
                assert_eq!(current, "skill-version-book-query-v3");
                let presentation: String = conn.query_row(
                    "SELECT current_version_id FROM skills WHERE id = 'skill-book-presentation'",
                    [],
                    |row| row.get(0),
                )?;
                assert_eq!(presentation, "skill-version-book-presentation-v3");
                let synthesis: String = conn.query_row(
                    "SELECT current_version_id FROM skills WHERE id = 'skill-book-synthesis'",
                    [],
                    |row| row.get(0),
                )?;
                assert_eq!(synthesis, "skill-version-book-synthesis-v2");
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn test_migrations_publish_detailed_ingest_skill_as_current_version() {
        let dir = TempDir::new().unwrap();
        let store = SqliteStore::new(&dir.path().join("detailed-ingest.db")).unwrap();

        store
            .with_connection(|conn| {
                let (current_version, release_state, content, content_hash, size_bytes, source_ref):
                    (String, String, String, String, i64, String) = conn.query_row(
                    "SELECT s.current_version_id, sv.release_state, f.content_text,
                            f.content_hash, f.size_bytes, o.source_ref
                     FROM skills s
                     JOIN skill_versions sv ON sv.skill_id = s.id
                     JOIN skill_files f ON f.skill_version_id = sv.id AND f.relative_path = 'SKILL.md'
                     JOIN skill_version_origins o ON o.skill_version_id = sv.id
                     WHERE s.id = 'skill-book-ingest' AND sv.revision = 3",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
                )?;
                assert_eq!(current_version, "skill-version-book-ingest-v3");
                assert_eq!(release_state, "published");
                assert_eq!(content, include_str!("../../skills/book-ingest/SKILL.md"));
                assert_eq!(content_hash, hex::encode(Sha256::digest(content.as_bytes())));
                assert_eq!(size_bytes, content.len() as i64);
                assert!(content.chars().count() > 1_500);
                for section in [
                    "## 运行边界",
                    "## 一、提取证据，再确定主题",
                    "## 二、确定身份与变化",
                    "## 四、输出协议与长度控制",
                    "## 提交前逐项检查",
                ] {
                    assert!(content.contains(section), "missing section: {section}");
                }
                assert_eq!(source_ref.len(), 40);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn test_migration_030_publishes_all_detailed_builtin_skill_files() {
        let dir = TempDir::new().unwrap();
        let store = SqliteStore::new(&dir.path().join("detailed-skills.db")).unwrap();
        let expected = [
            (
                "book-ingest",
                3,
                include_str!("../../skills/book-ingest/SKILL.md"),
            ),
            (
                "book-query",
                3,
                include_str!("../../skills/book-query/SKILL.md"),
            ),
            (
                "book-research",
                3,
                include_str!("../../skills/book-research/SKILL.md"),
            ),
            (
                "book-presentation",
                3,
                include_str!("../../skills/book-presentation/SKILL.md"),
            ),
            (
                "book-lint",
                2,
                include_str!("../../skills/book-lint/SKILL.md"),
            ),
            (
                "book-synthesis",
                2,
                include_str!("../../skills/book-synthesis/SKILL.md"),
            ),
            (
                "markdown-collection",
                2,
                include_str!("../../skills/markdown-collection/SKILL.md"),
            ),
        ];
        store
            .with_connection(|conn| {
                for (slug, revision, expected_content) in expected {
                    let (current_version, version_id, release_state, content, content_hash, size_bytes):
                        (String, String, String, String, String, i64) = conn.query_row(
                        "SELECT s.current_version_id, sv.id, sv.release_state, sf.content_text,
                                sf.content_hash, sf.size_bytes
                         FROM skills s
                         JOIN skill_versions sv ON sv.skill_id = s.id AND sv.revision = ?2
                         JOIN skill_files sf ON sf.skill_version_id = sv.id AND sf.relative_path = 'SKILL.md'
                         WHERE s.slug = ?1",
                        params![slug, revision],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
                    )?;
                    assert_eq!(current_version, version_id, "{slug} should be active");
                    assert_eq!(release_state, "published");
                    assert_eq!(content, expected_content);
                    assert_eq!(content_hash, hex::encode(Sha256::digest(content.as_bytes())));
                    assert_eq!(size_bytes, content.len() as i64);
                    assert!(content.chars().count() > 700, "{slug} is still too terse");
                    assert!(content.chars().count() <= 3_000 || slug == "book-ingest",
                        "{slug} will be truncated by the runtime prompt");
                }
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn test_migration_030_keeps_existing_user_skill_and_app_state() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("upgrade-from-029.db");
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "PRAGMA foreign_keys=ON;
             CREATE TABLE _migrations (
                 version INTEGER PRIMARY KEY,
                 description TEXT NOT NULL,
                 applied_at DATETIME DEFAULT CURRENT_TIMESTAMP
             );",
        )
        .unwrap();
        for migration in MIGRATIONS.iter().filter(|migration| migration.version < 30) {
            conn.execute_batch(migration.sql).unwrap();
            if migration.version == 29 {
                seed_detailed_ingest_skill(&conn).unwrap();
            }
            conn.execute(
                "INSERT INTO _migrations (version, description) VALUES (?1, ?2)",
                params![migration.version, migration.description],
            )
            .unwrap();
        }
        conn.execute(
            "INSERT INTO app_state (key, value) VALUES ('test-user-state', ?1)",
            params!["{\"books\":[\"user-data\"]}"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO skills
                (id, slug, name, source_type, status, current_version_id, created_at, updated_at)
             VALUES ('skill-user-review', 'user-review', '用户 Skill', 'custom', 'ready',
                     'skill-version-user-review-v1', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            [],
        )
        .unwrap();
        let custom_content = "我的自定义审校规则";
        let custom_hash = hex::encode(Sha256::digest(custom_content.as_bytes()));
        conn.execute(
            "INSERT INTO skill_versions (id, skill_id, revision, content_hash, release_state, created_at)
             VALUES ('skill-version-user-review-v1', 'skill-user-review', 1, ?1,
                     'published', CURRENT_TIMESTAMP)",
            params![custom_hash],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO skill_files
                (skill_version_id, relative_path, media_type, content_text, content_hash, size_bytes)
             VALUES ('skill-version-user-review-v1', 'SKILL.md', 'text/markdown', ?1, ?2, ?3)",
            params![custom_content, custom_hash, custom_content.len() as i64],
        )
        .unwrap();
        drop(conn);

        let store = SqliteStore::new(&db_path).unwrap();
        store
            .with_connection(|conn| {
                let state: String = conn.query_row(
                    "SELECT value FROM app_state WHERE key = 'test-user-state'",
                    [],
                    |row| row.get(0),
                )?;
                assert_eq!(state, "{\"books\":[\"user-data\"]}");
                let custom: (String, String) = conn.query_row(
                    "SELECT s.current_version_id, f.content_text FROM skills s
                     JOIN skill_files f ON f.skill_version_id = s.current_version_id
                     WHERE s.id = 'skill-user-review' AND f.relative_path = 'SKILL.md'",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )?;
                assert_eq!(custom.0, "skill-version-user-review-v1");
                assert_eq!(custom.1, "我的自定义审校规则");
                let ingest: String = conn.query_row(
                    "SELECT current_version_id FROM skills WHERE id = 'skill-book-ingest'",
                    [],
                    |row| row.get(0),
                )?;
                assert_eq!(ingest, "skill-version-book-ingest-v3");
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn test_migration_016_backfills_compile_state_and_message_evidence_snapshot() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("upgrade.db");
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE _migrations (
                version INTEGER PRIMARY KEY,
                description TEXT NOT NULL,
                applied_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );",
        )
        .unwrap();
        for migration in MIGRATIONS.iter().filter(|migration| migration.version < 16) {
            conn.execute_batch(migration.sql).unwrap();
            conn.execute(
                "INSERT INTO _migrations (version, description) VALUES (?1, ?2)",
                params![migration.version, migration.description],
            )
            .unwrap();
        }
        conn.execute_batch(
            "INSERT INTO reader_books
                (id, path, kind, name, description, category, added_at)
             VALUES ('book-upgrade', '/tmp/book-upgrade', 'folder', '升级书籍', '', '', 1);
             INSERT INTO knowledge_bases
                (id, book_id, lifecycle, sync_state, health_state, created_at, updated_at)
             VALUES ('base-upgrade', 'book-upgrade', 'active', 'clean', 'healthy',
                     CURRENT_TIMESTAMP, CURRENT_TIMESTAMP);
             INSERT INTO source_documents
                (id, knowledge_base_id, source_type, original_path, relative_path, title,
                 mime_type, current_version_id, created_at, updated_at)
             VALUES ('source-upgrade', 'base-upgrade', 'markdown', '/tmp/book-upgrade/a.md',
                     'a.md', '旧来源', 'text/markdown', 'version-upgrade',
                     CURRENT_TIMESTAMP, CURRENT_TIMESTAMP);
             INSERT INTO source_versions
                (id, source_document_id, content_hash, size_bytes, extraction_version,
                 extraction_status, created_at)
             VALUES ('version-upgrade', 'source-upgrade', 'hash', 10, 'markdown-v1',
                     'ready', CURRENT_TIMESTAMP);
             INSERT INTO source_spans
                (id, knowledge_base_id, source_version_id, ordinal, content, content_hash)
             VALUES ('span-upgrade', 'base-upgrade', 'version-upgrade', 0, '旧正文', 'hash');
             INSERT INTO knowledge_entries
                (id, knowledge_base_id, origin_document_id, entry_type, slug, title,
                 summary, content_md, status, revision, created_at, updated_at)
             VALUES ('entry-upgrade', 'base-upgrade', 'source-upgrade', 'source_section',
                     'old-entry', '旧标题', '旧摘要', '旧正文', 'verified', 3,
                     CURRENT_TIMESTAMP, CURRENT_TIMESTAMP);
             INSERT INTO knowledge_conversations (id, title, created_at, updated_at)
             VALUES ('conversation-upgrade', '历史问答', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP);
             INSERT INTO knowledge_conversation_scopes
                (conversation_id, knowledge_base_id, ordinal)
             VALUES ('conversation-upgrade', 'base-upgrade', 0);
             INSERT INTO knowledge_messages
                (id, conversation_id, ordinal, role, content, created_at)
             VALUES ('message-upgrade', 'conversation-upgrade', 0, 'assistant', '历史回答',
                     CURRENT_TIMESTAMP);
             INSERT INTO knowledge_message_citations (message_id, ordinal, entry_id)
             VALUES ('message-upgrade', 0, 'entry-upgrade');",
        )
        .unwrap();
        drop(conn);

        let store = SqliteStore::new(&db_path).unwrap();
        store
            .with_connection(|conn| {
                let compile: (String, String) = conn.query_row(
                    "SELECT compile_mode, compile_state FROM knowledge_bases
                     WHERE id = 'base-upgrade'",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )?;
                assert_eq!(compile, ("chapter".to_string(), "not_started".to_string()));
                let snapshot: (i64, String, String) = conn.query_row(
                    "SELECT entry_revision, title_snapshot, summary_snapshot
                     FROM knowledge_message_citations WHERE message_id = 'message-upgrade'",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )?;
                assert_eq!(snapshot, (3, "旧标题".to_string(), "旧摘要".to_string()));
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn test_app_state_crud() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let store = SqliteStore::new(&db_path).unwrap();

        assert_eq!(store.get_state("missing").unwrap(), None);

        store.set_state("test_key", "hello").unwrap();
        assert_eq!(
            store.get_state("test_key").unwrap(),
            Some("hello".to_string())
        );

        store.set_state("test_key", "world").unwrap();
        assert_eq!(
            store.get_state("test_key").unwrap(),
            Some("world".to_string())
        );
    }

    #[test]
    fn test_tables_exist() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let store = SqliteStore::new(&db_path).unwrap();
        let conn = store.conn.lock().unwrap();

        for table in &[
            "code_repos",
            "note_repo_links",
            "radar_items",
            "inspiration_history",
            "timeline_events",
            "app_state",
            "memos",
            "knowledge_conversations",
            "knowledge_conversation_scopes",
            "knowledge_messages",
            "knowledge_message_citations",
            "skills",
            "skill_versions",
            "skill_files",
            "knowledge_base_skill_bindings",
            "agent_run_events",
            "knowledge_compile_checkpoints",
            "knowledge_entries_fts",
            "source_spans_fts",
            "agent_run_capabilities",
            "agent_run_capability_scopes",
        ] {
            let exists: bool = conn
                .query_row(
                    "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name=?1",
                    params![table],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(exists, "Table {table} should exist");
        }
    }

    /// 002 用 `source` 建了 radar_items，007 的 CREATE TABLE IF NOT EXISTS 因此失效，
    /// `source_name` / `saved_path` 从未被创建，而读写代码一直在用 `source_name`。
    #[test]
    fn test_radar_items_exposes_source_name_and_saved_path() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let store = SqliteStore::new(&db_path).unwrap();
        let conn = store.conn.lock().unwrap();

        for column in &["source_name", "saved_path"] {
            let count: u32 = conn
                .query_row(
                    "SELECT COUNT(*) FROM pragma_table_info('radar_items') WHERE name=?1",
                    params![column],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 1, "radar_items 应包含列 {column}");
        }
    }

    #[test]
    fn test_insert_and_get_radar_item_round_trips_source_name() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let store = SqliteStore::new(&db_path).unwrap();

        store
            .insert_radar_item("r1", "标题", "摘要", "hackernews", "https://example.com/a")
            .unwrap();

        let items = store.get_radar_items("new", 10).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].0, "r1");
        assert_eq!(items[0].3, "hackernews");
    }

    #[test]
    fn test_online_backup_is_valid_and_preserves_wal_state() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("backup-source.db");
        let store = SqliteStore::new(&db_path).unwrap();
        store.set_state("backup-marker", "snapshot-value").unwrap();

        let backup = store.create_managed_backup("manual", 7).unwrap();
        let backups = store.list_managed_backups().unwrap();
        assert_eq!(backups.len(), 1);
        assert_eq!(backups[0], backup);
        assert!(backup.size_bytes > 0);
        assert_eq!(backup.reason, "manual");
        let report = SqliteStore::validate_database_file(&backup.path).unwrap();
        assert!(report.integrity_ok);
        assert_eq!(report.foreign_key_violations, 0);
        assert_eq!(report.migration_version, MIGRATIONS.last().unwrap().version);

        let snapshot = SqliteStore::new(&backup.path).unwrap();
        assert_eq!(
            snapshot.get_state("backup-marker").unwrap().as_deref(),
            Some("snapshot-value")
        );
    }

    #[test]
    fn test_restore_replaces_live_database_and_keeps_safety_backup() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("restore-source.db");
        let store = SqliteStore::new(&db_path).unwrap();
        store.set_state("restore-marker", "before").unwrap();
        let desired = store.create_managed_backup("manual", 7).unwrap();
        store.set_state("restore-marker", "after").unwrap();

        let result = store
            .restore_database_file(&desired.path, "RESTORE", 7)
            .unwrap();

        assert_eq!(
            store.get_state("restore-marker").unwrap().as_deref(),
            Some("before")
        );
        assert!(result.integrity_ok);
        assert!(store
            .list_managed_backups()
            .unwrap()
            .iter()
            .any(|backup| backup.reason == "pre-restore"));
    }

    #[test]
    fn test_backup_retention_prunes_only_old_managed_snapshots() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("retention-source.db");
        let store = SqliteStore::new(&db_path).unwrap();

        for index in 0..4 {
            store
                .set_state("retention-marker", &index.to_string())
                .unwrap();
            store
                .create_managed_backup(&format!("manual-{index}"), 2)
                .unwrap();
        }

        let backups = store.list_managed_backups().unwrap();
        assert_eq!(backups.len(), 2);
        assert!(backups.iter().all(|backup| backup.path.exists()));
    }

    #[test]
    fn test_restore_rejects_while_agent_work_is_active() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("active-run.db");
        let store = SqliteStore::new(&db_path).unwrap();
        let desired = store.create_managed_backup("manual", 7).unwrap();
        store
            .with_connection(|conn| {
                conn.execute(
                    "INSERT INTO agent_runs
                        (id, runtime, task_type, status, input_json, created_at)
                     VALUES ('active-run', 'deepseek_harness', 'knowledge_qa', 'running', '{}',
                             CURRENT_TIMESTAMP)",
                    [],
                )?;
                Ok(())
            })
            .unwrap();

        let error = store
            .restore_database_file(&desired.path, "RESTORE", 7)
            .unwrap_err();
        assert!(error.to_string().contains("运行中"));
    }

    #[test]
    fn test_managed_backup_path_rejects_traversal_and_unknown_files() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("secure-path.db");
        let store = SqliteStore::new(&db_path).unwrap();
        let backup = store.create_managed_backup("manual", 7).unwrap();

        assert_eq!(
            store.managed_backup_path(&backup.filename).unwrap(),
            backup.path
        );
        assert!(store.managed_backup_path("../secure-path.db").is_err());
        assert!(store.managed_backup_path("unknown.sqlite3").is_err());
    }

    #[test]
    fn test_restore_rolls_back_when_post_restore_migration_fails() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("rollback-live.db");
        let store = SqliteStore::new(&db_path).unwrap();
        store.set_state("rollback-marker", "preserved").unwrap();
        let incompatible_path = dir.path().join("incomplete.db");
        let incompatible = Connection::open(&incompatible_path).unwrap();
        incompatible
            .execute_batch(
                "CREATE TABLE _migrations (
                    version INTEGER PRIMARY KEY,
                    description TEXT NOT NULL,
                    applied_at TEXT
                 );
                 INSERT INTO _migrations (version, description) VALUES (20, 'incomplete');",
            )
            .unwrap();
        drop(incompatible);

        let error = store
            .restore_database_file(&incompatible_path, "RESTORE", 7)
            .unwrap_err();

        assert!(error.to_string().contains("已回滚"));
        assert_eq!(
            store.get_state("rollback-marker").unwrap().as_deref(),
            Some("preserved")
        );
    }
}
