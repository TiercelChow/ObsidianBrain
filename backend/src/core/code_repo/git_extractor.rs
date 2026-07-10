//! Git 仓库元信息提取器

use chrono::{DateTime, Utc};
use git2::{BranchType, Repository, Sort, StatusOptions};

use crate::error::BrainError;
use crate::models::repo::{CommitSummary, WorkingDirStatus};

/// Git 仓库信息提取器
pub struct GitExtractor;

impl GitExtractor {
    /// 获取当前分支名
    pub fn current_branch(repo: &Repository) -> Result<String, BrainError> {
        let head = repo.head().map_err(|e| BrainError::GitError {
            path: repo.path().to_path_buf(),
            detail: format!("获取 HEAD 失败: {e}"),
        })?;

        if head.is_branch() {
            Ok(head.shorthand().unwrap_or("unknown").to_string())
        } else {
            let oid = head.target().map(|o| o.to_string()).unwrap_or_default();
            Ok(format!("HEAD (detached at {})", &oid[..7.min(oid.len())]))
        }
    }

    /// 获取所有本地分支
    pub fn branches(repo: &Repository) -> Result<Vec<String>, BrainError> {
        let branches =
            repo.branches(Some(BranchType::Local))
                .map_err(|e| BrainError::GitError {
                    path: repo.path().to_path_buf(),
                    detail: format!("获取分支列表失败: {e}"),
                })?;

        let mut branch_names = Vec::new();
        for branch in branches {
            let (branch, _) = branch.map_err(|e| BrainError::GitError {
                path: repo.path().to_path_buf(),
                detail: format!("读取分支失败: {e}"),
            })?;
            if let Some(name) = branch.name().map_err(|e| BrainError::GitError {
                path: repo.path().to_path_buf(),
                detail: format!("获取分支名失败: {e}"),
            })? {
                branch_names.push(name.to_string());
            }
        }
        Ok(branch_names)
    }

    /// 获取最近 n 条 commit
    pub fn recent_commits(repo: &Repository, n: usize) -> Result<Vec<CommitSummary>, BrainError> {
        let mut revwalk = repo.revwalk().map_err(|e| BrainError::GitError {
            path: repo.path().to_path_buf(),
            detail: format!("创建 revwalk 失败: {e}"),
        })?;

        let _ = revwalk.set_sorting(Sort::TIME);
        revwalk.push_head().map_err(|e| BrainError::GitError {
            path: repo.path().to_path_buf(),
            detail: format!("push_head 失败: {e}"),
        })?;

        let mut commits = Vec::new();
        for (i, oid) in revwalk.enumerate() {
            if i >= n {
                break;
            }
            let oid = oid.map_err(|e| BrainError::GitError {
                path: repo.path().to_path_buf(),
                detail: format!("读取 commit OID 失败: {e}"),
            })?;

            let commit = repo.find_commit(oid).map_err(|e| BrainError::GitError {
                path: repo.path().to_path_buf(),
                detail: format!("查找 commit 失败: {e}"),
            })?;

            let hash = commit.id().to_string();
            let short_hash = hash[..7.min(hash.len())].to_string();
            let author = commit.author().name().unwrap_or("unknown").to_string();
            let message = commit.summary().unwrap_or("").to_string();
            let timestamp = DateTime::from_timestamp(commit.time().seconds(), 0)
                .unwrap_or_default()
                .with_timezone(&Utc);

            commits.push(CommitSummary {
                hash: short_hash,
                author,
                message,
                timestamp,
            });
        }
        Ok(commits)
    }

    /// 检查工作区是否有未提交更改
    pub fn is_dirty(repo: &Repository) -> Result<bool, BrainError> {
        let mut opts = StatusOptions::new();
        opts.include_untracked(true)
            .recurse_untracked_dirs(false)
            .include_ignored(false);

        let statuses = repo
            .statuses(Some(&mut opts))
            .map_err(|e| BrainError::GitError {
                path: repo.path().to_path_buf(),
                detail: format!("获取状态失败: {e}"),
            })?;

        Ok(!statuses.is_empty())
    }

    /// 获取工作区详细状态
    pub fn working_dir_status(repo: &Repository) -> Result<WorkingDirStatus, BrainError> {
        let mut opts = StatusOptions::new();
        opts.include_untracked(true)
            .recurse_untracked_dirs(false)
            .include_ignored(false);

        let statuses = repo
            .statuses(Some(&mut opts))
            .map_err(|e| BrainError::GitError {
                path: repo.path().to_path_buf(),
                detail: format!("获取状态失败: {e}"),
            })?;

        let mut status = WorkingDirStatus::default();
        for entry in statuses.iter() {
            let s = entry.status();
            if s.is_wt_modified() || s.is_index_modified() {
                status.modified += 1;
            }
            if s.is_index_new() {
                status.added += 1;
            }
            if s.is_wt_deleted() || s.is_index_deleted() {
                status.deleted += 1;
            }
            if s.is_wt_new() {
                status.untracked += 1;
            }
        }
        status.total = status.modified + status.added + status.deleted + status.untracked;
        Ok(status)
    }

    /// 获取 HEAD commit 哈希
    pub fn head_hash(repo: &Repository) -> Result<String, BrainError> {
        let head = repo.head().map_err(|e| BrainError::GitError {
            path: repo.path().to_path_buf(),
            detail: format!("获取 HEAD 失败: {e}"),
        })?;

        Ok(head.target().map(|o| o.to_string()).unwrap_or_default())
    }

    /// 获取远端 URL
    pub fn remote_urls(repo: &Repository) -> Result<Vec<String>, BrainError> {
        let remotes = repo.remotes().map_err(|e| BrainError::GitError {
            path: repo.path().to_path_buf(),
            detail: format!("获取远端列表失败: {e}"),
        })?;

        let mut urls = Vec::new();
        for name in remotes.iter().flatten() {
            if let Ok(remote) = repo.find_remote(name) {
                if let Some(url) = remote.url() {
                    urls.push(url.to_string());
                }
            }
        }
        Ok(urls)
    }

    /// 获取最后活动时间
    pub fn last_activity(repo: &Repository) -> Result<DateTime<Utc>, BrainError> {
        let commits = Self::recent_commits(repo, 1)?;
        Ok(commits
            .first()
            .map(|c| c.timestamp)
            .unwrap_or_else(Utc::now))
    }

    /// 获取总 commit 数量（采样，最多统计 max_count）
    pub fn total_commits(repo: &Repository, max_count: usize) -> Result<usize, BrainError> {
        let mut revwalk = repo.revwalk().map_err(|e| BrainError::GitError {
            path: repo.path().to_path_buf(),
            detail: format!("创建 revwalk 失败: {e}"),
        })?;
        revwalk.push_head().map_err(|e| BrainError::GitError {
            path: repo.path().to_path_buf(),
            detail: format!("push_head 失败: {e}"),
        })?;

        Ok(revwalk.take(max_count).count())
    }

    /// 获取贡献者列表（去重）
    pub fn contributors(repo: &Repository, sample_size: usize) -> Result<Vec<String>, BrainError> {
        let commits = Self::recent_commits(repo, sample_size)?;
        let mut authors: Vec<String> = commits.iter().map(|c| c.author.clone()).collect();
        authors.sort();
        authors.dedup();
        Ok(authors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_repo() -> (TempDir, Repository) {
        let dir = TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();

        // Create initial commit using a block to limit borrows
        {
            let mut index = repo.index().unwrap();
            let tree_id = index.write_tree().unwrap();
            let tree = repo.find_tree(tree_id).unwrap();
            let sig = git2::Signature::now("Test", "test@test.com").unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
                .unwrap();
        }

        (dir, repo)
    }

    #[test]
    fn test_current_branch() {
        let (_dir, repo) = create_test_repo();
        let branch = GitExtractor::current_branch(&repo).unwrap();
        // Default branch could be "main" or "master"
        assert!(branch == "main" || branch == "master");
    }

    #[test]
    fn test_branches() {
        let (_dir, repo) = create_test_repo();
        let branches = GitExtractor::branches(&repo).unwrap();
        // Default branch could be "main" or "master"
        assert!(branches.contains(&"main".to_string()) || branches.contains(&"master".to_string()));
    }

    #[test]
    fn test_recent_commits() {
        let (_dir, repo) = create_test_repo();
        let commits = GitExtractor::recent_commits(&repo, 10).unwrap();
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].message, "Initial commit");
    }

    #[test]
    fn test_is_dirty_clean() {
        let (_dir, repo) = create_test_repo();
        let dirty = GitExtractor::is_dirty(&repo).unwrap();
        assert!(!dirty);
    }

    #[test]
    fn test_head_hash() {
        let (_dir, repo) = create_test_repo();
        let hash = GitExtractor::head_hash(&repo).unwrap();
        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 40);
    }

    #[test]
    fn test_working_dir_status_clean() {
        let (_dir, repo) = create_test_repo();
        let status = GitExtractor::working_dir_status(&repo).unwrap();
        assert_eq!(status.total, 0);
    }

    #[test]
    fn test_last_activity() {
        let (_dir, repo) = create_test_repo();
        let activity = GitExtractor::last_activity(&repo).unwrap();
        assert!(activity <= Utc::now());
    }
}
