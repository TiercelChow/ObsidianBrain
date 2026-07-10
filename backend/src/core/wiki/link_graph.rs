//! Wiki 链接图谱分析
//!
//! 构建 Wiki 页面间的 [[wikilink]] 引用图谱，用于 Lint 健康检查。

use std::collections::{HashMap, HashSet};

/// 链接图谱
pub struct LinkGraph {
    /// 所有页面路径
    pub pages: HashSet<String>,
    /// 页面 → 链接到它的源列表
    pub inbound: HashMap<String, Vec<String>>,
}

impl LinkGraph {
    /// 从页面内容构建链接图谱
    pub fn build(page_contents: &[(String, String)]) -> Self {
        let pages: HashSet<String> = page_contents.iter().map(|(p, _)| p.clone()).collect();
        let mut inbound: HashMap<String, Vec<String>> = HashMap::new();

        for (source_path, content) in page_contents {
            let links = extract_wikilinks(content);
            for target in links {
                let resolved = resolve_link(&target, &pages, source_path);
                if let Some(resolved_path) = resolved {
                    inbound
                        .entry(resolved_path)
                        .or_default()
                        .push(source_path.clone());
                }
            }
        }

        Self { pages, inbound }
    }

    /// 找出孤岛页（没有任何入链的页面）
    pub fn find_orphans(&self) -> Vec<String> {
        let mut orphans: Vec<String> = self
            .pages
            .iter()
            .filter(|p| !self.inbound.contains_key(*p))
            .cloned()
            .collect();
        orphans.sort();
        orphans
    }

    /// 找出知识枢纽（入链最多的页面）
    pub fn find_hubs(&self, top_n: usize) -> Vec<(String, usize)> {
        let mut hubs: Vec<(String, usize)> = self
            .inbound
            .iter()
            .map(|(path, refs)| (path.clone(), refs.len()))
            .collect();
        hubs.sort_by_key(|a| std::cmp::Reverse(a.1));
        hubs.truncate(top_n);
        hubs
    }
}

/// 从 Markdown 内容中提取 [[wikilink]] 目标
fn extract_wikilinks(content: &str) -> Vec<String> {
    let mut links = Vec::new();
    let mut pos = 0;

    while let Some(start) = content[pos..].find("[[") {
        let abs_start = pos + start + 2;
        if let Some(end) = content[abs_start..].find("]]") {
            let raw = &content[abs_start..abs_start + end];
            // 去掉显示部分（| 之后）
            let target = raw.split('|').next().unwrap_or(raw).trim();
            if !target.is_empty() {
                links.push(target.to_string());
            }
            pos = abs_start + end + 2;
        } else {
            break;
        }
    }

    links
}

/// 将链接目标解析为实际文件路径
fn resolve_link(target: &str, all_pages: &HashSet<String>, source_path: &str) -> Option<String> {
    // 直接匹配
    if all_pages.contains(target) {
        return Some(target.to_string());
    }

    // 尝试加 .md 后缀
    let with_md = if target.ends_with(".md") {
        target.to_string()
    } else {
        format!("{}.md", target)
    };
    if all_pages.contains(&with_md) {
        return Some(with_md);
    }

    // 尝试在 source_path 同目录下解析
    let source_dir = source_path
        .rsplit_once('/')
        .map(|(dir, _)| dir)
        .unwrap_or("");
    let relative = format!("{}/{}", source_dir, with_md);
    if all_pages.contains(&relative) {
        return Some(relative);
    }

    // 尝试在 Wiki/ 各子目录下查找
    for dir in &[
        "Wiki/entities",
        "Wiki/concepts",
        "Wiki/sources",
        "Wiki/synthesis",
    ] {
        let full = format!("{}/{}", dir, with_md);
        if all_pages.contains(&full) {
            return Some(full);
        }
    }

    // 按文件名匹配
    let filename = target.rsplit('/').next().unwrap_or(target);
    let filename_md = if filename.ends_with(".md") {
        filename.to_string()
    } else {
        format!("{}.md", filename)
    };
    for page in all_pages {
        if page.ends_with(&format!("/{}", filename_md)) || page == &filename_md {
            return Some(page.clone());
        }
    }

    None
}
