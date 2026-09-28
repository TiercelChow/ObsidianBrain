use super::*;
use crate::infra::book_wiki_store::BookWikiStore;
use crate::models::book_wiki::{BookKind, ReaderBook};
use rusqlite::types::Value;

const PRESERVED_TABLES: &[&str] = &[
    "reader_books",
    "knowledge_tasks",
    "agent_runs",
    "knowledge_research_workspaces",
    "knowledge_research_stages",
    "knowledge_research_stage_versions",
    "knowledge_research_baselines",
];

fn snapshot(conn: &Connection) -> Vec<Vec<Vec<Value>>> {
    PRESERVED_TABLES
        .iter()
        .map(|table| {
            let mut statement = conn
                .prepare(&format!("SELECT * FROM {table} ORDER BY 1,2"))
                .unwrap();
            let columns = statement.column_count();
            statement
                .query_map([], |row| {
                    (0..columns).map(|column| row.get(column)).collect()
                })
                .unwrap()
                .collect::<Result<Vec<Vec<Value>>, _>>()
                .unwrap()
        })
        .collect()
}

fn assert_research_upgrade_preserves_data(legacy_constraint: bool) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("upgrade-v51.db");
    let db = Arc::new(SqliteStore::new(&path).unwrap());
    // Reproduce both deployed v51 schemas without touching a real database.
    db.with_connection(|conn| {
        conn.execute_batch(
            "DROP TABLE knowledge_research_stage_versions;
             DROP TABLE knowledge_research_stages;
             DELETE FROM _migrations WHERE version > 51;",
        )?;
        let mut schema =
            include_str!("../../migrations/051_research_stage_checkpoints.sql").to_owned();
        if legacy_constraint {
            schema = schema.replace("'section','synthesis',", "'section',");
        }
        conn.execute_batch(&schema)?;
        Ok(())
    })
    .unwrap();
    let store = BookWikiStore::new(db.clone());
    store
        .save_reader_books(&[ReaderBook {
            id: "upgrade-book".into(),
            path: dir.path().display().to_string(),
            kind: BookKind::Folder,
            name: "已有书架".into(),
            description: "保留书籍与研究成果".into(),
            category: "测试".into(),
            added_at: 1,
            progress: None,
        }])
        .unwrap();
    let base = store.initialize_base("upgrade-book").unwrap();
    let task = store
        .create_task(&base.id, "已有研究", "已有目标", "research")
        .unwrap();
    let run = store
        .start_agent_run(
            &base.id,
            "deepseek_harness",
            "knowledge_task_research",
            &serde_json::json!({"knowledge_task_id":task.id}),
        )
        .unwrap();
    db.with_connection(|conn| {
        conn.execute(
            r#"INSERT INTO knowledge_research_workspaces VALUES (?1,?2,'{"goal":"已有目标"}','request-hash','{"goal":"已保存规划"}','created','updated')"#,
            params![task.id, base.id],
        )?;
        for (ordinal, key, kind) in [(0, "plan", "plan"), (1, "section:existing", "section")] {
            conn.execute(
                r#"INSERT INTO knowledge_research_stages VALUES (?1,?2,'已有阶段',?3,?4,'completed',3,'claim-id',7,?5,?5,'已有摘要','已有正文 $$d_k=128$$','[{"finding":"条件"}]','[{"run_id":"历史来源"}]',NULL,'updated')"#,
                params![task.id,key,kind,ordinal,run.id],
            )?;
            for revision in [1,2] {
                conn.execute(r#"INSERT INTO knowledge_research_stage_versions VALUES (?1,?2,?3,'{"content_md":"历史正文"}','created')"#,params![task.id,key,revision])?;
            }
        }
        conn.execute(r#"INSERT INTO knowledge_research_baselines VALUES (?1,'existing','entry-id','{"content_md":"冻结基线"}','created')"#,[&task.id])?;
        if !legacy_constraint {
            conn.execute("INSERT INTO knowledge_research_stages(task_id,stage_key,title,kind,ordinal,status,content_md,updated_at) VALUES (?1,'synthesis','已有综合','synthesis',2,'completed','已有综合正文','updated')",[&task.id])?;
        }
        Ok(())
    }).unwrap();
    let before = db.with_connection(|conn| Ok(snapshot(conn))).unwrap();
    // Even a failure after both tables were rebuilt must restore the old data
    // and constraint, not merely report failure after dropping the snapshots.
    let migration = MIGRATIONS
        .iter()
        .find(|migration| migration.version == 52)
        .unwrap();
    let rollback: Result<(), BrainError> = db.transaction(|conn| {
        conn.execute_batch(migration.sql)?;
        Err(BrainError::Internal("模拟提交前故障".into()))
    });
    assert!(rollback.is_err());
    db.with_connection(|conn| {
        assert_eq!(snapshot(conn), before);
        let schema: String = conn.query_row(
            "SELECT sql FROM sqlite_master WHERE name='knowledge_research_stages'",
            [],
            |row| row.get(0),
        )?;
        assert_eq!(schema.contains("'synthesis'"), !legacy_constraint);
        let version: u32 =
            conn.query_row("SELECT MAX(version) FROM _migrations", [], |row| row.get(0))?;
        assert_eq!(version, 51);
        Ok(())
    })
    .unwrap();
    drop(store);
    drop(db);

    let upgraded = SqliteStore::new(&path).unwrap();
    upgraded.with_connection(|conn| {
        assert_eq!(snapshot(conn), before, "升级必须完整保留每个字段和历史版本");
        conn.execute("INSERT INTO knowledge_research_stages(task_id,stage_key,title,kind,ordinal,status,updated_at) VALUES (?1,'new-synthesis','综合','synthesis',3,'pending','updated')",[&task.id])?;
        let version:u32=conn.query_row("SELECT MAX(version) FROM _migrations",[],|row|row.get(0))?;
        assert!(version >= 52);
        assert!(conn.execute("INSERT INTO knowledge_research_stages(task_id,stage_key,title,kind,ordinal,status,updated_at) VALUES (?1,'invalid','错误','unknown',4,'pending','updated')",[&task.id]).is_err());
        let violations:u32=conn.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check",[],|row|row.get(0))?;
        assert_eq!(violations,0);
        let ordered_index:u32=conn.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_research_stages_task_order'",[],|row|row.get(0))?;
        assert_eq!(ordered_index,1);
        Ok(())
    }).unwrap();
    let backups = upgraded.list_managed_backups().unwrap();
    assert_eq!(backups.len(), 1);
    assert_eq!(backups[0].reason, "pre-migration");
    let backup =
        Connection::open_with_flags(&backups[0].path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    assert_eq!(snapshot(&backup), before);
    drop(backup);
    drop(upgraded);

    let reopened = SqliteStore::new(&path).unwrap();
    assert_eq!(reopened.list_managed_backups().unwrap(), backups);
    reopened.with_connection(|conn| {
        conn.execute("DELETE FROM agent_runs WHERE id=?1",[&run.id])?;
        let null_runs:u32=conn.query_row("SELECT COUNT(*) FROM knowledge_research_stages WHERE task_id=?1 AND run_id IS NULL AND content_run_id IS NULL",[&task.id],|row|row.get(0))?;
        assert_eq!(null_runs,if legacy_constraint {3} else {4});
        conn.execute("DELETE FROM knowledge_research_workspaces WHERE task_id=?1",[&task.id])?;
        for table in ["knowledge_research_stages","knowledge_research_stage_versions","knowledge_research_baselines"] {
            let count:u32=conn.query_row(&format!("SELECT COUNT(*) FROM {table}"),[],|row|row.get(0))?;
            assert_eq!(count,0,"外键级联仍必须生效：{table}");
        }
        Ok(())
    }).unwrap();
}

#[test]
fn test_upgrade_v51_legacy_research_constraint_preserves_versions_and_allows_synthesis() {
    assert_research_upgrade_preserves_data(true);
}

#[test]
fn test_upgrade_v51_with_synthesis_preserves_existing_results_and_foreign_keys() {
    assert_research_upgrade_preserves_data(false);
}
