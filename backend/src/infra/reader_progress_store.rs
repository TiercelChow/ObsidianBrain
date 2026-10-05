//! Atomic single-book reading updates. No filesystem access or book-list replacement.
use crate::{
    error::BrainError,
    infra::sqlite_store::SqliteStore,
    models::book_wiki::{BookProgress, ReaderFileKind, ReaderFileProgress, ReaderProgressState},
};
use rusqlite::{params, OptionalExtension};
use std::{collections::BTreeMap, sync::Arc};

const MAX_FILES: usize = 10_000;
const MAX_BYTES: usize = 2 * 1024 * 1024;

pub struct ReaderProgressStore {
    db: Arc<SqliteStore>,
}

fn invalid(detail: &str) -> BrainError {
    BrainError::KnowledgeValidation(format!("阅读状态：{detail}"))
}

// Lexical paths: readers also store Windows paths when the service runs on Windows.
// Never resolve symlinks or touch the source to save a reading position.
fn normalized(path: &str) -> Result<String, BrainError> {
    if path.is_empty() || path.len() > 8192 || path.contains('\0') {
        return Err(invalid("文件路径不合法"));
    }
    let path = path.replace('\\', "/");
    if path.split('/').any(|part| part == ".." || part == ".") {
        return Err(invalid("文件路径不能包含上级或当前目录跳转"));
    }
    let path = path.trim_end_matches('/').to_string();
    Ok(if path.as_bytes().get(1) == Some(&b':') {
        path.to_lowercase()
    } else {
        path
    })
}

fn belongs(path: &str, book_path: &str, kind: &str) -> Result<bool, BrainError> {
    let file = normalized(path)?;
    let root = normalized(book_path)?;
    Ok(if kind == "pdf" {
        file == root
    } else {
        file.starts_with(&format!("{root}/"))
    })
}

impl ReaderProgressStore {
    pub fn new(db: Arc<SqliteStore>) -> Self {
        Self { db }
    }

    pub fn save(
        &self,
        id: &str,
        incoming: ReaderProgressState,
    ) -> Result<ReaderProgressState, BrainError> {
        if id.is_empty()
            || id.len() > 200
            || incoming.by_file.is_empty()
            || incoming.by_file.len() > MAX_FILES
        {
            return Err(invalid("书籍 ID 或文件数量不合法"));
        }
        let max_time = chrono::Utc::now().timestamp_millis() + 5 * 60 * 1000;
        if incoming.last_read_at <= 0 || incoming.last_read_at > max_time {
            return Err(invalid("阅读时间不合法"));
        }
        self.db.with_connection(|conn| {
            let tx = conn.unchecked_transaction()?;
            let stored: Option<(String, String, Option<String>)> = tx
                .query_row(
                    "SELECT path,kind,progress_json FROM reader_books WHERE id=?1 AND shelf_state='active'",
                    [id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .optional()?;
            let (book_path, book_kind, json) =
                stored.ok_or_else(|| BrainError::KnowledgeNotFound(id.into()))?;
            for (file, progress) in &incoming.by_file {
                if !belongs(file, &book_path, &book_kind)? {
                    return Err(invalid("文件不属于这本书"));
                }
                let lower = file.to_ascii_lowercase();
                let pdf = lower.ends_with(".pdf");
                let md = lower.ends_with(".md") || lower.ends_with(".markdown");
                if (pdf && progress.kind != ReaderFileKind::Pdf)
                    || (md && progress.kind != ReaderFileKind::Md)
                    || (!pdf && !md)
                {
                    return Err(invalid("文件类型与进度类型不一致"));
                }
                if !progress.position.is_finite()
                    || (pdf && (progress.position < 1.0 || progress.position.fract() != 0.0))
                    || (md && !(0.0..=1.0).contains(&progress.position))
                {
                    return Err(invalid("阅读位置不合法"));
                }
                if progress.updated_at <= 0
                    || progress.updated_at > max_time
                    || progress.updated_at > incoming.last_read_at
                {
                    return Err(invalid("位置更新时间不合法"));
                }
                if let Some(count) = progress.page_count {
                    if !pdf || !(1..=1_000_000).contains(&count) || progress.position > count as f64 {
                        return Err(invalid("PDF 页码或总页数不合法"));
                    }
                }
            }
            if !belongs(&incoming.last_file, &book_path, &book_kind)? {
                return Err(invalid("最近阅读文件不属于这本书"));
            }
            let old: Option<BookProgress> = json
                .as_deref()
                .filter(|value| *value != "null")
                .map(serde_json::from_str)
                .transpose()
                .map_err(|e| BrainError::Internal(format!("已有阅读状态无法解析，未覆盖: {e}")))?;
            let mut state = ReaderProgressState {
                last_file: String::new(),
                last_read_at: 0,
                by_file: BTreeMap::new(),
            };
            if let Some(old) = old {
                state.last_read_at = old.last_read_at.unwrap_or(old.updated_at);
                state.last_file = old.last_file.unwrap_or_else(|| {
                    if book_kind == "pdf" { book_path.clone() } else { String::new() }
                });
                state.by_file = old.by_file;
                if !state.last_file.is_empty() && !state.by_file.contains_key(&state.last_file) {
                    let kind = if state.last_file.to_ascii_lowercase().ends_with(".pdf") {
                        ReaderFileKind::Pdf
                    } else {
                        ReaderFileKind::Md
                    };
                    state.by_file.insert(
                        state.last_file.clone(),
                        ReaderFileProgress {
                            kind,
                            position: old.position,
                            page_count: old.page_count,
                            updated_at: old.updated_at,
                        },
                    );
                }
            }
            for (file, progress) in incoming.by_file {
                if state.by_file.get(&file).is_none_or(|old| progress.updated_at >= old.updated_at) {
                    state.by_file.insert(file, progress);
                }
            }
            if !state.by_file.contains_key(&incoming.last_file) {
                return Err(invalid("最近阅读文件缺少对应进度"));
            }
            if incoming.last_read_at >= state.last_read_at || state.last_file.is_empty() {
                state.last_file = incoming.last_file;
                state.last_read_at = incoming.last_read_at;
            }
            if state.by_file.len() > MAX_FILES {
                return Err(invalid("保存文件数量超过上限，旧记录未删除"));
            }
            let current = state.by_file
                .get(&state.last_file)
                .ok_or_else(|| invalid("最近阅读文件无有效进度"))?;
            let progress = BookProgress {
                last_file: Some(state.last_file.clone()),
                position: current.position,
                page_count: current.page_count,
                updated_at: state.last_read_at,
                last_read_at: Some(state.last_read_at),
                by_file: state.by_file.clone(),
            };
            let encoded = serde_json::to_string(&progress)
                .map_err(|e| BrainError::Internal(e.to_string()))?;
            if encoded.len() > MAX_BYTES {
                return Err(invalid("阅读状态超过 2 MiB，旧记录未删除"));
            }
            tx.execute(
                "UPDATE reader_books SET progress_json=?2 WHERE id=?1 AND shelf_state='active'",
                params![id, encoded],
            )?;
            tx.commit()?;
            Ok(state)
        })
    }
}

#[cfg(test)]
#[path = "reader_progress_store_tests.rs"]
mod tests;
