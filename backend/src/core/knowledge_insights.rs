//! 知识库洞察模块
//!
//! 分析 Obsidian Vault 的笔记结构和链接关系，提供知识健康度洞察。

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use chrono::{DateTime, Local, Utc};

use crate::error::BrainError;
use crate::infra::obsidian_client::ObsidianClient;

/// 排除的目录前缀
const EXCLUDED_DIRS: &[&str] = &[".obsidian/", "templates/", ".trash/", "Timeline/images/"];

/// 知识库洞察结果
#[derive(Debug, serde::Serialize)]
pub struct KnowledgeInsights {
    pub islands: IslandData,
    pub hubs: HubData,
    pub dormant: DormantData,
    pub fresh: FreshData,
    pub domains: DomainData,
}

#[derive(Debug, serde::Serialize)]
pub struct IslandData {
    pub count: usize,
    pub notes: Vec<NoteInfo>,
}

#[derive(Debug, serde::Serialize)]
pub struct HubData {
    pub notes: Vec<HubNote>,
}

#[derive(Debug, serde::Serialize)]
pub struct HubNote {
    pub path: String,
    pub refs: usize,
    pub referenced_by: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct DormantData {
    pub notes: Vec<NoteInfo>,
}

#[derive(Debug, serde::Serialize)]
pub struct FreshData {
    pub notes: Vec<FreshNote>,
}

#[derive(Debug, serde::Serialize)]
pub struct FreshNote {
    pub path: String,
    pub created: String,
}

#[derive(Debug, serde::Serialize)]
pub struct DomainData {
    pub folders: Vec<FolderStat>,
}

#[derive(Debug, serde::Serialize)]
pub struct FolderStat {
    pub folder: String,
    pub count: usize,
    pub percentage: f32,
}

#[derive(Debug, serde::Serialize)]
pub struct NoteInfo {
    pub path: String,
    pub modified: String,
    pub days_ago: u64,
}

/// 知识库洞察引擎
pub struct KnowledgeInsightEngine {
    obsidian: Arc<ObsidianClient>,
}

impl KnowledgeInsightEngine {
    pub fn new(obsidian: Arc<ObsidianClient>) -> Self {
        Self { obsidian }
    }

    /// 获取完整的知识库洞察
    pub async fn get_insights(&self) -> Result<KnowledgeInsights, BrainError> {
        // 1. 获取所有 .md 文件
        let all_files = self.obsidian.list_all_files().await?;
        let md_files: Vec<String> = all_files
            .into_iter()
            .filter(|f| f.ends_with(".md") && !is_excluded(f))
            .collect();

        let total = md_files.len();
        let now = Utc::now();

        // 2. 读取每个文件的内容和元数据
        let mut note_data: Vec<(String, String, Option<u64>, Option<u64>)> = Vec::new(); // (path, content, ctime, mtime)
        for path in &md_files {
            match self.obsidian.read_note(path).await {
                Ok(note) => {
                    let content = note.content.unwrap_or_default();
                    let ctime = note.stat.as_ref().and_then(|s| s.ctime);
                    let mtime = note.stat.as_ref().and_then(|s| s.mtime);
                    note_data.push((path.clone(), content, ctime, mtime));
                }
                Err(_) => continue,
            }
        }

        // 3. 构建链接图谱
        let mut inbound: HashMap<String, Vec<String>> = HashMap::new(); // target -> [sources]
        let mut all_paths: HashSet<String> = note_data.iter().map(|(p, _, _, _)| p.clone()).collect();

        for (source_path, content, _, _) in &note_data {
            let links = extract_links(content);
            for target in links {
                // Resolve link target to actual path
                let resolved = resolve_link(&target, &all_paths);
                if let Some(resolved_path) = resolved {
                    inbound
                        .entry(resolved_path)
                        .or_default()
                        .push(source_path.clone());
                }
            }
        }

        // 4. 计算各维度洞察
        let islands = compute_islands(&note_data, &inbound, &now);
        let hubs = compute_hubs(&inbound);
        let dormant = compute_dormant(&note_data, &now);
        let fresh = compute_fresh(&note_data);
        let domains = compute_domains(&note_data, total);

        Ok(KnowledgeInsights {
            islands,
            hubs,
            dormant,
            fresh,
            domains,
        })
    }
}

/// 检查文件是否在排除目录中
fn is_excluded(path: &str) -> bool {
    EXCLUDED_DIRS.iter().any(|dir| path.starts_with(dir))
}

/// 从 Markdown 内容中提取链接目标
fn extract_links(content: &str) -> Vec<String> {
    let mut links = Vec::new();

    // [[wikilink]] or [[wikilink|display]]
    let mut pos = 0;
    while let Some(start) = content[pos..].find("[[") {
        let abs_start = pos + start + 2;
        if let Some(end) = content[abs_start..].find("]]") {
            let raw = &content[abs_start..abs_start + end];
            // Remove display part (after |)
            let target = raw.split('|').next().unwrap_or(raw).trim();
            if !target.is_empty() {
                links.push(target.to_string());
            }
            pos = abs_start + end + 2;
        } else {
            break;
        }
    }

    // [text](path) — only .md links
    pos = 0;
    while let Some(start) = content[pos..].find("](") {
        let abs_start = pos + start + 2;
        if let Some(end) = content[abs_start..].find(')') {
            let target = content[abs_start..abs_start + end].trim();
            if target.ends_with(".md") && !target.starts_with("http") {
                links.push(target.to_string());
            }
            pos = abs_start + end + 1;
        } else {
            break;
        }
    }

    links
}

/// 将链接目标解析为实际文件路径
fn resolve_link(target: &str, all_paths: &HashSet<String>) -> Option<String> {
    // Direct match
    if all_paths.contains(target) {
        return Some(target.to_string());
    }
    // Try with .md extension
    let with_md = if target.ends_with(".md") {
        target.to_string()
    } else {
        format!("{}.md", target)
    };
    if all_paths.contains(&with_md) {
        return Some(with_md);
    }
    // Try matching by filename only (wikilinks often omit path)
    let filename = target.rsplit('/').next().unwrap_or(target);
    let filename_md = if filename.ends_with(".md") {
        filename.to_string()
    } else {
        format!("{}.md", filename)
    };
    for path in all_paths {
        if path.ends_with(&format!("/{}", filename_md)) || path == &filename_md {
            return Some(path.clone());
        }
    }
    None
}

/// 知识孤岛：没有被任何笔记引用的笔记
fn compute_islands(
    note_data: &[(String, String, Option<u64>, Option<u64>)],
    inbound: &HashMap<String, Vec<String>>,
    now: &DateTime<Utc>,
) -> IslandData {
    let mut islands: Vec<NoteInfo> = note_data
        .iter()
        .filter(|(path, _, _, _)| !inbound.contains_key(path))
        .map(|(path, _, _, mtime)| {
            let modified = mtime
                .map(|t| format_timestamp(t))
                .unwrap_or_else(|| "unknown".to_string());
            let days_ago = mtime
                .map(|t| days_since(t, now))
                .unwrap_or(0);
            NoteInfo {
                path: path.clone(),
                modified,
                days_ago,
            }
        })
        .collect();
    islands.sort_by(|a, b| b.days_ago.cmp(&a.days_ago));
    let count = islands.len();
    islands.truncate(20);
    IslandData { count, notes: islands }
}

/// 知识枢纽：被引用最多的笔记
fn compute_hubs(inbound: &HashMap<String, Vec<String>>) -> HubData {
    let mut hubs: Vec<HubNote> = inbound
        .iter()
        .map(|(path, sources)| HubNote {
            path: path.clone(),
            refs: sources.len(),
            referenced_by: sources.clone(),
        })
        .collect();
    hubs.sort_by(|a, b| b.refs.cmp(&a.refs));
    hubs.truncate(10);
    HubData { notes: hubs }
}

/// 尘封笔记：最久未修改
fn compute_dormant(
    note_data: &[(String, String, Option<u64>, Option<u64>)],
    now: &DateTime<Utc>,
) -> DormantData {
    let mut notes: Vec<NoteInfo> = note_data
        .iter()
        .filter(|(_, _, _, mtime)| mtime.is_some())
        .map(|(path, _, _, mtime)| {
            let t = mtime.unwrap();
            NoteInfo {
                path: path.clone(),
                modified: format_timestamp(t),
                days_ago: days_since(t, now),
            }
        })
        .collect();
    notes.sort_by(|a, b| b.days_ago.cmp(&a.days_ago));
    notes.truncate(10);
    DormantData { notes }
}

/// 新生知识：最近创建
fn compute_fresh(note_data: &[(String, String, Option<u64>, Option<u64>)]) -> FreshData {
    let mut notes: Vec<FreshNote> = note_data
        .iter()
        .filter(|(_, _, ctime, _)| ctime.is_some())
        .map(|(path, _, ctime, _)| FreshNote {
            path: path.clone(),
            created: format_timestamp(ctime.unwrap()),
        })
        .collect();
    notes.sort_by(|a, b| b.created.cmp(&a.created));
    notes.truncate(10);
    FreshData { notes }
}

/// 知识领域：按一级文件夹聚类
fn compute_domains(
    note_data: &[(String, String, Option<u64>, Option<u64>)],
    total: usize,
) -> DomainData {
    let mut folder_counts: HashMap<String, usize> = HashMap::new();
    for (path, _, _, _) in note_data {
        let folder = path
            .split('/')
            .next()
            .unwrap_or("root")
            .to_string();
        *folder_counts.entry(folder).or_insert(0) += 1;
    }
    let mut folders: Vec<FolderStat> = folder_counts
        .into_iter()
        .map(|(folder, count)| FolderStat {
            folder,
            count,
            percentage: if total > 0 {
                (count as f32 / total as f32) * 100.0
            } else {
                0.0
            },
        })
        .collect();
    folders.sort_by(|a, b| b.count.cmp(&a.count));
    DomainData { folders }
}

/// 将毫秒时间戳格式化为日期字符串
fn format_timestamp(ms: u64) -> String {
    let secs = (ms / 1000) as i64;
    DateTime::from_timestamp(secs, 0)
        .map(|dt| dt.with_timezone(&Local).format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// 计算距今天数
fn days_since(ms: u64, now: &DateTime<Utc>) -> u64 {
    let secs = (ms / 1000) as i64;
    DateTime::from_timestamp(secs, 0)
        .map(|dt| {
            let duration = now.signed_duration_since(dt);
            duration.num_days().max(0) as u64
        })
        .unwrap_or(0)
}
