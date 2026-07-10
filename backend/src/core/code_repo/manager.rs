//! 仓库管理器 - 注册/查询/删除/刷新

use chrono::Utc;
use git2::Repository;
use std::path::PathBuf;
use std::sync::Arc;

use crate::core::code_repo::git_extractor::GitExtractor;
use crate::core::code_repo::language_detect::LanguageDetector;
use crate::error::BrainError;
use crate::infra::sqlite_store::SqliteStore;
use crate::models::repo::*;

/// 仓库管理器
pub struct RepoManager {
    db: Arc<SqliteStore>,
    config: RepoManagerConfig,
}

/// 仓库管理器配置
#[derive(Debug, Clone)]
pub struct RepoManagerConfig {
    pub exclude_dirs: Vec<String>,
    pub max_language_files: usize,
}

impl Default for RepoManagerConfig {
    fn default() -> Self {
        Self {
            exclude_dirs: vec![
                ".git".to_string(),
                "node_modules".to_string(),
                "target".to_string(),
                "vendor".to_string(),
                "__pycache__".to_string(),
                ".venv".to_string(),
                "dist".to_string(),
                "build".to_string(),
            ],
            max_language_files: 500,
        }
    }
}

impl RepoManager {
    /// 创建新的仓库管理器
    pub fn new(db: Arc<SqliteStore>, config: RepoManagerConfig) -> Self {
        Self { db, config }
    }

    /// 注册仓库
    pub fn register(&self, path: &str, name: &str) -> Result<CodeRepo, BrainError> {
        // 验证路径
        let repo_path = PathBuf::from(path);
        if !repo_path.exists() {
            return Err(BrainError::RepoNotFound(repo_path));
        }

        // 打开 Git 仓库
        let repo = Repository::open(&repo_path).map_err(|e| BrainError::GitError {
            path: repo_path.clone(),
            detail: format!("打开仓库失败: {e}"),
        })?;

        // 检查唯一性
        if let Some((_, existing_path, _)) = self.db.get_code_repo_by_name(name)? {
            return Err(BrainError::Internal(format!(
                "仓库名 '{}' 已被使用 (路径: {})",
                name, existing_path
            )));
        }

        // 提取元数据
        let current_branch = GitExtractor::current_branch(&repo)?;
        let recent_commits = GitExtractor::recent_commits(&repo, 5)?;
        let is_dirty = GitExtractor::is_dirty(&repo)?;
        let last_activity = GitExtractor::last_activity(&repo)?;
        let head_hash = GitExtractor::head_hash(&repo)?;

        // 语言检测
        let language_stats = LanguageDetector::detect(
            &repo_path,
            &self.config.exclude_dirs,
            self.config.max_language_files,
        );

        // 构建元数据缓存
        let metadata = RepoMetadataCache {
            current_branch: current_branch.clone(),
            language_stats: language_stats.clone(),
            is_dirty,
            latest_commit: recent_commits.first().cloned(),
            last_activity: last_activity.to_rfc3339(),
            head_hash,
            updated_at: Utc::now().to_rfc3339(),
        };

        let metadata_json = serde_json::to_string(&metadata)
            .map_err(|e| BrainError::Internal(format!("序列化元数据失败: {e}")))?;

        // 写入数据库
        self.db.insert_code_repo(name, path, &metadata_json)?;

        let now = Utc::now();
        Ok(CodeRepo {
            name: name.to_string(),
            path: repo_path,
            current_branch,
            language_stats,
            is_dirty,
            recent_commits,
            last_activity,
            linked_notes: vec![],
            registered_at: now,
            status: RepoStatus::Active,
        })
    }

    /// 列出所有仓库
    pub fn list(&self) -> Result<Vec<RepoCard>, BrainError> {
        let rows = self.db.list_code_repos()?;
        let mut cards = Vec::new();

        for (name, path, metadata_json) in rows {
            let metadata: RepoMetadataCache =
                serde_json::from_str(&metadata_json).unwrap_or(RepoMetadataCache {
                    current_branch: "unknown".to_string(),
                    language_stats: Default::default(),
                    is_dirty: false,
                    latest_commit: None,
                    last_activity: Utc::now().to_rfc3339(),
                    head_hash: String::new(),
                    updated_at: Utc::now().to_rfc3339(),
                });

            let linked_count = self.db.count_note_links(&name).unwrap_or(0);

            cards.push(RepoCard {
                name,
                path: PathBuf::from(path),
                current_branch: metadata.current_branch,
                latest_commit: metadata.latest_commit,
                is_dirty: metadata.is_dirty,
                languages: metadata.language_stats,
                linked_notes_count: linked_count,
                last_activity: chrono::DateTime::parse_from_rfc3339(&metadata.last_activity)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                status: RepoStatus::Active,
            });
        }

        Ok(cards)
    }

    /// 获取仓库详情
    pub fn detail(&self, name: &str) -> Result<RepoDetail, BrainError> {
        let (_, path, metadata_json) = self
            .db
            .get_code_repo_by_name(name)?
            .ok_or_else(|| BrainError::Internal(format!("仓库 '{}' 不存在", name)))?;

        let repo_path = PathBuf::from(&path);
        let repo = Repository::open(&repo_path).map_err(|e| BrainError::GitError {
            path: repo_path.clone(),
            detail: format!("打开仓库失败: {e}"),
        })?;

        let metadata: RepoMetadataCache = serde_json::from_str(&metadata_json)
            .map_err(|e| BrainError::Internal(format!("解析元数据失败: {e}")))?;

        let branches = GitExtractor::branches(&repo)?;
        let remote_urls = GitExtractor::remote_urls(&repo)?;
        let working_dir_status = GitExtractor::working_dir_status(&repo)?;
        let head_hash = GitExtractor::head_hash(&repo)?;
        let total_commits = GitExtractor::total_commits(&repo, 1000)?;
        let contributors = GitExtractor::contributors(&repo, 100)?;
        let recent_commits = GitExtractor::recent_commits(&repo, 20)?;
        let last_activity = GitExtractor::last_activity(&repo)?;
        let linked_notes = self.db.get_linked_notes(name)?;

        Ok(RepoDetail {
            base: CodeRepo {
                name: name.to_string(),
                path: repo_path.clone(),
                current_branch: metadata.current_branch,
                language_stats: metadata.language_stats,
                is_dirty: metadata.is_dirty,
                recent_commits,
                last_activity,
                linked_notes,
                registered_at: Utc::now(),
                status: RepoStatus::Active,
            },
            branches,
            remote_urls,
            working_dir_status,
            vscode_uri: format!("vscode://file{}", repo_path.display()),
            head_hash,
            total_commits,
            contributors,
        })
    }

    /// 删除仓库
    pub fn delete(&self, name: &str) -> Result<bool, BrainError> {
        self.db.delete_code_repo(name)
    }

    /// 刷新仓库元数据
    pub fn refresh_metadata(&self, name: &str) -> Result<(), BrainError> {
        let (_, path, _) = self
            .db
            .get_code_repo_by_name(name)?
            .ok_or_else(|| BrainError::Internal(format!("仓库 '{}' 不存在", name)))?;

        let repo_path = PathBuf::from(&path);
        let repo = Repository::open(&repo_path).map_err(|e| BrainError::GitError {
            path: repo_path.clone(),
            detail: format!("打开仓库失败: {e}"),
        })?;

        let current_branch = GitExtractor::current_branch(&repo)?;
        let is_dirty = GitExtractor::is_dirty(&repo)?;
        let last_activity = GitExtractor::last_activity(&repo)?;
        let head_hash = GitExtractor::head_hash(&repo)?;
        let recent_commits = GitExtractor::recent_commits(&repo, 1)?;

        let metadata = RepoMetadataCache {
            current_branch,
            language_stats: Default::default(), // 语言统计不频繁刷新
            is_dirty,
            latest_commit: recent_commits.first().cloned(),
            last_activity: last_activity.to_rfc3339(),
            head_hash,
            updated_at: Utc::now().to_rfc3339(),
        };

        let metadata_json = serde_json::to_string(&metadata)
            .map_err(|e| BrainError::Internal(format!("序列化元数据失败: {e}")))?;

        self.db.update_repo_metadata(name, &metadata_json)?;
        Ok(())
    }

    /// 关联笔记到仓库
    pub fn link_note(&self, note_path: &str, repo_name: &str) -> Result<(), BrainError> {
        // 验证仓库存在
        self.db
            .get_code_repo_by_name(repo_name)?
            .ok_or_else(|| BrainError::Internal(format!("仓库 '{}' 不存在", repo_name)))?;

        self.db.insert_note_repo_link(note_path, repo_name)?;
        Ok(())
    }

    /// 获取关联笔记列表
    pub fn get_linked_notes(&self, repo_name: &str) -> Result<Vec<String>, BrainError> {
        self.db.get_linked_notes(repo_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tempfile::TempDir;

    fn create_manager() -> (TempDir, RepoManager) {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db = Arc::new(SqliteStore::new(&db_path).unwrap());
        let config = RepoManagerConfig::default();
        let manager = RepoManager::new(db, config);
        (dir, manager)
    }

    fn create_test_git_repo(dir: &Path) -> PathBuf {
        let repo_dir = dir.join("test_repo");
        std::fs::create_dir_all(&repo_dir).unwrap();
        let repo = Repository::init(&repo_dir).unwrap();

        {
            let mut index = repo.index().unwrap();
            let tree_id = index.write_tree().unwrap();
            let tree = repo.find_tree(tree_id).unwrap();
            let sig = git2::Signature::now("Test", "test@test.com").unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
                .unwrap();
        }

        repo_dir
    }

    #[test]
    fn test_register_repo() {
        let (dir, manager) = create_manager();
        let repo_path = create_test_git_repo(dir.path());

        let repo = manager
            .register(repo_path.to_str().unwrap(), "test-repo")
            .unwrap();
        assert_eq!(repo.name, "test-repo");
        assert_eq!(repo.path, repo_path);
    }

    #[test]
    fn test_list_repos() {
        let (dir, manager) = create_manager();
        let repo_path = create_test_git_repo(dir.path());

        manager
            .register(repo_path.to_str().unwrap(), "test-repo")
            .unwrap();
        let repos = manager.list().unwrap();
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].name, "test-repo");
    }

    #[test]
    fn test_delete_repo() {
        let (dir, manager) = create_manager();
        let repo_path = create_test_git_repo(dir.path());

        manager
            .register(repo_path.to_str().unwrap(), "test-repo")
            .unwrap();
        let deleted = manager.delete("test-repo").unwrap();
        assert!(deleted);

        let repos = manager.list().unwrap();
        assert_eq!(repos.len(), 0);
    }

    #[test]
    fn test_register_nonexistent_path() {
        let (_dir, manager) = create_manager();
        let result = manager.register("/nonexistent/path", "test");
        assert!(result.is_err());
    }
}
