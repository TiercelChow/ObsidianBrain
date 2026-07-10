//! 笔记关联器

use chrono::Utc;
use std::sync::Arc;

use crate::error::BrainError;
use crate::infra::sqlite_store::SqliteStore;
use crate::models::repo::NoteRepoLink;

/// 笔记关联器
pub struct NoteLinker {
    db: Arc<SqliteStore>,
}

impl NoteLinker {
    pub fn new(db: Arc<SqliteStore>) -> Self {
        Self { db }
    }

    /// 关联笔记到仓库
    pub fn link_note(&self, note_path: &str, repo_name: &str) -> Result<NoteRepoLink, BrainError> {
        // 验证仓库存在
        self.db
            .get_code_repo_by_name(repo_name)?
            .ok_or_else(|| BrainError::Internal(format!("仓库 '{}' 不存在", repo_name)))?;

        self.db.insert_note_repo_link(note_path, repo_name)?;
        Ok(NoteRepoLink {
            note_path: note_path.to_string(),
            repo_name: repo_name.to_string(),
            linked_at: Utc::now(),
        })
    }

    /// 获取仓库关联的笔记
    pub fn get_linked_notes(&self, repo_name: &str) -> Result<Vec<String>, BrainError> {
        self.db.get_linked_notes(repo_name)
    }

    /// 获取笔记关联的仓库
    pub fn get_note_repos(&self, note_path: &str) -> Result<Vec<String>, BrainError> {
        let repos = self.db.list_code_repos()?;
        let mut linked = Vec::new();
        for (name, _, _) in repos {
            let notes = self.db.get_linked_notes(&name)?;
            if notes.contains(&note_path.to_string()) {
                linked.push(name);
            }
        }
        Ok(linked)
    }
}
