//! Bounded, read-only dashboard projections. Never inspect files or runtime configuration.
use crate::error::BrainError;
use crate::infra::sqlite_store::SqliteStore;
use crate::models::home::*;
use chrono::{Datelike, Duration, NaiveDate, Utc};
use rusqlite::{params, Connection, OptionalExtension};

pub fn overview(db: &SqliteStore, today: NaiveDate) -> Result<HomeOverview, BrainError> {
    let week_start = today - Duration::days(today.weekday().num_days_from_monday().into());
    db.with_connection(|conn| {
        Ok(HomeOverview {
            today: today.to_string(),
            week_start: week_start.to_string(),
            generated_at: Utc::now().to_rfc3339(),
            tasks: section("任务", tasks(conn, today)),
            reading: section("最近阅读", reading(conn)),
            memos: section("小记", memos(conn, week_start, today)),
            wiki: section("Wiki", wiki(conn)),
            storage: section("图片存储", storage(conn)),
        })
    })
}

fn section<T>(name: &str, result: Result<T, BrainError>) -> HomeSection<T> {
    match result {
        Ok(data) => HomeSection {
            data: Some(data),
            error: None,
        },
        Err(error) => {
            tracing::warn!(section = name, %error, "首页汇总读取失败");
            HomeSection {
                data: None,
                error: Some(format!("{name}暂时无法读取，请重试")),
            }
        }
    }
}

fn tasks(conn: &Connection, today: NaiveDate) -> Result<HomeTasks, BrainError> {
    let day = today.to_string();
    let (active_count, overdue_count, today_count) = conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(end_date < ?1),0), COALESCE(SUM(start_date = ?1 OR end_date = ?1),0)
         FROM task_nodes WHERE role='root' AND archived_at IS NULL AND status NOT IN ('completed','cancelled')", [&day],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
    // Child risks are attached to their parent, not counted again in attention totals.
    let mut stmt = conn.prepare(
        "SELECT r.id, substr(r.title,1,200), r.kind, r.status, r.importance, r.start_date, r.end_date, r.progress_percent,
          (SELECT c.id FROM task_nodes c WHERE c.root_id=r.id AND c.role='subtask'
           AND c.status NOT IN ('completed','cancelled') AND c.archived_at IS NULL AND (c.end_date < ?1 OR c.status='blocked')
           ORDER BY c.end_date < ?1 DESC,c.end_date,c.id LIMIT 1) AS risk_id,
          (SELECT substr(title,1,200) FROM task_nodes WHERE id=(SELECT c.id FROM task_nodes c WHERE c.root_id=r.id AND c.role='subtask'
           AND c.status NOT IN ('completed','cancelled') AND c.archived_at IS NULL AND (c.end_date < ?1 OR c.status='blocked')
           ORDER BY c.end_date < ?1 DESC,c.end_date,c.id LIMIT 1))
         FROM task_nodes r WHERE r.role='root' AND r.archived_at IS NULL AND r.status NOT IN ('completed','cancelled')
         ORDER BY r.end_date < ?1 DESC, (r.status='blocked' OR EXISTS(SELECT 1 FROM task_nodes c WHERE c.root_id=r.id AND c.role='subtask' AND c.status NOT IN ('completed','cancelled') AND c.archived_at IS NULL AND (c.end_date < ?1 OR c.status='blocked'))) DESC,
           (r.end_date=?1 OR r.start_date=?1) DESC, r.end_date,
           CASE r.importance WHEN 'urgent' THEN 0 WHEN 'high' THEN 1 WHEN 'normal' THEN 2 ELSE 3 END,r.id LIMIT 4")?;
    let items = stmt
        .query_map([day], |r| {
            Ok(HomeTask {
                id: r.get(0)?,
                title: r.get(1)?,
                kind: r.get(2)?,
                status: r.get(3)?,
                importance: r.get(4)?,
                start_date: r.get(5)?,
                end_date: r.get(6)?,
                progress_percent: r.get(7)?,
                child_risk_id: r.get(8)?,
                child_risk_title: r.get(9)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(HomeTasks {
        active_count,
        overdue_count,
        today_count,
        items,
    })
}

fn reading(conn: &Connection) -> Result<Vec<HomeBook>, BrainError> {
    let mut stmt = conn.prepare(
        "SELECT id,substr(name,1,200),kind,json_extract(progress_json,'$.lastFile'),json_extract(progress_json,'$.position'),json_extract(progress_json,'$.pageCount'),json_extract(progress_json,'$.updatedAt')
         FROM reader_books WHERE shelf_state='active' AND json_valid(progress_json)
           AND CASE WHEN json_valid(progress_json) THEN json_type(progress_json,'$.updatedAt') IN ('integer','real') AND json_extract(progress_json,'$.updatedAt')>0 AND json_type(progress_json,'$.position') IN ('integer','real') ELSE 0 END
         ORDER BY json_extract(progress_json,'$.updatedAt') DESC,id LIMIT 4")?;
    let books = stmt
        .query_map([], |r| {
            Ok(HomeBook {
                id: r.get(0)?,
                name: r.get(1)?,
                kind: r.get(2)?,
                last_file: r.get(3)?,
                position: r.get(4)?,
                page_count: r.get(5)?,
                read_at: r.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(books)
}

fn memos(conn: &Connection, week: NaiveDate, today: NaiveDate) -> Result<HomeMemos, BrainError> {
    let (total_count, week_count) = conn.query_row(
        "SELECT COUNT(*),COALESCE(SUM(date BETWEEN ?1 AND ?2),0) FROM memos",
        params![week.to_string(), today.to_string()],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let mut stmt = conn.prepare("SELECT id,date,timestamp,substr(content,1,240),CASE WHEN json_valid(images) THEN json_extract(images,'$[0]') END, CASE WHEN json_valid(tags) THEN json_extract(tags,'$[0]') END, CASE WHEN json_valid(tags) THEN json_extract(tags,'$[1]') END, CASE WHEN json_valid(tags) THEN json_extract(tags,'$[2]') END FROM memos ORDER BY timestamp DESC,id LIMIT 3")?;
    let items = stmt
        .query_map([], |r| {
            let tags = (5..8)
                .map(|i| r.get::<_, Option<String>>(i))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .flatten()
                .map(|s| s.chars().take(40).collect())
                .collect();
            Ok(HomeMemo {
                id: r.get(0)?,
                date: r.get(1)?,
                timestamp: r.get(2)?,
                excerpt: r.get(3)?,
                thumbnail: r.get(4)?,
                tags,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(HomeMemos {
        total_count,
        week_count,
        items,
    })
}

fn wiki(conn: &Connection) -> Result<HomeWiki, BrainError> {
    // waiting_review is not an actively running compilation, even on older records.
    let compile_running: u64 = conn.query_row("SELECT COUNT(*) FROM knowledge_bases WHERE lifecycle!='archived' AND compile_state='compiling' AND compile_phase!='waiting_review'", [], |r| r.get(0))?;
    let research_running: u64 = conn.query_row("SELECT COUNT(*) FROM knowledge_tasks t JOIN knowledge_bases b ON b.id=t.knowledge_base_id WHERE b.lifecycle!='archived' AND t.status IN ('queued','running')", [], |r| r.get(0))?;
    let review_count = conn.query_row("SELECT COUNT(*) FROM knowledge_change_sets c JOIN knowledge_bases b ON b.id=c.knowledge_base_id WHERE b.lifecycle!='archived' AND c.status='proposed'", [], |r| r.get(0))?;
    let failed_count = conn.query_row("SELECT (SELECT COUNT(*) FROM knowledge_bases WHERE lifecycle!='archived' AND compile_state='failed') + (SELECT COUNT(*) FROM knowledge_tasks t JOIN knowledge_bases b ON b.id=t.knowledge_base_id WHERE b.lifecycle!='archived' AND (t.status='failed' OR t.artifact_state='failed'))", [], |r| r.get(0))?;
    let mut stmt = conn.prepare(
        "SELECT id,base_id,book_name,kind,title,status,detail,artifact_state,updated_at FROM (
         SELECT b.id AS id,b.id AS base_id,substr(r.name,1,200) AS book_name,'compile' AS kind,'知识编译' AS title,b.compile_state AS status,substr(b.compile_message,1,160) AS detail,NULL AS artifact_state,b.updated_at,
          CASE WHEN b.compile_state='compiling' THEN 0 ELSE 1 END AS priority
         FROM knowledge_bases b JOIN reader_books r ON r.id=b.book_id WHERE b.lifecycle!='archived' AND b.compile_state IN ('compiling','failed') AND b.compile_phase!='waiting_review'
         UNION ALL
         SELECT t.id,b.id,substr(r.name,1,200),'research',substr(t.title,1,200),t.status,substr(t.result_summary,1,160),t.artifact_state,t.updated_at,
          CASE WHEN t.status IN ('queued','running') THEN 0 WHEN t.status='failed' OR t.artifact_state='failed' THEN 1 ELSE 3 END
         FROM knowledge_tasks t JOIN knowledge_bases b ON b.id=t.knowledge_base_id JOIN reader_books r ON r.id=b.book_id WHERE b.lifecycle!='archived' AND t.status IN ('queued','running','failed','completed')
         UNION ALL
         SELECT c.id,b.id,substr(r.name,1,200),'review',substr(c.title,1,200),c.status,substr(c.reason,1,160),NULL,c.created_at,2
         FROM knowledge_change_sets c JOIN knowledge_bases b ON b.id=c.knowledge_base_id JOIN reader_books r ON r.id=b.book_id WHERE b.lifecycle!='archived' AND c.status='proposed'
         ) ORDER BY priority,updated_at DESC,id LIMIT 5")?;
    let items = stmt
        .query_map([], |r| {
            Ok(HomeWikiItem {
                id: r.get(0)?,
                base_id: r.get(1)?,
                book_name: r.get(2)?,
                kind: r.get(3)?,
                title: r.get(4)?,
                status: r.get(5)?,
                detail: r.get(6)?,
                artifact_state: r.get(7)?,
                updated_at: r.get(8)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(HomeWiki {
        running_count: compile_running + research_running,
        review_count,
        failed_count,
        items,
    })
}

fn storage(conn: &Connection) -> Result<HomeStorage, BrainError> {
    let limit: Option<String> = conn
        .query_row(
            "SELECT value FROM app_state WHERE key='timeline_cache_limit_bytes'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    let cache_limit_bytes = match limit {
        Some(value) => value
            .parse()
            .map_err(|_| BrainError::Internal("图片缓存容量损坏".into()))?,
        None => 256 * 1024 * 1024,
    };
    Ok(HomeStorage {
        originals_bytes: conn.query_row(
            "SELECT COALESCE(SUM(size_bytes),0) FROM timeline_images",
            [],
            |r| r.get(0),
        )?,
        cache_bytes: conn.query_row(
            "SELECT COALESCE(SUM(size_bytes),0) FROM timeline_image_cache",
            [],
            |r| r.get(0),
        )?,
        cache_limit_bytes,
        pending_cleanup: conn
            .query_row("SELECT COUNT(*) FROM timeline_image_gc", [], |r| r.get(0))?,
    })
}

#[cfg(test)]
#[path = "home_store_tests.rs"]
mod tests;
