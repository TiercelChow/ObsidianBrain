//! 代码仓相关数据模型

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// 仓库状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RepoStatus {
    Active,
    Inactive,
}

impl std::fmt::Display for RepoStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RepoStatus::Active => write!(f, "active"),
            RepoStatus::Inactive => write!(f, "inactive"),
        }
    }
}

/// Commit 摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitSummary {
    /// 短哈希 (7 字符)
    pub hash: String,
    /// 作者
    pub author: String,
    /// 提交消息
    pub message: String,
    /// 提交时间
    pub timestamp: DateTime<Utc>,
}

/// 已注册的代码仓库（完整信息）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeRepo {
    /// 仓库显示名称（唯一标识）
    pub name: String,
    /// 本地绝对路径
    pub path: PathBuf,
    /// 当前分支名
    pub current_branch: String,
    /// 语言构成：语言名 → 占比 (0.0~1.0)
    pub language_stats: HashMap<String, f32>,
    /// 工作区是否有未提交更改
    pub is_dirty: bool,
    /// 最近 commit 摘要列表
    pub recent_commits: Vec<CommitSummary>,
    /// 最后活动时间（最新 commit 时间）
    pub last_activity: DateTime<Utc>,
    /// 关联的笔记路径列表（vault 内相对路径）
    pub linked_notes: Vec<String>,
    /// 注册时间
    pub registered_at: DateTime<Utc>,
    /// 仓库状态
    pub status: RepoStatus,
}

/// 仓库卡片信息（列表展示用）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoCard {
    pub name: String,
    pub path: PathBuf,
    pub current_branch: String,
    pub latest_commit: Option<CommitSummary>,
    pub is_dirty: bool,
    pub languages: HashMap<String, f32>,
    pub linked_notes_count: usize,
    pub last_activity: DateTime<Utc>,
    pub status: RepoStatus,
}

/// 工作区状态
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkingDirStatus {
    pub modified: usize,
    pub added: usize,
    pub deleted: usize,
    pub untracked: usize,
    pub total: usize,
}

/// 仓库详情
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoDetail {
    #[serde(flatten)]
    pub base: CodeRepo,
    pub branches: Vec<String>,
    pub remote_urls: Vec<String>,
    pub working_dir_status: WorkingDirStatus,
    pub vscode_uri: String,
    pub head_hash: String,
    pub total_commits: usize,
    pub contributors: Vec<String>,
}

/// 笔记-仓库关联记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteRepoLink {
    pub note_path: String,
    pub repo_name: String,
    pub linked_at: DateTime<Utc>,
}

/// VSCode 打开结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VscodeResult {
    pub repo_name: String,
    pub vscode_uri: String,
    pub opened: bool,
    pub message: String,
}

/// SQLite 行模型：code_repos
#[derive(Debug, Clone)]
pub struct CodeRepoRow {
    pub name: String,
    pub path: String,
    pub registered_at: String,
    pub metadata: String,
}

/// 缓存的仓库元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoMetadataCache {
    pub current_branch: String,
    pub language_stats: HashMap<String, f32>,
    pub is_dirty: bool,
    pub latest_commit: Option<CommitSummary>,
    pub last_activity: String,
    pub head_hash: String,
    pub updated_at: String,
}

/// SQLite 行模型：note_repo_links
#[derive(Debug, Clone)]
pub struct NoteRepoLinkRow {
    pub note_path: String,
    pub repo_name: String,
    pub linked_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repo_status_display() {
        assert_eq!(RepoStatus::Active.to_string(), "active");
        assert_eq!(RepoStatus::Inactive.to_string(), "inactive");
    }

    #[test]
    fn test_commit_summary_roundtrip() {
        let commit = CommitSummary {
            hash: "abc1234".to_string(),
            author: "test".to_string(),
            message: "test commit".to_string(),
            timestamp: Utc::now(),
        };
        let json = serde_json::to_string(&commit).unwrap();
        let parsed: CommitSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.hash, "abc1234");
    }
}
