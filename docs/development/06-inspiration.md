# 开发设计文档：灵感熔炉 (Inspiration Forge)

> **模块编号**: 06 | **版本**: v0.1 | **状态**: 设计中 | **最后更新**: 2026-05-29
>
> **上游文档**: [顶层设计文档 (top_design.md)](../top_design.md) §5.4 | [需求设计文档 (requirement/06-inspiration.md)](../requirement/06-inspiration.md)

---

## 1. 技术架构详细设计

### 1.1 模块在系统中的位置

灵感熔炉作为核心服务层的一个 Service，位于 API 层和基础设施层之间：

```
┌─────────────────────────────────────────────────────────────────┐
│                        API 层 (Axum)                             │
│  handlers/inspiration.rs ── 解析 get_inspiration 请求             │
└─────────────────────────────┬───────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                   Inspiration Service                             │
│                                                                   │
│  ┌──────────────┐  ┌──────────────┐  ┌───────────────────────┐  │
│  │ ConceptPool  │  │  Concept     │  │  LlmCreative          │  │
│  │ Builder      │→ │  Selector    │→ │  Generator            │  │
│  │ (概念池构建)  │  │ (概念选择)    │  │ (LLM 创意生成)        │  │
│  └──────┬───────┘  └──────────────┘  └───────────┬───────────┘  │
│         │                                        │              │
│  ┌──────┴───────┐  ┌──────────────┐  ┌───────────┴───────────┐  │
│  │  Tag TF-IDF  │  │  Distance    │  │  Result               │  │
│  │  Calculator  │  │  Matrix      │  │  Formatter            │  │
│  │ (标签权重)    │  │ (距离矩阵)    │  │ (结果格式化)           │  │
│  └──────────────┘  └──────────────┘  └───────────┬───────────┘  │
│                                                  │              │
│                                       ┌──────────┴──────────┐   │
│                                       │  History Manager     │   │
│                                       │  (历史记录管理)       │   │
│                                       └─────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
        │              │              │                │
        ▼              ▼              ▼                ▼
   ┌────────┐   ┌──────────┐   ┌──────────┐   ┌──────────┐
   │Memory  │   │Timeline  │   │CodeRepo  │   │  LLM     │
   │Service │   │Service   │   │Service   │   │  Client  │
   └────────┘   └──────────┘   └──────────┘   └──────────┘
```

### 1.2 请求处理流程

```
get_inspiration(type, note_path?)
    │
    ├── type == "concept_combo"
    │       │
    │       ├── 1. ConceptPoolBuilder.build() → ConceptPool
    │       │      ├── 从 MemoryService 获取全量标签 + TF-IDF 权重
    │       │      ├── 从笔记标题提取关键词
    │       │      └── 从 CodeRepoService 获取仓库名 + 技术栈
    │       │
    │       ├── 2. ConceptSelector.select_pair(pool) → (ConceptA, ConceptB)
    │       │      ├── 随机选取第一个概念
    │       │      └── 距离加权选取第二个概念
    │       │
    │       ├── 3. LlmCreativeGenerator.concept_combo(a, b) → String
    │       │      ├── 搜索与 A、B 相关的笔记作为上下文
    │       │      └── 组装 prompt → 调用 LLM
    │       │
    │       └── 4. ResultFormatter.format_concept_combo(...) → InspirationResult
    │              ├── 生成 Obsidian URI 链接
    │              └── 写入历史记录
    │
    ├── type == "reverse_question"
    │       │
    │       ├── 1. 确定目标笔记（指定 or 最近修改）
    │       ├── 2. 读取笔记内容
    │       ├── 3. LlmCreativeGenerator.reverse_question(content) → Vec<Question>
    │       └── 4. ResultFormatter.format_reverse_question(...) → InspirationResult
    │
    └── type == "counterpoint"
            │
            ├── 1. 校验 note_path 必填 + 笔记字数 > 300
            ├── 2. 读取笔记内容
            ├── 3. LlmCreativeGenerator.counterpoint(content) → Vec<Counterpoint>
            └── 4. ResultFormatter.format_counterpoint(...) → InspirationResult
```

### 1.3 数据流总览

```
┌─────────────┐     ┌──────────────┐     ┌──────────────┐     ┌─────────────┐
│  概念选取     │────→│  Prompt 组装  │────→│  LLM 调用     │────→│  结果格式化   │
│             │     │              │     │              │     │             │
│ ConceptPool │     │ PromptBuilder│     │ LlmClient    │     │ Formatter   │
│ + Selector  │     │ + Template   │     │ .generate()  │     │ + History   │
└──────┬──────┘     └──────────────┘     └──────────────┘     └──────┬──────┘
       │                                                             │
       ├── 标签 TF-IDF                                                ├── Obsidian URI
       ├── 概念距离矩阵                                                ├── 结构化 JSON
       └── 历史去重                                                   └── SQLite 写入
```

---

## 2. 目录与文件组织

### 2.1 文件布局

```
src/
├── core/
│   ├── mod.rs                  // 导出所有 core 模块
│   └── inspiration.rs          // 灵感熔炉主模块（本文档核心）
├── api/
│   └── handlers/
│       └── inspiration.rs      // get_inspiration 请求处理器
├── tools/
│   └── definitions.rs          // get_inspiration 工具 schema 定义
└── models/
    └── inspiration.rs          // 灵感相关数据模型
```

### 2.2 `src/core/inspiration.rs` 模块内部结构

该文件采用单文件多 struct 的方式组织（Rust 惯例），内部通过注释分区：

```rust
// src/core/inspiration.rs
//
// 灵感熔炉 (Inspiration Forge)
// 故意制造知识碰撞，用用户自己的笔记和代码库为原料产生新想法。
//
// 内部分区：
//   1. 数据模型（struct / enum）
//   2. ConceptPoolBuilder - 概念池构建器
//   3. ConceptSelector - 概念选择器
//   4. LlmCreativeGenerator - LLM 创意生成器
//   5. ResultFormatter - 结果格式化器
//   6. HistoryManager - 灵感历史管理器
//   7. InspirationService - 统一入口
```

---

## 3. 各子模块详细设计

### 3.1 概念池构建器（ConceptPoolBuilder）

#### 3.1.1 职责

从 vault 笔记和代码仓库中构建一个加权的概念池，并计算概念间的距离矩阵。

#### 3.1.2 核心数据结构

```rust
use std::collections::HashMap;
use std::path::PathBuf;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 概念来源
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConceptSource {
    /// 来自笔记标签
    NoteTag { note_paths: Vec<PathBuf> },
    /// 来自笔记标题/关键词
    NoteKeyword { note_paths: Vec<PathBuf> },
    /// 来自代码仓库
    CodeRepo { repo_name: String, tech_stack: Vec<String> },
}

/// 概念池中的一个概念
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Concept {
    /// 概念名称（标签名、关键词、仓库名/技术栈）
    pub term: String,
    /// 概念的 TF-IDF 权重（越高越有代表性）
    pub weight: f64,
    /// 概念的来源
    pub source: ConceptSource,
    /// 关联的标签列表（用于距离计算）
    pub co_tags: Vec<String>,
    /// 概念出现的笔记数量
    pub doc_frequency: usize,
}

/// 概念池
#[derive(Debug, Clone)]
pub struct ConceptPool {
    /// 所有概念，key 为概念 term
    pub concepts: HashMap<String, Concept>,
    /// 概念间距离矩阵（稀疏存储）
    /// key: (term_a, term_b) 其中 term_a < term_b（字母序保证唯一性）
    /// value: 距离值 [0.0, 1.0]，0.0 表示完全相同，1.0 表示完全无关
    pub distance_matrix: HashMap<(String, String), f64>,
    /// 构建时间
    pub built_at: DateTime<Utc>,
    /// 概念总数
    pub total_concepts: usize,
}

/// 概念池构建器
pub struct ConceptPoolBuilder {
    /// 概念池大小上限
    max_concepts: usize,
    /// 最低 TF-IDF 阈值（过滤噪音概念）
    min_tfidf: f64,
}
```

#### 3.1.3 标签提取与 TF-IDF 计算

**标签提取来源**：
- 笔记正文中的 `#tag` 格式标签
- 笔记 `frontmatter.tags` 列表
- 嵌套标签展开：`#programming/rust` 展开为 `["programming", "programming/rust"]`

**TF-IDF 计算公式**：

```
TF-IDF(tag, note) = TF(tag, note) × IDF(tag)

其中：
  TF(tag, note) = tag 在 note 中出现的次数 / note 中所有标签的总数
  IDF(tag) = ln(N / df(tag)) + 1

  N = vault 中笔记总数
  df(tag) = 包含该 tag 的笔记数量
```

**Rust 实现**：

```rust
impl ConceptPoolBuilder {
    /// 从所有笔记中提取标签并计算 TF-IDF
    ///
    /// # 参数
    /// - `notes`: vault 中所有笔记的 (path, tags) 列表
    ///
    /// # 返回
    /// 标签到 TF-IDF 权重的映射
    pub fn compute_tag_tfidf(
        &self,
        notes: &[(PathBuf, Vec<String>)],
    ) -> HashMap<String, TfidfResult> {
        let total_docs = notes.len() as f64;
        let mut doc_freq: HashMap<String, usize> = HashMap::new();
        let mut term_freq: HashMap<PathBuf, HashMap<String, f64>> = HashMap::new();

        // 第一遍：统计 df 和 tf
        for (path, tags) in notes {
            let unique_tags: std::collections::HashSet<&String> = tags.iter().collect();
            let total_tags = tags.len() as f64;

            // 文档频率：每个标签在此笔记中出现算一次
            for tag in unique_tags {
                *doc_freq.entry(tag.clone()).or_insert(0) += 1;
            }

            // 词频：标签在此笔记中的出现频率
            let mut tf_map = HashMap::new();
            for tag in tags {
                *tf_map.entry(tag.clone()).or_insert(0.0_f64) += 1.0;
            }
            // 归一化
            if total_tags > 0.0 {
                for v in tf_map.values_mut() {
                    *v /= total_tags;
                }
            }
            term_freq.insert(path.clone(), tf_map);
        }

        // 第二遍：计算 TF-IDF
        let mut tfidf_results: HashMap<String, TfidfResult> = HashMap::new();
        for (tag, &df) in &doc_freq {
            let idf = (total_docs / df as f64).ln() + 1.0;

            // 计算所有包含此标签的笔记的平均 TF-IDF
            let mut sum_tfidf = 0.0;
            let mut count = 0;
            for (_path, tf_map) in &term_freq {
                if let Some(&tf) = tf_map.get(tag) {
                    sum_tfidf += tf * idf;
                    count += 1;
                }
            }
            let avg_tfidf = if count > 0 { sum_tfidf / count as f64 } else { 0.0 };

            if avg_tfidf >= self.min_tfidf {
                tfidf_results.insert(
                    tag.clone(),
                    TfidfResult {
                        term: tag.clone(),
                        tfidf: avg_tfidf,
                        doc_frequency: df,
                    },
                );
            }
        }

        tfidf_results
    }
}

/// TF-IDF 计算结果
#[derive(Debug, Clone)]
pub struct TfidfResult {
    pub term: String,
    pub tfidf: f64,
    pub doc_frequency: usize,
}
```

#### 3.1.4 关键词提取

关键词从两个来源提取：

1. **笔记标题**：文件名（去除 `.md` 后缀）作为关键词候选
2. **高频词**：对笔记正文做分词后，按频率排序取 top-N

```rust
impl ConceptPoolBuilder {
    /// 从笔记标题提取关键词
    pub fn extract_title_keywords(
        &self,
        notes: &[(PathBuf, String)], // (path, title)
    ) -> Vec<(String, Vec<PathBuf>)> {
        notes
            .iter()
            .map(|(path, title)| {
                // 清理标题：去除日期前缀、特殊字符
                let cleaned = title
                    .trim()
                    .trim_start_matches(|c: char| c.is_ascii_digit() || c == '-' || c == ' ')
                    .to_string();
                (cleaned, vec![path.clone()])
            })
            .filter(|(title, _)| !title.is_empty() && title.len() > 1)
            .collect()
    }

    /// 从笔记正文提取高频词（使用 jieba-rs 分词）
    pub fn extract_content_keywords(
        &self,
        notes: &[(PathBuf, String)], // (path, content)
        top_n: usize,
    ) -> Vec<(String, f64, Vec<PathBuf>)> {
        use jieba_rs::Jieba;

        let jieba = Jieba::new();
        let stopwords = load_stopwords(); // 加载停用词表
        let mut word_freq: HashMap<String, (f64, Vec<PathBuf>)> = HashMap::new();
        let total_notes = notes.len() as f64;

        for (path, content) in notes {
            let words = jieba.cut(content, true);
            let mut local_freq: HashMap<String, usize> = HashMap::new();

            for word in &words {
                let w = word.trim();
                if w.len() >= 2 && !stopwords.contains(w) {
                    *local_freq.entry(w.to_string()).or_insert(0) += 1;
                }
            }

            for (word, count) in local_freq {
                let entry = word_freq.entry(word).or_insert_with(|| (0.0, Vec::new()));
                entry.0 += count as f64 / total_notes; // 简单频率归一化
                if !entry.1.contains(path) {
                    entry.1.push(path.clone());
                }
            }
        }

        let mut results: Vec<_> = word_freq
            .into_iter()
            .map(|(word, (freq, paths))| (word, freq, paths))
            .collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        results.truncate(top_n);
        results
    }
}
```

#### 3.1.5 仓库概念提取

```rust
impl ConceptPoolBuilder {
    /// 从代码仓库提取技术概念
    pub fn extract_repo_concepts(
        &self,
        repos: &[RepoInfo],
    ) -> Vec<Concept> {
        repos
            .iter()
            .flat_map(|repo| {
                let mut concepts = Vec::new();

                // 仓库名本身作为一个概念
                concepts.push(Concept {
                    term: repo.name.clone(),
                    weight: 2.0, // 仓库名给予较高基础权重
                    source: ConceptSource::CodeRepo {
                        repo_name: repo.name.clone(),
                        tech_stack: repo.languages.keys().cloned().collect(),
                    },
                    co_tags: vec![], // 仓库概念没有 co_tags，用 tech_stack 代替
                    doc_frequency: 1,
                });

                // 主要技术栈（占比 > 10%）作为概念
                for (lang, ratio) in &repo.languages {
                    if *ratio > 0.1 {
                        concepts.push(Concept {
                            term: format!("{} ({})", lang, repo.name),
                            weight: *ratio as f64 * 3.0,
                            source: ConceptSource::CodeRepo {
                                repo_name: repo.name.clone(),
                                tech_stack: vec![lang.clone()],
                            },
                            co_tags: vec![],
                            doc_frequency: 1,
                        });
                    }
                }

                concepts
            })
            .collect()
    }
}

/// 仓库信息（从 CodeRepoService 获取）
#[derive(Debug, Clone)]
pub struct RepoInfo {
    pub name: String,
    pub path: PathBuf,
    pub languages: HashMap<String, f32>,
    pub linked_notes: Vec<PathBuf>,
}
```

#### 3.1.6 概念距离矩阵

**距离定义**：两个概念间的距离基于它们的标签共现度（co-occurrence）。距离值域 [0.0, 1.0]，其中 0.0 表示完全相同（总是共同出现），1.0 表示完全无关（从未共同出现）。

```rust
impl ConceptPoolBuilder {
    /// 计算概念间距离矩阵
    ///
    /// 距离基于标签共现度：
    ///   co_occurrence(A, B) = |notes_with_A ∩ notes_with_B| / |notes_with_A ∪ notes_with_B|
    ///   distance(A, B) = 1.0 - co_occurrence(A, B)
    ///
    /// 对于代码仓库概念，使用关联笔记的标签作为共现基础。
    pub fn compute_distance_matrix(
        &self,
        concepts: &HashMap<String, Concept>,
        note_tag_map: &HashMap<PathBuf, Vec<String>>,
    ) -> HashMap<(String, String), f64> {
        // 构建概念 → 出现笔记集合的映射
        let mut concept_notes: HashMap<String, HashSet<PathBuf>> = HashMap::new();

        for (term, concept) in concepts {
            let notes_set: HashSet<PathBuf> = match &concept.source {
                ConceptSource::NoteTag { note_paths } => note_paths.iter().cloned().collect(),
                ConceptSource::NoteKeyword { note_paths } => note_paths.iter().cloned().collect(),
                ConceptSource::CodeRepo { repo_name, .. } => {
                    // 仓库概念：使用关联笔记，如果没有则使用名称匹配的笔记
                    HashSet::new() // 由外部传入 repo_linked_notes 补充
                }
            };
            concept_notes.insert(term.clone(), notes_set);
        }

        // 计算两两距离
        let terms: Vec<&String> = concepts.keys().collect();
        let mut matrix = HashMap::new();

        for i in 0..terms.len() {
            for j in (i + 1)..terms.len() {
                let term_a = terms[i];
                let term_b = terms[j];

                let notes_a = concept_notes.get(term_a.as_str()).cloned().unwrap_or_default();
                let notes_b = concept_notes.get(term_b.as_str()).cloned().unwrap_or_default();

                let intersection = notes_a.intersection(&notes_b).count() as f64;
                let union = notes_a.union(&notes_b).count() as f64;

                let co_occurrence = if union > 0.0 { intersection / union } else { 0.0 };
                let distance = 1.0 - co_occurrence;

                // 字母序保证 key 唯一性
                let key = if term_a < term_b {
                    (term_a.clone(), term_b.clone())
                } else {
                    (term_b.clone(), term_a.clone())
                };

                matrix.insert(key, distance);
            }
        }

        matrix
    }

    /// 查询两个概念间的距离
    pub fn get_distance(
        matrix: &HashMap<(String, String), f64>,
        term_a: &str,
        term_b: &str,
    ) -> f64 {
        if term_a == term_b {
            return 0.0;
        }
        let key = if term_a < term_b {
            (term_a.to_string(), term_b.to_string())
        } else {
            (term_b.to_string(), term_a.to_string())
        };
        // 如果没有记录（例如仓库概念），默认距离为 0.8（较远）
        *matrix.get(&key).unwrap_or(&0.8)
    }
}
```

#### 3.1.7 概念池构建完整流程

```rust
impl ConceptPoolBuilder {
    pub fn new(max_concepts: usize, min_tfidf: f64) -> Self {
        Self {
            max_concepts,
            min_tfidf,
        }
    }

    /// 构建完整概念池
    pub async fn build(
        &self,
        memory_service: &MemoryService,
        code_repo_service: &CodeRepoService,
    ) -> Result<ConceptPool, BrainError> {
        // 1. 获取所有笔记的标签
        let all_tags = memory_service.get_all_tags_with_notes().await?;
        // all_tags: Vec<(String, Vec<PathBuf>)>  -- (tag, note_paths)

        // 2. 计算标签 TF-IDF
        let notes_for_tfidf: Vec<(PathBuf, Vec<String>)> = all_tags
            .iter()
            .flat_map(|(tag, paths)| paths.iter().map(|p| (p.clone(), vec![tag.clone()])))
            .collect();
        let tfidf_results = self.compute_tag_tfidf(&notes_for_tfidf);

        // 3. 获取笔记标题关键词
        let note_titles = memory_service.get_all_note_titles().await?;
        let title_keywords = self.extract_title_keywords(&note_titles);

        // 4. 获取代码仓库概念
        let repos = code_repo_service.list_repos().await?;
        let repo_concepts = self.extract_repo_concepts(&repos);

        // 5. 合并概念
        let mut concepts: HashMap<String, Concept> = HashMap::new();

        // 标签概念
        for (tag, result) in &tfidf_results {
            let note_paths = all_tags
                .iter()
                .find(|(t, _)| t == tag)
                .map(|(_, paths)| paths.clone())
                .unwrap_or_default();

            // co_tags: 与当前标签共同出现最多的其他标签
            let co_tags = self.compute_co_tags(tag, &all_tags);

            concepts.insert(
                tag.clone(),
                Concept {
                    term: tag.clone(),
                    weight: result.tfidf,
                    source: ConceptSource::NoteTag { note_paths },
                    co_tags,
                    doc_frequency: result.doc_frequency,
                },
            );
        }

        // 关键词概念
        for (keyword, paths) in &title_keywords {
            if !concepts.contains_key(keyword) {
                concepts.insert(
                    keyword.clone(),
                    Concept {
                        term: keyword.clone(),
                        weight: 1.0, // 关键词基础权重
                        source: ConceptSource::NoteKeyword {
                            note_paths: paths.clone(),
                        },
                        co_tags: vec![],
                        doc_frequency: paths.len(),
                    },
                );
            }
        }

        // 仓库概念
        for concept in repo_concepts {
            concepts.insert(concept.term.clone(), concept);
        }

        // 6. 按权重排序，截取 max_concepts
        if concepts.len() > self.max_concepts {
            let mut sorted: Vec<_> = concepts.into_iter().collect();
            sorted.sort_by(|a, b| b.1.weight.partial_cmp(&a.1.weight).unwrap());
            sorted.truncate(self.max_concepts);
            concepts = sorted.into_iter().collect();
        }

        // 7. 计算距离矩阵
        let note_tag_map = memory_service.get_note_tag_map().await?;
        let distance_matrix = self.compute_distance_matrix(&concepts, &note_tag_map);

        Ok(ConceptPool {
            total_concepts: concepts.len(),
            concepts,
            distance_matrix,
            built_at: Utc::now(),
        })
    }

    /// 计算与指定标签共同出现频率最高的标签列表
    fn compute_co_tags(
        &self,
        target_tag: &str,
        all_tags: &[(String, Vec<PathBuf>)],
    ) -> Vec<String> {
        let target_paths: HashSet<PathBuf> = all_tags
            .iter()
            .find(|(t, _)| t == target_tag)
            .map(|(_, paths)| paths.iter().cloned().collect())
            .unwrap_or_default();

        let mut co_occur: HashMap<String, usize> = HashMap::new();
        for (tag, paths) in all_tags {
            if tag == target_tag {
                continue;
            }
            let overlap = paths.iter().filter(|p| target_paths.contains(p)).count();
            if overlap > 0 {
                *co_occur.entry(tag.clone()).or_insert(0) += overlap;
            }
        }

        let mut sorted: Vec<_> = co_occur.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
        sorted.truncate(20); // 保留 top-20 co-tags
        sorted.into_iter().map(|(tag, _)| tag).collect()
    }
}
```

### 3.2 概念选择器（ConceptSelector）

#### 3.2.1 职责

从概念池中选择两个"距离合适"的概念对——距离足够远以产生跨界联想，但不是完全无关。

#### 3.2.2 核心数据结构

```rust
/// 概念选择器配置
#[derive(Debug, Clone)]
pub struct SelectorConfig {
    /// 最小距离阈值（低于此值的概念对不会被选中）
    /// 默认 0.6，表示至少要有 60% 的不共现度
    pub min_distance: f64,
    /// 最大距离阈值（高于此值的概念对不会被选中）
    /// 默认 0.95，避免选取完全无关的概念
    pub max_distance: f64,
    /// 距离加权指数（越高越偏向远距离概念）
    /// 默认 2.0
    pub distance_weight_power: f64,
    /// 历史去重天数
    pub dedup_days: u32,
}

impl Default for SelectorConfig {
    fn default() -> Self {
        Self {
            min_distance: 0.6,
            max_distance: 0.95,
            distance_weight_power: 2.0,
            dedup_days: 7,
        }
    }
}

/// 概念选择器
pub struct ConceptSelector {
    config: SelectorConfig,
    rng: rand::rngs::ThreadRng,
}
```

#### 3.2.3 选择算法

```rust
impl ConceptSelector {
    pub fn new(config: SelectorConfig) -> Self {
        Self {
            config,
            rng: rand::thread_rng(),
        }
    }

    /// 选择一个概念对
    ///
    /// # 算法
    /// 1. 从概念池中按权重随机选取第一个概念
    /// 2. 从历史记录中获取近期已使用的概念对，用于去重
    /// 3. 筛选候选的第二个概念：
    ///    - 与第一个概念的距离在 [min_distance, max_distance] 区间内
    ///    - 不在近期去重列表中
    /// 4. 按距离加权随机选取第二个概念（距离越远权重越高，但不是完全无关）
    pub fn select_pair(
        &mut self,
        pool: &ConceptPool,
        recent_pairs: &[(String, String)], // 近 dedup_days 天内的历史概念对
    ) -> Result<(Concept, Concept), BrainError> {
        if pool.concepts.len() < 2 {
            return Err(BrainError::Internal(
                "概念池中的概念不足，需要至少 2 个概念".into(),
            ));
        }

        // Step 1: 按权重随机选取第一个概念
        let concept_a = self.weighted_random_select(&pool.concepts)?;

        // Step 2: 筛选候选的第二个概念
        let candidates: Vec<(&String, f64)> = pool
            .concepts
            .iter()
            .filter(|(term, _)| *term != &concept_a.term)
            .filter_map(|(term, concept)| {
                let distance = ConceptPoolBuilder::get_distance(
                    &pool.distance_matrix,
                    &concept_a.term,
                    term,
                );

                // 距离在阈值范围内
                if distance < self.config.min_distance || distance > self.config.max_distance {
                    return None;
                }

                // 去重检查
                let is_duplicate = recent_pairs.iter().any(|(a, b)| {
                    (a == &concept_a.term && b == term) || (b == &concept_a.term && a == term)
                });
                if is_duplicate {
                    return None;
                }

                Some((term, distance))
            })
            .collect();

        if candidates.is_empty() {
            // 降级：放宽距离阈值
            tracing::warn!("无候选概念对，放宽距离阈值重新选择");
            return self.select_pair_relaxed(pool, &concept_a, recent_pairs);
        }

        // Step 3: 距离加权随机选取第二个概念
        // 权重 = distance ^ power（距离越远权重越高）
        let weights: Vec<f64> = candidates
            .iter()
            .map(|(_, d)| d.powf(self.config.distance_weight_power))
            .collect();

        let idx = weighted_random_index(&weights, &mut self.rng);
        let (term_b, _distance) = &candidates[idx];
        let concept_b = pool.concepts.get(*term_b).unwrap().clone();

        Ok((concept_a, concept_b))
    }

    /// 按权重随机选取一个概念
    fn weighted_random_select(
        &mut self,
        concepts: &HashMap<String, Concept>,
    ) -> Result<Concept, BrainError> {
        let items: Vec<(&String, &Concept)> = concepts.iter().collect();
        let weights: Vec<f64> = items.iter().map(|(_, c)| c.weight).collect();
        let idx = weighted_random_index(&weights, &mut self.rng);
        Ok(items[idx].1.clone())
    }

    /// 放宽条件的选择（降级策略）
    fn select_pair_relaxed(
        &mut self,
        pool: &ConceptPool,
        concept_a: &Concept,
        _recent_pairs: &[(String, String)],
    ) -> Result<(Concept, Concept), BrainError> {
        // 放宽到任意距离，仅排除完全相同的概念
        let candidates: Vec<&Concept> = pool
            .concepts
            .values()
            .filter(|c| c.term != concept_a.term)
            .collect();

        if candidates.is_empty() {
            return Err(BrainError::Internal(
                "概念池中无可用概念对".into(),
            ));
        }

        let idx = rand::Rng::gen_range(&mut self.rng, 0..candidates.len());
        Ok((concept_a.clone(), candidates[idx].clone()))
    }
}

/// 按权重随机选择索引
fn weighted_random_index(weights: &[f64], rng: &mut impl rand::Rng) -> usize {
    let total: f64 = weights.iter().sum();
    if total <= 0.0 {
        return rng.gen_range(0..weights.len());
    }
    let mut threshold = rng.gen::<f64>() * total;
    for (i, &w) in weights.iter().enumerate() {
        threshold -= w;
        if threshold <= 0.0 {
            return i;
        }
    }
    weights.len() - 1
}
```

### 3.3 LLM 创意生成器（LlmCreativeGenerator）

#### 3.3.1 职责

负责组装 prompt 并调用 LLM 生成三种模式的创意输出。

#### 3.3.2 核心数据结构

```rust
/// LLM 创意生成器
pub struct LlmCreativeGenerator {
    llm_client: Arc<LlmClient>,
    /// concept_combo 模式的 temperature
    combo_temperature: f32,
    /// reverse_question 模式的 temperature
    question_temperature: f32,
    /// counterpoint 模式的 temperature
    counterpoint_temperature: f32,
    /// 最大输出 token 数
    max_tokens: u32,
}

/// LLM 生成的概念组合创意
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComboOutput {
    /// 创意联想文本
    pub inspiration: String,
    /// 具体实践建议列表
    pub suggestions: Vec<String>,
    /// 可能的实验方案
    pub experiment_idea: Option<String>,
}

/// LLM 生成的反向问题
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionOutput {
    /// 问题文本
    pub question: String,
    /// 为什么这个问题值得思考
    pub why_it_matters: String,
    /// 问题类型
    pub question_type: QuestionType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QuestionType {
    /// 假设反事实
    Counterfactual,
    /// 延伸应用场景
    Extension,
    /// 逻辑一致性检验
    LogicCheck,
    /// 时间维度推演
    TemporalProjection,
}

/// LLM 生成的对立观点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CounterpointOutput {
    /// 原文中的主张
    pub claim: String,
    /// 反方观点
    pub counter: String,
    /// 逻辑漏洞分析
    pub weakness: String,
    /// 完善建议
    pub suggestion: String,
}

/// 完整的 counterpoint 输出
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullCounterpointOutput {
    pub counterpoints: Vec<CounterpointOutput>,
    pub overall_assessment: String,
}
```

#### 3.3.3 Prompt 模板 — concept_combo 模式

```rust
impl LlmCreativeGenerator {
    /// concept_combo 模式的完整 prompt
    ///
    /// 设计要点：
    /// - 明确两个概念及其来源上下文
    /// - 要求"合理且有启发性"的联想，避免牵强
    /// - 要求具体的实践建议，不是空泛的口号
    /// - 使用 JSON 格式输出以便结构化解析
    pub fn build_concept_combo_prompt(
        &self,
        concept_a: &Concept,
        concept_b: &Concept,
        related_notes_a: &[NoteSnippet],
        related_notes_b: &[NoteSnippet],
    ) -> String {
        let context_a = related_notes_a
            .iter()
            .map(|n| format!("- 笔记「{}」: {}", n.title, n.snippet))
            .collect::<Vec<_>>()
            .join("\n");

        let context_b = related_notes_b
            .iter()
            .map(|n| format!("- 笔记「{}」: {}", n.title, n.snippet))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
r#"你是一个创意催化剂，擅长在不同领域的知识之间建立出人意料的联系。

## 任务

请基于以下两个概念，生成一个跨界创意联想。这个联想应该是合理的、有启发性的，而不是牵强的。

## 概念 A：{term_a}
- 来源：{source_a}
- 相关笔记内容：
{context_a}

## 概念 B：{term_b}
- 来源：{source_b}
- 相关笔记内容：
{context_b}

## 输出要求

请以 JSON 格式输出，包含以下字段：

```json
{{
  "inspiration": "一段 200-400 字的跨界创意联想，描述这两个概念如何产生意想不到的关联。请用第二人称（'你'）称呼读者，语气启发而非说教。",
  "suggestions": [
    "具体实践建议 1：可执行的具体行动步骤",
    "具体实践建议 2：可执行的具体行动步骤",
    "具体实践建议 3：可执行的具体行动步骤"
  ],
  "experiment_idea": "一个可以在 1-2 周内完成的小型实验方案，用于验证这个跨界想法的可行性。如果两个概念确实难以设计实验，此字段可以为 null。"
}}
```

## 质量标准
- 联想应该让读者产生"确实没想到可以这样联系"的感觉
- 实践建议必须具体到可执行，不要"多思考"、"深入研究"这类空泛建议
- 如果两个概念确实难以建立合理联系，可以承认这一点，但仍然尝试找到一个有趣的角度

请直接输出 JSON，不要有其他文字。"#,
            term_a = concept_a.term,
            source_a = format_concept_source(&concept_a.source),
            context_a = if context_a.is_empty() { "（无直接相关笔记）".to_string() } else { context_a },
            term_b = concept_b.term,
            source_b = format_concept_source(&concept_b.source),
            context_b = if context_b.is_empty() { "（无直接相关笔记）".to_string() } else { context_b },
        )
    }
}
```

#### 3.3.4 Prompt 模板 — reverse_question 模式

```rust
impl LlmCreativeGenerator {
    /// reverse_question 模式的完整 prompt
    ///
    /// 设计要点：
    /// - 深入理解笔记的核心论点
    /// - 生成的问题超越表面，触及隐含假设和盲点
    /// - 每个问题分属不同类型
    /// - 附带"为什么值得思考"的说明
    pub fn build_reverse_question_prompt(
        &self,
        note_title: &str,
        note_content: &str,
        note_tags: &[String],
        related_note_titles: &[String],
    ) -> String {
        let tags_str = if note_tags.is_empty() {
            "无".to_string()
        } else {
            note_tags.join(", ")
        };

        let related_str = if related_note_titles.is_empty() {
            "无".to_string()
        } else {
            related_note_titles.join(", ")
        };

        // 截断过长的笔记内容（控制 prompt 长度）
        let content_truncated = if note_content.len() > 6000 {
            format!("{}...\n（内容已截断）", &note_content[..6000])
        } else {
            note_content.to_string()
        };

        format!(
r#"你是一个苏格拉底式的提问者，擅长通过深刻的问题帮助人们发现自己思维中的盲点和隐含假设。

## 任务

阅读以下笔记，然后生成 3 个作者可能从未想过但值得深入思考的问题。

## 笔记信息
- 标题：{title}
- 标签：{tags}
- 引用/关联的其他笔记：{related}

## 笔记内容

{content}

## 问题设计要求

1. **问题必须与笔记内容相关**，但要超越表面——不要问笔记中已经回答了的问题
2. **三个问题应分属不同类型**：
   - 假设反事实（如果笔记中的某个前提不成立会怎样？）
   - 延伸应用（笔记中的想法能否应用到一个完全意想不到的领域？）
   - 逻辑一致性检验（笔记中的不同论点之间是否存在矛盾？）
   - 时间维度推演（笔记中的观点在 10 年后还成立吗？10 年前呢？）
3. **每个问题附带简短说明**：为什么这个问题值得思考，它可能揭示什么

## 输出要求

请以 JSON 格式输出：

```json
{{
  "questions": [
    {{
      "question": "问题的完整表述",
      "why_it_matters": "2-3 句话说明为什么这个问题值得思考",
      "question_type": "counterfactual | extension | logic_check | temporal_projection"
    }},
    {{
      "question": "...",
      "why_it_matters": "...",
      "question_type": "..."
    }},
    {{
      "question": "...",
      "why_it_matters": "...",
      "question_type": "..."
    }}
  ]
}}
```

## 质量标准
- 问题应该让读者产生"确实没从这个角度想过"的感觉
- 避免问笔记中已有明确答案的问题
- 问题应该是开放性的，没有唯一正确答案
- 如果笔记内容太短或太浅，提出能引导作者深入探索的方向性问题

请直接输出 JSON，不要有其他文字。"#,
            title = note_title,
            tags = tags_str,
            related = related_str,
            content = content_truncated,
        )
    }
}
```

#### 3.3.5 Prompt 模板 — counterpoint 模式

```rust
impl LlmCreativeGenerator {
    /// counterpoint 模式的完整 prompt
    ///
    /// 设计要点：
    /// - 识别笔记中的核心主张和论证结构
    /// - 对每个主张生成反方观点
    /// - 指出逻辑漏洞和未覆盖的论证角度
    /// - 提供完善论证的具体建议
    pub fn build_counterpoint_prompt(
        &self,
        note_title: &str,
        note_content: &str,
        note_tags: &[String],
    ) -> String {
        let tags_str = if note_tags.is_empty() {
            "无".to_string()
        } else {
            note_tags.join(", ")
        };

        // 截断过长的笔记内容
        let content_truncated = if note_content.len() > 6000 {
            format!("{}...\n（内容已截断）", &note_content[..6000])
        } else {
            note_content.to_string()
        };

        format!(
r#"你是一位严谨的学术审稿人和"魔鬼代言人"，你的任务是帮助作者发现自己论证中的盲点和薄弱环节。

## 任务

阅读以下笔记，识别其中的核心主张，然后对每个主张生成反方观点、指出逻辑漏洞，并提供完善论证的建议。

## 笔记信息
- 标题：{title}
- 标签：{tags}

## 笔记内容

{content}

## 分析要求

### 1. 识别核心主张
找出笔记中 2-4 个核心主张（有明确立场的陈述，不是事实描述）。

### 2. 对每个主张生成反方观点
- 提供至少一个有力的反方论点
- 引用历史上的反例、替代解释或对立理论

### 3. 指出逻辑漏洞
- 样本偏差（基于有限的经验过度概括）
- 确认偏差（只看到支持自己观点的证据）
- 隐含假设（未经论证就被当作前提的假设）
- 因果混淆（相关性被误认为因果性）
- 时间局限（当前趋势未必持续）

### 4. 提供完善建议
- 具体说明作者应该如何补强论证
- 建议应该讨论哪些未覆盖的角度
- 推荐可以参考的反方资料方向

## 输出要求

请以 JSON 格式输出：

```json
{{
  "counterpoints": [
    {{
      "claim": "笔记中的主张原文（简要概括）",
      "counter": "反方观点（100-200 字）",
      "weakness": "逻辑漏洞分析（50-150 字）",
      "suggestion": "完善论证的具体建议（50-150 字）"
    }}
  ],
  "overall_assessment": "整体评估（100-200 字）：论证的整体强度、最薄弱的环节、最值得优先完善的方向"
}}
```

## 质量标准
- 反方观点应该有力且有建设性，不是为了反对而反对
- 逻辑漏洞分析应该具体，指出是哪类谬误或偏差
- 完善建议必须可操作，不要"多角度思考"这类空泛建议
- 如果笔记的论证本身很强，诚实地承认，但仍指出可以进一步加强的方向

请直接输出 JSON，不要有其他文字。"#,
            title = note_title,
            tags = tags_str,
            content = content_truncated,
        )
    }
}
```

#### 3.3.6 LLM 调用与输出解析

```rust
impl LlmCreativeGenerator {
    pub fn new(
        llm_client: Arc<LlmClient>,
        combo_temperature: f32,
        question_temperature: f32,
        counterpoint_temperature: f32,
        max_tokens: u32,
    ) -> Self {
        Self {
            llm_client,
            combo_temperature,
            question_temperature,
            counterpoint_temperature,
            max_tokens,
        }
    }

    /// concept_combo 模式生成
    pub async fn generate_concept_combo(
        &self,
        concept_a: &Concept,
        concept_b: &Concept,
        related_notes_a: &[NoteSnippet],
        related_notes_b: &[NoteSnippet],
    ) -> Result<ComboOutput, BrainError> {
        let prompt = self.build_concept_combo_prompt(
            concept_a,
            concept_b,
            related_notes_a,
            related_notes_b,
        );

        let response = self
            .llm_client
            .generate(
                &prompt,
                self.combo_temperature,
                self.max_tokens,
                Some("json_object"), // 要求 JSON 格式输出
            )
            .await
            .map_err(|e| BrainError::LlmApiError {
                provider: "llm".into(),
                detail: format!("concept_combo LLM 调用失败: {}", e),
            })?;

        // 解析 JSON 输出
        let output: ComboOutput = serde_json::from_str(&response).map_err(|e| {
            BrainError::Internal(format!("解析 LLM concept_combo 输出失败: {} | 原始输出: {}", e, response))
        })?;

        Ok(output)
    }

    /// reverse_question 模式生成
    pub async fn generate_reverse_question(
        &self,
        note_title: &str,
        note_content: &str,
        note_tags: &[String],
        related_note_titles: &[String],
    ) -> Result<Vec<QuestionOutput>, BrainError> {
        let prompt = self.build_reverse_question_prompt(
            note_title,
            note_content,
            note_tags,
            related_note_titles,
        );

        let response = self
            .llm_client
            .generate(
                &prompt,
                self.question_temperature,
                self.max_tokens,
                Some("json_object"),
            )
            .await
            .map_err(|e| BrainError::LlmApiError {
                provider: "llm".into(),
                detail: format!("reverse_question LLM 调用失败: {}", e),
            })?;

        #[derive(Deserialize)]
        struct Wrapper {
            questions: Vec<QuestionOutput>,
        }
        let wrapper: Wrapper = serde_json::from_str(&response).map_err(|e| {
            BrainError::Internal(format!(
                "解析 LLM reverse_question 输出失败: {} | 原始输出: {}",
                e, response
            ))
        })?;

        // 确保恰好 3 个问题
        if wrapper.questions.is_empty() {
            return Err(BrainError::Internal(
                "LLM 未生成任何问题".into(),
            ));
        }

        Ok(wrapper.questions.into_iter().take(3).collect())
    }

    /// counterpoint 模式生成
    pub async fn generate_counterpoint(
        &self,
        note_title: &str,
        note_content: &str,
        note_tags: &[String],
    ) -> Result<FullCounterpointOutput, BrainError> {
        let prompt = self.build_counterpoint_prompt(note_title, note_content, note_tags);

        let response = self
            .llm_client
            .generate(
                &prompt,
                self.counterpoint_temperature,
                self.max_tokens,
                Some("json_object"),
            )
            .await
            .map_err(|e| BrainError::LlmApiError {
                provider: "llm".into(),
                detail: format!("counterpoint LLM 调用失败: {}", e),
            })?;

        let output: FullCounterpointOutput = serde_json::from_str(&response).map_err(|e| {
            BrainError::Internal(format!(
                "解析 LLM counterpoint 输出失败: {} | 原始输出: {}",
                e, response
            ))
        })?;

        Ok(output)
    }
}

/// 笔记片段（用于 prompt 上下文）
#[derive(Debug, Clone)]
pub struct NoteSnippet {
    pub title: String,
    pub path: PathBuf,
    pub snippet: String, // 200 字以内的摘要
}

/// 格式化概念来源描述
fn format_concept_source(source: &ConceptSource) -> String {
    match source {
        ConceptSource::NoteTag { note_paths } => {
            let paths_str = note_paths
                .iter()
                .take(3)
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            format!("笔记标签（相关笔记: {}）", paths_str)
        }
        ConceptSource::NoteKeyword { note_paths } => {
            let paths_str = note_paths
                .iter()
                .take(3)
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            format!("笔记标题/关键词（相关笔记: {}）", paths_str)
        }
        ConceptSource::CodeRepo {
            repo_name,
            tech_stack,
        } => {
            format!(
                "代码仓库 '{}' (技术栈: {})",
                repo_name,
                tech_stack.join(", ")
            )
        }
    }
}
```

### 3.4 结果格式化器（ResultFormatter）

#### 3.4.1 职责

将 LLM 的原始输出格式化为标准的 API 响应，包含 Obsidian 链接和历史记录写入。

#### 3.4.2 核心数据结构

```rust
/// 灵感结果格式化器
pub struct ResultFormatter {
    vault_name: String,
    vault_path: PathBuf,
}

/// 统一的灵感结果（API 响应）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum InspirationResult {
    #[serde(rename = "concept_combo")]
    ConceptCombo {
        concept_a: ConceptRef,
        concept_b: ConceptRef,
        inspiration: String,
        suggestions: Vec<String>,
        experiment_idea: Option<String>,
        related_notes: Vec<String>,
        related_repos: Vec<RepoRef>,
        generated_at: DateTime<Utc>,
    },
    #[serde(rename = "reverse_question")]
    ReverseQuestion {
        note: NoteRef,
        questions: Vec<QuestionItem>,
        generated_at: DateTime<Utc>,
    },
    #[serde(rename = "counterpoint")]
    Counterpoint {
        note: NoteRef,
        counterpoints: Vec<CounterpointItem>,
        overall_assessment: String,
        related_notes: Vec<String>,
        generated_at: DateTime<Utc>,
    },
}

/// 概念引用（带链接）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConceptRef {
    pub term: String,
    pub source: String,           // "note_tag" | "note_keyword" | "code_repo"
    pub source_path: Option<String>,
    pub obsidian_uri: Option<String>,
}

/// 笔记引用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteRef {
    pub path: String,
    pub title: String,
    pub obsidian_uri: String,
}

/// 仓库引用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoRef {
    pub name: String,
    pub vscode_uri: String,
}

/// 问题项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionItem {
    pub question: String,
    pub why_it_matters: String,
    pub question_type: String,
    pub related_notes: Vec<String>,
}

/// 对立观点项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CounterpointItem {
    pub claim: String,
    pub counter: String,
    pub weakness: String,
    pub suggestion: String,
}
```

#### 3.4.3 格式化方法

```rust
impl ResultFormatter {
    pub fn new(vault_name: String, vault_path: PathBuf) -> Self {
        Self {
            vault_name,
            vault_path,
        }
    }

    /// 生成 Obsidian URI
    fn obsidian_uri(&self, note_path: &str) -> String {
        format!(
            "obsidian://open?vault={}&file={}",
            self.vault_name,
            urlencoding::encode(note_path)
        )
    }

    /// 生成 VSCode URI
    fn vscode_uri(&self, repo_path: &Path) -> String {
        format!("vscode://file/{}", repo_path.display())
    }

    /// 格式化 concept_combo 结果
    pub fn format_concept_combo(
        &self,
        concept_a: &Concept,
        concept_b: &Concept,
        combo_output: &ComboOutput,
        related_note_paths: &[PathBuf],
        related_repos: &[RepoInfo],
    ) -> InspirationResult {
        let concept_a_ref = self.concept_to_ref(concept_a);
        let concept_b_ref = self.concept_to_ref(concept_b);

        let related_notes: Vec<String> = related_note_paths
            .iter()
            .map(|p| p.display().to_string())
            .collect();

        let related_repos: Vec<RepoRef> = related_repos
            .iter()
            .map(|r| RepoRef {
                name: r.name.clone(),
                vscode_uri: self.vscode_uri(&r.path),
            })
            .collect();

        // 组合 inspiration 文本：主体 + 建议 + 实验想法
        let mut inspiration = combo_output.inspiration.clone();
        if !combo_output.suggestions.is_empty() {
            inspiration.push_str("\n\n**实践建议：**\n");
            for (i, s) in combo_output.suggestions.iter().enumerate() {
                inspiration.push_str(&format!("{}. {}\n", i + 1, s));
            }
        }
        if let Some(ref exp) = combo_output.experiment_idea {
            inspiration.push_str(&format!("\n**实验方案：**\n{}\n", exp));
        }

        InspirationResult::ConceptCombo {
            concept_a: concept_a_ref,
            concept_b: concept_b_ref,
            inspiration,
            suggestions: combo_output.suggestions.clone(),
            experiment_idea: combo_output.experiment_idea.clone(),
            related_notes,
            related_repos,
            generated_at: Utc::now(),
        }
    }

    /// 格式化 reverse_question 结果
    pub fn format_reverse_question(
        &self,
        note_path: &str,
        note_title: &str,
        questions: Vec<QuestionOutput>,
        related_notes_map: &HashMap<String, Vec<PathBuf>>, // question_type -> related paths
    ) -> InspirationResult {
        let note_ref = NoteRef {
            path: note_path.to_string(),
            title: note_title.to_string(),
            obsidian_uri: self.obsidian_uri(note_path),
        };

        let question_items: Vec<QuestionItem> = questions
            .into_iter()
            .map(|q| {
                let type_str = match q.question_type {
                    QuestionType::Counterfactual => "counterfactual",
                    QuestionType::Extension => "extension",
                    QuestionType::LogicCheck => "logic_check",
                    QuestionType::TemporalProjection => "temporal_projection",
                };
                let related = related_notes_map
                    .get(type_str)
                    .map(|paths| paths.iter().map(|p| p.display().to_string()).collect())
                    .unwrap_or_default();

                QuestionItem {
                    question: q.question,
                    why_it_matters: q.why_it_matters,
                    question_type: type_str.to_string(),
                    related_notes: related,
                }
            })
            .collect();

        InspirationResult::ReverseQuestion {
            note: note_ref,
            questions: question_items,
            generated_at: Utc::now(),
        }
    }

    /// 格式化 counterpoint 结果
    pub fn format_counterpoint(
        &self,
        note_path: &str,
        note_title: &str,
        output: &FullCounterpointOutput,
        related_note_paths: &[PathBuf],
    ) -> InspirationResult {
        let note_ref = NoteRef {
            path: note_path.to_string(),
            title: note_title.to_string(),
            obsidian_uri: self.obsidian_uri(note_path),
        };

        let counterpoint_items: Vec<CounterpointItem> = output
            .counterpoints
            .iter()
            .map(|cp| CounterpointItem {
                claim: cp.claim.clone(),
                counter: cp.counter.clone(),
                weakness: cp.weakness.clone(),
                suggestion: cp.suggestion.clone(),
            })
            .collect();

        let related_notes: Vec<String> = related_note_paths
            .iter()
            .map(|p| p.display().to_string())
            .collect();

        InspirationResult::Counterpoint {
            note: note_ref,
            counterpoints: counterpoint_items,
            overall_assessment: output.overall_assessment.clone(),
            related_notes,
            generated_at: Utc::now(),
        }
    }

    fn concept_to_ref(&self, concept: &Concept) -> ConceptRef {
        let (source, path, uri) = match &concept.source {
            ConceptSource::NoteTag { note_paths } => {
                let path = note_paths.first().map(|p| p.display().to_string());
                let uri = note_paths.first().map(|p| self.obsidian_uri(&p.display().to_string()));
                ("note_tag", path, uri)
            }
            ConceptSource::NoteKeyword { note_paths } => {
                let path = note_paths.first().map(|p| p.display().to_string());
                let uri = note_paths.first().map(|p| self.obsidian_uri(&p.display().to_string()));
                ("note_keyword", path, uri)
            }
            ConceptSource::CodeRepo { .. } => ("code_repo", None, None),
        };

        ConceptRef {
            term: concept.term.clone(),
            source: source.to_string(),
            source_path: path,
            obsidian_uri: uri,
        }
    }
}
```

### 3.5 灵感历史管理器（HistoryManager）

#### 3.5.1 职责

管理 `inspiration_history` 表，实现历史记录存储、查询和去重。

#### 3.5.2 核心数据结构

```rust
/// 灵感历史记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspirationRecord {
    pub id: Uuid,
    pub inspiration_type: InspirationType,
    pub input_refs: serde_json::Value,
    pub output: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InspirationType {
    ConceptCombo,
    ReverseQuestion,
    Counterpoint,
}

/// 灵感历史管理器
pub struct HistoryManager {
    db: Arc<rusqlite::Connection>,
}
```

#### 3.5.3 数据库操作

```rust
impl HistoryManager {
    pub fn new(db: Arc<rusqlite::Connection>) -> Self {
        Self { db }
    }

    /// 初始化数据库表（如不存在）
    pub fn init_table(&self) -> Result<(), BrainError> {
        self.db.execute(
            "CREATE TABLE IF NOT EXISTS inspiration_history (
                id          TEXT PRIMARY KEY,
                type        TEXT NOT NULL,
                input_refs  JSON,
                output      TEXT NOT NULL,
                created_at  DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        ).map_err(|e| BrainError::Internal(format!("创建 inspiration_history 表失败: {}", e)))?;

        // 创建索引以加速去重查询
        self.db.execute(
            "CREATE INDEX IF NOT EXISTS idx_inspiration_type_created
             ON inspiration_history (type, created_at)",
            [],
        ).map_err(|e| BrainError::Internal(format!("创建索引失败: {}", e)))?;

        Ok(())
    }

    /// 保存灵感记录
    pub fn save_record(
        &self,
        inspiration_type: InspirationType,
        input_refs: &serde_json::Value,
        output: &str,
    ) -> Result<Uuid, BrainError> {
        let id = Uuid::new_v4();
        let type_str = match inspiration_type {
            InspirationType::ConceptCombo => "concept_combo",
            InspirationType::ReverseQuestion => "reverse_question",
            InspirationType::Counterpoint => "counterpoint",
        };
        let input_json = serde_json::to_string(input_refs)
            .map_err(|e| BrainError::Internal(format!("序列化 input_refs 失败: {}", e)))?;

        self.db.execute(
            "INSERT INTO inspiration_history (id, type, input_refs, output)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![id.to_string(), type_str, input_json, output],
        ).map_err(|e| BrainError::Internal(format!("写入灵感历史失败: {}", e)))?;

        Ok(id)
    }

    /// 查询近期概念组合对（用于去重）
    ///
    /// 返回近 `days` 天内所有 concept_combo 的概念对
    pub fn get_recent_concept_pairs(
        &self,
        days: u32,
    ) -> Result<Vec<(String, String)>, BrainError> {
        let cutoff = Utc::now() - chrono::Duration::days(days as i64);
        let mut stmt = self.db.prepare(
            "SELECT input_refs FROM inspiration_history
             WHERE type = 'concept_combo' AND created_at >= ?1",
        ).map_err(|e| BrainError::Internal(format!("查询历史失败: {}", e)))?;

        let pairs: Vec<(String, String)> = stmt
            .query_map(rusqlite::params![cutoff.to_rfc3339()], |row| {
                let json_str: String = row.get(0)?;
                Ok(json_str)
            })
            .map_err(|e| BrainError::Internal(format!("查询历史失败: {}", e)))?
            .filter_map(|r| r.ok())
            .filter_map(|json_str| {
                // 从 input_refs JSON 中提取概念对
                let v: serde_json::Value = serde_json::from_str(&json_str).ok()?;
                let a = v.get("concept_a")?.as_str()?.to_string();
                let b = v.get("concept_b")?.as_str()?.to_string();
                Some((a, b))
            })
            .collect();

        Ok(pairs)
    }

    /// 查询历史记录列表
    pub fn list_records(
        &self,
        inspiration_type: Option<InspirationType>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<InspirationRecord>, BrainError> {
        let type_filter = inspiration_type.map(|t| match t {
            InspirationType::ConceptCombo => "concept_combo",
            InspirationType::ReverseQuestion => "reverse_question",
            InspirationType::Counterpoint => "counterpoint",
        });

        let (query, params) = if let Some(ref t) = type_filter {
            (
                "SELECT id, type, input_refs, output, created_at
                 FROM inspiration_history
                 WHERE type = ?1
                 ORDER BY created_at DESC
                 LIMIT ?2 OFFSET ?3",
                rusqlite::params![t, limit as i64, offset as i64],
            )
        } else {
            (
                "SELECT id, type, input_refs, output, created_at
                 FROM inspiration_history
                 ORDER BY created_at DESC
                 LIMIT ?1 OFFSET ?2",
                rusqlite::params![limit as i64, offset as i64],
            )
        };

        let mut stmt = self.db.prepare(query)
            .map_err(|e| BrainError::Internal(format!("查询历史失败: {}", e)))?;

        let records: Vec<InspirationRecord> = stmt
            .query_map(params, |row| {
                let id_str: String = row.get(0)?;
                let type_str: String = row.get(1)?;
                let input_refs_str: String = row.get(2)?;
                let output: String = row.get(3)?;
                let created_at_str: String = row.get(4)?;

                Ok(InspirationRecord {
                    id: Uuid::parse_str(&id_str).unwrap_or(Uuid::nil()),
                    inspiration_type: match type_str.as_str() {
                        "concept_combo" => InspirationType::ConceptCombo,
                        "reverse_question" => InspirationType::ReverseQuestion,
                        "counterpoint" => InspirationType::Counterpoint,
                        _ => InspirationType::ConceptCombo, // fallback
                    },
                    input_refs: serde_json::from_str(&input_refs_str)
                        .unwrap_or(serde_json::Value::Null),
                    output,
                    created_at: DateTime::parse_from_rfc3339(&created_at_str)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                })
            })
            .map_err(|e| BrainError::Internal(format!("查询历史失败: {}", e)))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(records)
    }

    /// 清理过期记录（保留期限 1 年）
    pub fn cleanup_expired(&self) -> Result<usize, BrainError> {
        let cutoff = Utc::now() - chrono::Duration::days(365);
        let deleted = self.db.execute(
            "DELETE FROM inspiration_history WHERE created_at < ?1",
            rusqlite::params![cutoff.to_rfc3339()],
        ).map_err(|e| BrainError::Internal(format!("清理历史记录失败: {}", e)))?;

        Ok(deleted)
    }
}
```

---

## 4. 统一入口：InspirationService

### 4.1 Service 定义

```rust
use std::sync::Arc;
use tokio::sync::RwLock;

/// 灵感熔炉服务 — 对外暴露的统一入口
pub struct InspirationService {
    /// 概念池构建器
    pool_builder: ConceptPoolBuilder,
    /// 概念选择器
    selector: RwLock<ConceptSelector>,
    /// LLM 创意生成器
    generator: LlmCreativeGenerator,
    /// 结果格式化器
    formatter: ResultFormatter,
    /// 历史管理器
    history: HistoryManager,
    /// 概念池缓存
    cached_pool: RwLock<Option<ConceptPool>>,
    /// 缓存 TTL（秒）
    cache_ttl_seconds: i64,
    /// 依赖的服务
    memory_service: Arc<MemoryService>,
    timeline_service: Arc<TimelineService>,
    code_repo_service: Arc<CodeRepoService>,
}

/// 灵感服务配置
#[derive(Debug, Clone, Deserialize)]
pub struct InspirationConfig {
    /// 概念池大小上限
    #[serde(default = "default_max_concepts")]
    pub max_concepts: usize,
    /// 最低 TF-IDF 阈值
    #[serde(default = "default_min_tfidf")]
    pub min_tfidf: f64,
    /// 最小概念距离
    #[serde(default = "default_min_distance")]
    pub min_distance: f64,
    /// 最大概念距离
    #[serde(default = "default_max_distance")]
    pub max_distance: f64,
    /// 概念池缓存 TTL（秒）
    #[serde(default = "default_cache_ttl")]
    pub cache_ttl_seconds: i64,
    /// concept_combo temperature
    #[serde(default = "default_combo_temp")]
    pub combo_temperature: f32,
    /// reverse_question temperature
    #[serde(default = "default_question_temp")]
    pub question_temperature: f32,
    /// counterpoint temperature
    #[serde(default = "default_counterpoint_temp")]
    pub counterpoint_temperature: f32,
    /// LLM 最大输出 token
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
}

fn default_max_concepts() -> usize { 5000 }
fn default_min_tfidf() -> f64 { 0.01 }
fn default_min_distance() -> f64 { 0.6 }
fn default_max_distance() -> f64 { 0.95 }
fn default_cache_ttl() -> i64 { 3600 }
fn default_combo_temp() -> f32 { 0.9 }
fn default_question_temp() -> f32 { 0.8 }
fn default_counterpoint_temp() -> f32 { 0.7 }
fn default_max_tokens() -> u32 { 2048 }
```

### 4.2 核心方法

```rust
impl InspirationService {
    /// 创建灵感服务实例
    pub fn new(
        config: InspirationConfig,
        vault_name: String,
        vault_path: PathBuf,
        db: Arc<rusqlite::Connection>,
        llm_client: Arc<LlmClient>,
        memory_service: Arc<MemoryService>,
        timeline_service: Arc<TimelineService>,
        code_repo_service: Arc<CodeRepoService>,
    ) -> Result<Self, BrainError> {
        let pool_builder = ConceptPoolBuilder::new(config.max_concepts, config.min_tfidf);
        let selector_config = SelectorConfig {
            min_distance: config.min_distance,
            max_distance: config.max_distance,
            ..Default::default()
        };
        let selector = ConceptSelector::new(selector_config);
        let generator = LlmCreativeGenerator::new(
            llm_client,
            config.combo_temperature,
            config.question_temperature,
            config.counterpoint_temperature,
            config.max_tokens,
        );
        let formatter = ResultFormatter::new(vault_name, vault_path);
        let history = HistoryManager::new(db);
        history.init_table()?;

        Ok(Self {
            pool_builder,
            selector: RwLock::new(selector),
            generator,
            formatter,
            history,
            cached_pool: RwLock::new(None),
            cache_ttl_seconds: config.cache_ttl_seconds,
            memory_service,
            timeline_service,
            code_repo_service,
        })
    }

    /// 获取或构建概念池（带缓存）
    async fn get_or_build_pool(&self) -> Result<ConceptPool, BrainError> {
        // 检查缓存
        {
            let cached = self.cached_pool.read().await;
            if let Some(ref pool) = *cached {
                let age = Utc::now() - pool.built_at;
                if age.num_seconds() < self.cache_ttl_seconds {
                    return Ok(pool.clone());
                }
            }
        }

        // 缓存过期或不存在，重新构建
        let pool = self
            .pool_builder
            .build(&self.memory_service, &self.code_repo_service)
            .await?;

        {
            let mut cached = self.cached_pool.write().await;
            *cached = Some(pool.clone());
        }

        Ok(pool)
    }

    /// 统一入口：获取灵感
    pub async fn get_inspiration(
        &self,
        inspiration_type: Option<&str>,
        note_path: Option<&str>,
    ) -> Result<InspirationResult, BrainError> {
        match inspiration_type.unwrap_or("concept_combo") {
            "concept_combo" => self.handle_concept_combo().await,
            "reverse_question" => self.handle_reverse_question(note_path).await,
            "counterpoint" => self.handle_counterpoint(note_path).await,
            other => Err(BrainError::Internal(format!(
                "未知的灵感类型: '{}'，支持: concept_combo, reverse_question, counterpoint",
                other
            ))),
        }
    }

    /// 处理 concept_combo 模式
    async fn handle_concept_combo(&self) -> Result<InspirationResult, BrainError> {
        // 1. 获取概念池
        let pool = self.get_or_build_pool().await?;

        if pool.total_concepts < 2 {
            return Err(BrainError::Internal(
                "概念池中的概念不足（至少需要 2 个），请先积累更多带标签的笔记".into(),
            ));
        }

        // 2. 获取历史概念对（去重用）
        let recent_pairs = self
            .history
            .get_recent_concept_pairs(7)
            .unwrap_or_default();

        // 3. 选择概念对
        let (concept_a, concept_b) = {
            let mut selector = self.selector.write().await;
            selector.select_pair(&pool, &recent_pairs)?
        };

        // 4. 搜索相关笔记作为 LLM 上下文
        let related_notes_a = self.search_related_notes(&concept_a).await;
        let related_notes_b = self.search_related_notes(&concept_b).await;

        // 5. LLM 生成
        let combo_output = self
            .generator
            .generate_concept_combo(&concept_a, &concept_b, &related_notes_a, &related_notes_b)
            .await?;

        // 6. 收集相关链接
        let related_note_paths = self.collect_related_paths(&concept_a, &concept_b);
        let related_repos = self.collect_related_repos(&concept_a, &concept_b).await;

        // 7. 格式化结果
        let result = self.formatter.format_concept_combo(
            &concept_a,
            &concept_b,
            &combo_output,
            &related_note_paths,
            &related_repos,
        );

        // 8. 写入历史
        let input_refs = serde_json::json!({
            "concept_a": concept_a.term,
            "concept_b": concept_b.term,
        });
        let output_text = serde_json::to_string(&result).unwrap_or_default();
        let _ = self.history.save_record(
            InspirationType::ConceptCombo,
            &input_refs,
            &output_text,
        );

        Ok(result)
    }

    /// 处理 reverse_question 模式
    async fn handle_reverse_question(
        &self,
        note_path: Option<&str>,
    ) -> Result<InspirationResult, BrainError> {
        // 1. 确定目标笔记
        let (path, title, content, tags) = self.resolve_note(note_path).await?;

        // 2. 校验字数
        let char_count = content.chars().count();
        if char_count < 200 {
            return Err(BrainError::Internal(format!(
                "笔记 '{}' 内容过短（{}字），反向提问需要至少 200 字",
                path, char_count
            )));
        }

        // 3. 获取关联笔记标题
        let related_titles = self.get_related_note_titles(&path).await;

        // 4. LLM 生成
        let questions = self
            .generator
            .generate_reverse_question(&title, &content, &tags, &related_titles)
            .await?;

        // 5. 格式化结果
        let related_map = HashMap::new(); // 可扩展：根据问题搜索相关笔记
        let result = self
            .formatter
            .format_reverse_question(&path, &title, questions, &related_map);

        // 6. 写入历史
        let input_refs = serde_json::json!({
            "note_path": path,
        });
        let output_text = serde_json::to_string(&result).unwrap_or_default();
        let _ = self.history.save_record(
            InspirationType::ReverseQuestion,
            &input_refs,
            &output_text,
        );

        Ok(result)
    }

    /// 处理 counterpoint 模式
    async fn handle_counterpoint(
        &self,
        note_path: Option<&str>,
    ) -> Result<InspirationResult, BrainError> {
        // 1. 校验 note_path 必填
        let note_path = note_path.ok_or_else(|| {
            BrainError::Internal(
                "counterpoint 模式必须指定 note_path 参数".into(),
            )
        })?;

        // 2. 读取笔记
        let (path, title, content, tags) = self.resolve_note(Some(note_path)).await?;

        // 3. 校验字数
        let char_count = content.chars().count();
        if char_count < 300 {
            return Err(BrainError::Internal(format!(
                "笔记 '{}' 内容过短（{}字），对立观点生成需要至少 300 字",
                path, char_count
            )));
        }

        // 4. LLM 生成
        let output = self
            .generator
            .generate_counterpoint(&title, &content, &tags)
            .await?;

        // 5. 搜索相关笔记
        let related_paths = self
            .memory_service
            .search(&title, 3, &tags)
            .await
            .unwrap_or_default()
            .iter()
            .map(|m| m.note_path.clone())
            .collect::<Vec<_>>();

        // 6. 格式化结果
        let result = self
            .formatter
            .format_counterpoint(&path, &title, &output, &related_paths);

        // 7. 写入历史
        let input_refs = serde_json::json!({
            "note_path": path,
        });
        let output_text = serde_json::to_string(&result).unwrap_or_default();
        let _ = self.history.save_record(
            InspirationType::Counterpoint,
            &input_refs,
            &output_text,
        );

        Ok(result)
    }

    // === 辅助方法 ===

    /// 解析笔记路径，获取笔记信息
    async fn resolve_note(
        &self,
        note_path: Option<&str>,
    ) -> Result<(String, String, String, Vec<String>), BrainError> {
        let path = match note_path {
            Some(p) => p.to_string(),
            None => {
                // 自动选择最近修改的笔记
                let recent = self.timeline_service.get_recent_notes(7).await?;
                recent
                    .into_iter()
                    .find(|n| {
                        // 排除过短的笔记
                        n.word_count > 200
                    })
                    .map(|n| n.path.display().to_string())
                    .ok_or_else(|| {
                        BrainError::Internal("未找到合适的笔记，请指定 note_path 参数".into())
                    })?
            }
        };

        let note = self.memory_service.get_note(&path).await?;
        Ok((
            path,
            note.title,
            note.content,
            note.tags,
        ))
    }

    /// 搜索与概念相关的笔记片段
    async fn search_related_notes(&self, concept: &Concept) -> Vec<NoteSnippet> {
        let results = self
            .memory_service
            .search(&concept.term, 3, &[])
            .await
            .unwrap_or_default();

        results
            .into_iter()
            .map(|m| NoteSnippet {
                title: m.summary.clone().unwrap_or_else(|| m.content.chars().take(50).collect()),
                path: m.note_path.clone(),
                snippet: m.content.chars().take(200).collect::<String>(),
            })
            .collect()
    }

    /// 收集两个概念相关的笔记路径
    fn collect_related_paths(&self, a: &Concept, b: &Concept) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        match &a.source {
            ConceptSource::NoteTag { note_paths } | ConceptSource::NoteKeyword { note_paths } => {
                paths.extend(note_paths.iter().take(3).cloned());
            }
            _ => {}
        }
        match &b.source {
            ConceptSource::NoteTag { note_paths } | ConceptSource::NoteKeyword { note_paths } => {
                for p in note_paths.iter().take(3) {
                    if !paths.contains(p) {
                        paths.push(p.clone());
                    }
                }
            }
            _ => {}
        }
        paths
    }

    /// 收集两个概念相关的仓库
    async fn collect_related_repos(&self, a: &Concept, b: &Concept) -> Vec<RepoInfo> {
        let mut repos = Vec::new();
        for concept in [a, b] {
            if let ConceptSource::CodeRepo { repo_name, .. } = &concept.source {
                if let Ok(detail) = self.code_repo_service.get_repo_detail(repo_name).await {
                    repos.push(RepoInfo {
                        name: detail.name,
                        path: detail.path,
                        languages: detail.language_stats,
                        linked_notes: detail.linked_notes,
                    });
                }
            }
        }
        repos
    }

    /// 获取笔记关联的其他笔记标题
    async fn get_related_note_titles(&self, _note_path: &str) -> Vec<String> {
        // 可扩展：解析 [[wikilinks]] 获取关联笔记
        // 当前版本返回空列表
        Vec::new()
    }
}
```

---

## 5. API Handler 层

### 5.1 请求处理器

```rust
// src/api/handlers/inspiration.rs

use axum::{
    extract::State,
    Json,
};
use serde::{Deserialize, Serialize};

/// get_inspiration 请求参数
#[derive(Debug, Deserialize)]
pub struct GetInspirationRequest {
    #[serde(rename = "type")]
    pub inspiration_type: Option<String>,
    pub note_path: Option<String>,
}

/// get_inspiration 响应
#[derive(Debug, Serialize)]
pub struct GetInspirationResponse {
    pub tool: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<InspirationResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorResponse>,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub code: String,
    pub message: String,
    pub suggestion: Option<String>,
}

/// get_inspiration 请求处理器
pub async fn handle_get_inspiration(
    State(state): State<AppState>,
    Json(request): Json<GetInspirationRequest>,
) -> Json<GetInspirationResponse> {
    match state
        .inspiration_service
        .get_inspiration(
            request.inspiration_type.as_deref(),
            request.note_path.as_deref(),
        )
        .await
    {
        Ok(result) => Json(GetInspirationResponse {
            tool: "get_inspiration".to_string(),
            status: "success".to_string(),
            result: Some(result),
            error: None,
        }),
        Err(e) => {
            let (code, message, suggestion) = match &e {
                BrainError::NoteNotFound(path) => (
                    "NOTE_NOT_FOUND",
                    format!("笔记 '{}' 未找到", path.display()),
                    Some("请使用 list_recent_notes 查看可用笔记".to_string()),
                ),
                BrainError::LlmApiError { detail, .. } => (
                    "LLM_API_ERROR",
                    detail.clone(),
                    Some("请稍后重试，或检查 LLM API 配置".to_string()),
                ),
                _ => (
                    "INTERNAL_ERROR",
                    e.to_string(),
                    None,
                ),
            };

            tracing::error!("get_inspiration 失败: {}", e);

            Json(GetInspirationResponse {
                tool: "get_inspiration".to_string(),
                status: "error".to_string(),
                result: None,
                error: Some(ErrorResponse {
                    code: code.to_string(),
                    message,
                    suggestion,
                }),
            })
        }
    }
}
```

### 5.2 工具 Schema 定义

```rust
// src/tools/definitions.rs 中的 get_inspiration 定义

pub fn get_inspiration_schema() -> serde_json::Value {
    serde_json::json!({
        "name": "get_inspiration",
        "description": "从用户的知识库中生成灵感。支持三种模式：随机概念组合（concept_combo）——选取两个距离较远的概念生成跨界联想；反向提问（reverse_question）——对一篇笔记生成深入问题；对立观点（counterpoint）——对指定笔记生成反方观点。",
        "inputSchema": {
            "type": "object",
            "properties": {
                "type": {
                    "type": "string",
                    "description": "灵感模式。'concept_combo': 随机概念组合（默认）；'reverse_question': 反向提问；'counterpoint': 对立观点",
                    "enum": ["concept_combo", "reverse_question", "counterpoint"],
                    "default": "concept_combo"
                },
                "note_path": {
                    "type": "string",
                    "description": "目标笔记路径（vault 内相对路径）。concept_combo 模式不需要此参数；reverse_question 可选（默认最近修改的笔记）；counterpoint 必填。"
                }
            }
        }
    })
}
```

---

## 6. 完整的请求/响应示例

### 6.1 concept_combo 示例

**请求**：
```json
{
  "tool": "get_inspiration",
  "arguments": {
    "type": "concept_combo"
  }
}
```

**响应**：
```json
{
  "tool": "get_inspiration",
  "status": "success",
  "result": {
    "type": "concept_combo",
    "concept_a": {
      "term": "缓存替换策略",
      "source": "note_tag",
      "source_path": "cs/cache-algorithms.md",
      "obsidian_uri": "obsidian://open?vault=brain&file=cs%2Fcache-algorithms.md"
    },
    "concept_b": {
      "term": "睡眠实验",
      "source": "note_keyword",
      "source_path": "life/sleep-experiment.md",
      "obsidian_uri": "obsidian://open?vault=brain&file=life%2Fsleep-experiment.md"
    },
    "inspiration": "你有没有想过，你的大脑本身就是一个缓存系统？...\n\n**实践建议：**\n1. 为每天的数据打'访问时间戳'\n2. 超过 7 天未回顾的数据自动合并为周摘要\n3. 观察哪些'冷数据'偶尔被'重新加载'\n\n**实验方案：**\n在下周的睡眠记录中，试着用 LRU 思路管理...",
    "suggestions": [
      "为每天的数据打'访问时间戳'",
      "超过 7 天未回顾的数据自动合并为周摘要",
      "观察哪些'冷数据'偶尔被'重新加载'"
    ],
    "experiment_idea": "在下周的睡眠记录中，试着用 LRU 思路管理...",
    "related_notes": [
      "cs/cache-algorithms.md",
      "life/sleep-experiment.md",
      "cs/data-structures.md"
    ],
    "related_repos": [
      {
        "name": "cache-sim",
        "vscode_uri": "vscode://file/Users/me/projects/cache-sim"
      }
    ],
    "generated_at": "2026-05-29T10:30:00Z"
  }
}
```

### 6.2 reverse_question 示例

**请求**：
```json
{
  "tool": "get_inspiration",
  "arguments": {
    "type": "reverse_question",
    "note_path": "essays/sleep-experiment.md"
  }
}
```

**响应**：
```json
{
  "tool": "get_inspiration",
  "status": "success",
  "result": {
    "type": "reverse_question",
    "note": {
      "path": "essays/sleep-experiment.md",
      "title": "我的睡眠实验：30天早睡记录",
      "obsidian_uri": "obsidian://open?vault=brain&file=essays%2Fsleep-experiment.md"
    },
    "questions": [
      {
        "question": "如果你的睡眠数据中，'感觉良好'和实际睡眠时长呈负相关...",
        "why_it_matters": "你的笔记假设了'睡够 8 小时最好'，但数据可能揭示...",
        "question_type": "counterfactual",
        "related_notes": ["health/cortisol-notes.md"]
      },
      {
        "question": "如果把你的睡眠记录方法和缓存命中率概念类比...",
        "why_it_matters": "跨领域类比可能产生全新的量化框架...",
        "question_type": "extension",
        "related_notes": ["cs/cache-algorithms.md"]
      },
      {
        "question": "30 天后你会停止记录吗？如果记录行为本身改变了你的睡眠...",
        "why_it_matters": "这是所有自我量化实验的根本方法论问题...",
        "question_type": "logic_check",
        "related_notes": []
      }
    ],
    "generated_at": "2026-05-29T10:35:00Z"
  }
}
```

### 6.3 counterpoint 示例

**请求**：
```json
{
  "tool": "get_inspiration",
  "arguments": {
    "type": "counterpoint",
    "note_path": "essays/ai-future.md"
  }
}
```

**响应**：
```json
{
  "tool": "get_inspiration",
  "status": "success",
  "result": {
    "type": "counterpoint",
    "note": {
      "path": "essays/ai-future.md",
      "title": "AI 将在 5 年内取代大部分白领工作",
      "obsidian_uri": "obsidian://open?vault=brain&file=essays%2Fai-future.md"
    },
    "counterpoints": [
      {
        "claim": "AI 将在 5 年内取代大部分白领工作",
        "counter": "历史上每次技术革命都伴随'大规模失业'的预测...",
        "weakness": "你的论证基于当前 AI 能力的线性外推...",
        "suggestion": "增加一节讨论'为什么这次可能不同'..."
      },
      {
        "claim": "编程工作将首先被替代",
        "counter": "编程的核心价值不在写代码，而在理解问题域...",
        "weakness": "你将'编程'等同于'代码生产'...",
        "suggestion": "区分'代码生成'和'软件工程'..."
      }
    ],
    "overall_assessment": "你的文章在技术趋势分析上有力，但在历史对比和社会学因素上论证不足...",
    "related_notes": ["essays/tech-history.md", "economics/automation-paradox.md"],
    "generated_at": "2026-05-29T10:40:00Z"
  }
}
```

### 6.4 错误响应示例

**概念池为空**：
```json
{
  "tool": "get_inspiration",
  "status": "error",
  "error": {
    "code": "INTERNAL_ERROR",
    "message": "概念池中的概念不足（至少需要 2 个），请先积累更多带标签的笔记"
  }
}
```

**counterpoint 未指定笔记**：
```json
{
  "tool": "get_inspiration",
  "status": "error",
  "error": {
    "code": "INTERNAL_ERROR",
    "message": "counterpoint 模式必须指定 note_path 参数"
  }
}
```

**笔记过短**：
```json
{
  "tool": "get_inspiration",
  "status": "error",
  "error": {
    "code": "NOTE_NOT_FOUND",
    "message": "笔记 'essays/draft.md' 内容过短（120字），反向提问需要至少 200 字",
    "suggestion": "请选择内容更丰富的笔记，或先补充笔记内容"
  }
}
```

---

## 7. 概念距离计算算法详述

### 7.1 算法概述

概念距离是灵感熔炉的核心算法，用于衡量两个概念之间的"跨界程度"。距离值域 [0.0, 1.0]：
- `0.0`：完全相同（两个概念总是共同出现）
- `1.0`：完全无关（两个概念从未共同出现）
- `0.6-0.95`：理想的"跨界"区间（有关联但距离较远）

### 7.2 基于 Jaccard 距离的计算

**核心思想**：如果两个标签经常出现在同一篇笔记中，说明它们在用户的知识体系中关联紧密（距离近）。反之，如果很少共同出现，说明它们属于不同的知识领域（距离远）。

```
co_occurrence(A, B) = |notes_with_A ∩ notes_with_B| / |notes_with_A ∪ notes_with_B|

distance(A, B) = 1.0 - co_occurrence(A, B)
```

这就是 **Jaccard 距离**——集合论中衡量两个集合差异的标准方法。

### 7.3 不同类型概念的距离计算

| 概念 A 类型 | 概念 B 类型 | 距离计算方式 |
|---|---|---|
| NoteTag | NoteTag | Jaccard 距离（基于出现笔记集合） |
| NoteTag | NoteKeyword | NoteKeyword 映射到其来源笔记，然后计算 Jaccard 距离 |
| NoteTag | CodeRepo | CodeRepo 映射到其关联笔记（`linked_notes`），然后计算 Jaccard 距离 |
| NoteKeyword | NoteKeyword | 映射到来源笔记，计算 Jaccard 距离 |
| NoteKeyword | CodeRepo | 分别映射到来源笔记/关联笔记，计算 Jaccard 距离 |
| CodeRepo | CodeRepo | 映射到关联笔记，计算 Jaccard 距离；若无关联笔记则默认 0.8 |

### 7.4 缺失距离的处理

当两个概念的距离在矩阵中没有记录时（例如新加入的概念），使用默认值：
- 两个都是仓库概念：默认 `0.8`（假设不同项目的技术栈差异较大）
- 一个是仓库一个是笔记：默认 `0.7`（代码和笔记通常属于不同维度）
- 其他情况：默认 `0.75`

### 7.5 算法复杂度

| 操作 | 复杂度 | 说明 |
|---|---|---|
| 距离矩阵构建 | O(n²) | n = 概念数量，两两计算 |
| 单次距离查询 | O(1) | HashMap 查找 |
| 概念对选择 | O(n) | 遍历所有候选 |

对于 5,000 个概念，距离矩阵包含约 12.5M 个条目。使用 `HashMap<(String, String), f64>` 存储，每个条目约 100 bytes（两个 String key + f64 value），总计约 1.25GB —— **需要优化**。

### 7.6 存储优化：稀疏矩阵 + ID 映射

实际中，大多数概念对的距离为 1.0（完全无关），只需存储距离 < 1.0 的条目：

```rust
/// 优化后的距离矩阵
pub struct DistanceMatrix {
    /// 概念 term → 整数 ID 映射（减少 key 大小）
    term_to_id: HashMap<String, u32>,
    id_to_term: Vec<String>,
    /// 稀疏距离存储：只存储 distance < 1.0 的概念对
    /// key: (id_a, id_b) 其中 id_a < id_b
    distances: HashMap<(u32, u32), f32>, // 用 f32 代替 f64 节省空间
    /// 默认距离
    default_distance: f32,
}

impl DistanceMatrix {
    pub fn new(
        concepts: &[String],
        raw_distances: HashMap<(String, String), f64>,
        default_distance: f32,
    ) -> Self {
        let mut term_to_id = HashMap::new();
        let mut id_to_term = Vec::new();

        for (id, term) in concepts.iter().enumerate() {
            term_to_id.insert(term.clone(), id as u32);
            id_to_term.push(term.clone());
        }

        let mut distances = HashMap::new();
        for ((term_a, term_b), dist) in raw_distances {
            if dist < 1.0 {
                if let (Some(&id_a), Some(&id_b)) = (
                    term_to_id.get(&term_a),
                    term_to_id.get(&term_b),
                ) {
                    let key = if id_a < id_b {
                        (id_a, id_b)
                    } else {
                        (id_b, id_a)
                    };
                    distances.insert(key, dist as f32);
                }
            }
        }

        Self {
            term_to_id,
            id_to_term,
            distances,
            default_distance,
        }
    }

    /// 查询距离
    pub fn get(&self, term_a: &str, term_b: &str) -> f32 {
        if term_a == term_b {
            return 0.0;
        }
        let (id_a, id_b) = match (self.term_to_id.get(term_a), self.term_to_id.get(term_b)) {
            (Some(&a), Some(&b)) => (a, b),
            _ => return self.default_distance,
        };
        let key = if id_a < id_b { (id_a, id_b) } else { (id_b, id_a) };
        *self.distances.get(&key).unwrap_or(&self.default_distance)
    }
}
```

**优化后内存估算**：
- 假设 5,000 个概念，其中 20% 有共现（2.5M 对）
- 每个条目：`(u32, u32, f32)` = 12 bytes + HashMap 开销约 24 bytes = 36 bytes
- 总计：2.5M × 36 = **约 90MB** — 可接受
- `term_to_id` + `id_to_term`：5,000 × 80 bytes ≈ **0.4MB**

---

## 8. 错误处理

### 8.1 错误分类与处理策略

| 错误场景 | 错误类型 | 处理方式 | 用户提示 |
|---|---|---|---|
| LLM API 调用失败 | `BrainError::LlmApiError` | 重试 1 次，仍失败则返回错误 | "LLM 暂时不可用，请稍后重试" |
| LLM 输出 JSON 解析失败 | `BrainError::Internal` | 记录原始输出到日志，返回错误 | "灵感生成失败，请重试" |
| 概念池为空/概念不足 | `BrainError::Internal` | 返回错误 + 建议 | "概念不足，请先积累更多带标签的笔记" |
| 笔记不存在 | `BrainError::NoteNotFound` | 返回错误 + 可用笔记列表 | "笔记未找到，请使用 list_recent_notes 查看" |
| 笔记过短 | `BrainError::Internal` | 返回错误 + 最低字数要求 | "笔记内容过短，请选择更长的笔记" |
| counterpoint 未指定笔记 | `BrainError::Internal` | 返回错误 + 参数说明 | "counterpoint 模式需要指定 note_path" |
| 所有候选概念对都被去重 | `BrainError::Internal` | 放宽去重条件重试 | 无（内部自动处理） |
| 距离矩阵计算超时 | `BrainError::Internal` | 使用简化距离（跳过矩阵计算） | 无（内部自动处理） |

### 8.2 错误恢复流程

```
get_inspiration 调用
    │
    ├── 参数校验失败 → 直接返回错误
    │
    ├── 概念池构建
    │   ├── 成功 → 继续
    │   └── 失败
    │       ├── 标签不足 → 尝试纯关键词模式
    │       └── 完全为空 → 返回错误
    │
    ├── 概念选择
    │   ├── 成功 → 继续
    │   └── 无候选对
    │       ├── 放宽距离阈值重试
    │       └── 仍无候选 → 返回错误
    │
    ├── LLM 调用
    │   ├── 成功 → 继续
    │   └── 失败
    │       ├── 重试 1 次
    │       └── 仍失败 → 返回错误
    │
    └── 结果格式化
        ├── 成功 → 写入历史 → 返回结果
        └── 失败（罕见）→ 返回原始 LLM 输出
```

---

## 9. 性能优化

### 9.1 概念池缓存

```rust
/// 概念池缓存策略
///
/// - TTL: 默认 1 小时（可配置）
/// - 触发刷新：TTL 过期后的下一次 get_inspiration 调用
/// - 刷新方式：异步重建，不阻塞当前请求（使用过期缓存返回结果，后台更新）
impl InspirationService {
    /// 带缓存的概念池获取
    async fn get_or_build_pool(&self) -> Result<ConceptPool, BrainError> {
        let needs_refresh = {
            let cached = self.cached_pool.read().await;
            match *cached {
                Some(ref pool) => {
                    let age = Utc::now() - pool.built_at;
                    age.num_seconds() >= self.cache_ttl_seconds
                }
                None => true,
            }
        };

        if needs_refresh {
            // 尝试获取写锁来刷新
            let mut cached = self.cached_pool.write().await;
            // 双重检查：可能在等锁期间被其他线程刷新了
            let still_needs_refresh = match *cached {
                Some(ref pool) => {
                    let age = Utc::now() - pool.built_at;
                    age.num_seconds() >= self.cache_ttl_seconds
                }
                None => true,
            };

            if still_needs_refresh {
                let pool = self
                    .pool_builder
                    .build(&self.memory_service, &self.code_repo_service)
                    .await?;
                *cached = Some(pool);
            }

            Ok(cached.as_ref().unwrap().clone())
        } else {
            let cached = self.cached_pool.read().await;
            Ok(cached.as_ref().unwrap().clone())
        }
    }
}
```

### 9.2 距离矩阵预计算

距离矩阵在概念池构建时一次性计算完成，后续查询为 O(1)：

```rust
// 在 ConceptPoolBuilder::build() 中
// 概念池构建完成后，距离矩阵已经预计算并存储在 ConceptPool 中
let distance_matrix = self.compute_distance_matrix(&concepts, &note_tag_map);

// 后续 ConceptSelector::select_pair() 中的距离查询直接使用预计算结果
let distance = pool.distance_matrix.get(&concept_a.term, &concept_b.term);
```

### 9.3 笔记搜索并行化

concept_combo 模式中，搜索两个概念的相关笔记可以并行执行：

```rust
// 并行搜索两个概念的相关笔记
let (related_notes_a, related_notes_b) = tokio::join!(
    self.search_related_notes(&concept_a),
    self.search_related_notes(&concept_b),
);
```

### 9.4 LLM 调用超时控制

```rust
/// 带超时控制的 LLM 调用
async fn generate_with_timeout<F, T>(
    future: F,
    timeout_secs: u64,
    mode_name: &str,
) -> Result<T, BrainError>
where
    F: std::future::Future<Output = Result<T, BrainError>>,
{
    match tokio::time::timeout(
        std::time::Duration::from_secs(timeout_secs),
        future,
    )
    .await
    {
        Ok(result) => result,
        Err(_) => Err(BrainError::LlmApiError {
            provider: "llm".into(),
            detail: format!("{} LLM 调用超时（{}秒）", mode_name, timeout_secs),
        }),
    }
}
```

### 9.5 性能指标监控

```rust
/// 性能指标记录（使用 tracing span）
async fn handle_concept_combo(&self) -> Result<InspirationResult, BrainError> {
    let span = tracing::info_span!("inspiration_concept_combo");
    let _enter = span.enter();

    let pool_start = std::time::Instant::now();
    let pool = self.get_or_build_pool().await?;
    tracing::info!(
        pool_build_ms = pool_start.elapsed().as_millis(),
        total_concepts = pool.total_concepts,
        "概念池准备完成"
    );

    let select_start = std::time::Instant::now();
    // ... 概念选择 ...
    tracing::info!(
        select_ms = select_start.elapsed().as_millis(),
        "概念选择完成"
    );

    let llm_start = std::time::Instant::now();
    // ... LLM 调用 ...
    tracing::info!(
        llm_ms = llm_start.elapsed().as_millis(),
        "LLM 生成完成"
    );

    // ...
}
```

---

## 10. 配置集成

### 10.1 配置文件扩展

在 `config/default.toml` 中添加 `[inspiration]` 段：

```toml
[inspiration]
# 概念池大小上限
max_concepts = 5000
# 最低 TF-IDF 阈值（过滤低频噪音标签）
min_tfidf = 0.01
# 最小概念距离（低于此值的概念对不会被选中）
min_distance = 0.6
# 最大概念距离（高于此值的概念对不会被选中）
max_distance = 0.95
# 概念池缓存 TTL（秒）
cache_ttl = 3600
# concept_combo LLM temperature（越高越有创造性）
combo_temperature = 0.9
# reverse_question LLM temperature
question_temperature = 0.8
# counterpoint LLM temperature（越低越严谨）
counterpoint_temperature = 0.7
# LLM 最大输出 token
max_tokens = 2048
# LLM 调用超时（秒）
llm_timeout = 30
# 历史去重天数
dedup_days = 7
# 历史记录保留天数
history_retention_days = 365
```

### 10.2 配置加载

```rust
// src/config.rs 中添加

#[derive(Debug, Clone, Deserialize)]
pub struct InspirationConfig {
    #[serde(default = "default_max_concepts")]
    pub max_concepts: usize,
    #[serde(default = "default_min_tfidf")]
    pub min_tfidf: f64,
    #[serde(default = "default_min_distance")]
    pub min_distance: f64,
    #[serde(default = "default_max_distance")]
    pub max_distance: f64,
    #[serde(default = "default_cache_ttl")]
    pub cache_ttl: i64,
    #[serde(default = "default_combo_temp")]
    pub combo_temperature: f32,
    #[serde(default = "default_question_temp")]
    pub question_temperature: f32,
    #[serde(default = "default_counterpoint_temp")]
    pub counterpoint_temperature: f32,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(default = "default_llm_timeout")]
    pub llm_timeout: u64,
    #[serde(default = "default_dedup_days")]
    pub dedup_days: u32,
    #[serde(default = "default_history_retention")]
    pub history_retention_days: u32,
}
```

---

## 11. 测试策略

### 11.1 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // === ConceptPoolBuilder 测试 ===

    #[test]
    fn test_tfidf_calculation() {
        let builder = ConceptPoolBuilder::new(100, 0.0);
        let notes = vec![
            (PathBuf::from("a.md"), vec!["rust".into(), "async".into()]),
            (PathBuf::from("b.md"), vec!["rust".into(), "web".into()]),
            (PathBuf::from("c.md"), vec!["python".into(), "ml".into()]),
        ];

        let results = builder.compute_tag_tfidf(&notes);

        // "rust" 出现在 2/3 篇笔记中，IDF 较低
        assert!(results.contains_key("rust"));
        // "python" 只出现在 1/3 篇笔记中，IDF 较高
        assert!(results.contains_key("python"));
        assert!(results["python"].tfidf > results["rust"].tfidf);
    }

    #[test]
    fn test_distance_matrix() {
        let builder = ConceptPoolBuilder::new(100, 0.0);

        let mut concepts = HashMap::new();
        concepts.insert("rust".into(), Concept {
            term: "rust".into(),
            weight: 1.0,
            source: ConceptSource::NoteTag {
                note_paths: vec![PathBuf::from("a.md"), PathBuf::from("b.md")],
            },
            co_tags: vec![],
            doc_frequency: 2,
        });
        concepts.insert("async".into(), Concept {
            term: "async".into(),
            weight: 1.0,
            source: ConceptSource::NoteTag {
                note_paths: vec![PathBuf::from("a.md")],
            },
            co_tags: vec![],
            doc_frequency: 1,
        });
        concepts.insert("python".into(), Concept {
            term: "python".into(),
            weight: 1.0,
            source: ConceptSource::NoteTag {
                note_paths: vec![PathBuf::from("c.md")],
            },
            co_tags: vec![],
            doc_frequency: 1,
        });

        let note_tag_map = HashMap::new();
        let matrix = builder.compute_distance_matrix(&concepts, &note_tag_map);

        // rust 和 async 有共现（a.md），距离应较小
        let dist_rust_async = ConceptPoolBuilder::get_distance(&matrix, "async", "rust");
        // rust 和 python 无共现，距离应为 1.0
        let dist_rust_python = ConceptPoolBuilder::get_distance(&matrix, "python", "rust");

        assert!(dist_rust_async < dist_rust_python);
    }

    // === ConceptSelector 测试 ===

    #[test]
    fn test_select_pair_respects_distance_threshold() {
        let config = SelectorConfig {
            min_distance: 0.5,
            max_distance: 0.9,
            ..Default::default()
        };
        let mut selector = ConceptSelector::new(config);

        // 构建一个有 3 个概念的池
        let pool = build_test_pool();
        let recent_pairs = vec![];

        let (a, b) = selector.select_pair(&pool, &recent_pairs).unwrap();
        let distance = ConceptPoolBuilder::get_distance(
            &pool.distance_matrix,
            &a.term,
            &b.term,
        );

        // 距离应在阈值范围内
        assert!(distance >= 0.5);
        assert!(distance <= 0.9);
    }

    #[test]
    fn test_dedup_avoids_recent_pairs() {
        let config = SelectorConfig::default();
        let mut selector = ConceptSelector::new(config);
        let pool = build_test_pool();

        // 模拟历史：已经用过 (A, B) 组合
        let recent_pairs = vec![("concept_a".into(), "concept_b".into())];

        // 多次选择，不应返回 (concept_a, concept_b)
        for _ in 0..10 {
            let (a, b) = selector.select_pair(&pool, &recent_pairs).unwrap();
            let pair = (a.term.clone(), b.term.clone());
            assert_ne!(pair, ("concept_a".to_string(), "concept_b".to_string()));
        }
    }

    // === HistoryManager 测试 ===

    #[test]
    fn test_history_save_and_query() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        let db = Arc::new(db);
        let history = HistoryManager::new(db);
        history.init_table().unwrap();

        let input = serde_json::json!({"concept_a": "rust", "concept_b": "python"});
        let id = history
            .save_record(InspirationType::ConceptCombo, &input, "test output")
            .unwrap();

        let pairs = history.get_recent_concept_pairs(7).unwrap();
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0], ("rust".to_string(), "python".to_string()));
    }

    // === LlmCreativeGenerator 测试 ===

    #[test]
    fn test_concept_combo_prompt_structure() {
        let generator = build_test_generator();
        let concept_a = build_test_concept("rust");
        let concept_b = build_test_concept("sleep");

        let prompt = generator.build_concept_combo_prompt(
            &concept_a,
            &concept_b,
            &[],
            &[],
        );

        assert!(prompt.contains("rust"));
        assert!(prompt.contains("sleep"));
        assert!(prompt.contains("JSON"));
        assert!(prompt.contains("inspiration"));
    }

    #[test]
    fn test_reverse_question_prompt_truncation() {
        let generator = build_test_generator();
        let long_content = "x".repeat(10000);

        let prompt = generator.build_reverse_question_prompt(
            "Test",
            &long_content,
            &[],
            &[],
        );

        // 内容应被截断到 6000 字符
        assert!(prompt.contains("（内容已截断）"));
    }

    // === ResultFormatter 测试 ===

    #[test]
    fn test_obsidian_uri_encoding() {
        let formatter = ResultFormatter::new(
            "brain".to_string(),
            PathBuf::from("/vault"),
        );

        let uri = formatter.obsidian_uri("cs/cache algorithms.md");
        assert_eq!(
            uri,
            "obsidian://open?vault=brain&file=cs%2Fcache%20algorithms.md"
        );
    }
}
```

### 11.2 集成测试

```rust
/// 集成测试：完整的 concept_combo 流程
#[tokio::test]
async fn test_concept_combo_integration() {
    // 1. 设置测试环境
    let (service, _mock_llm) = setup_test_inspiration_service().await;

    // 2. 注入测试数据
    inject_test_notes(&service.memory_service).await;
    inject_test_repos(&service.code_repo_service).await;

    // 3. 调用 get_inspiration
    let result = service
        .get_inspiration(Some("concept_combo"), None)
        .await
        .unwrap();

    // 4. 验证结果
    match result {
        InspirationResult::ConceptCombo {
            concept_a,
            concept_b,
            inspiration,
            related_notes,
            ..
        } => {
            assert_ne!(concept_a.term, concept_b.term);
            assert!(!inspiration.is_empty());
            assert!(!related_notes.is_empty());
        }
        _ => panic!("期望 ConceptCombo 结果"),
    }
}

/// 集成测试：reverse_question 自动选择笔记
#[tokio::test]
async fn test_reverse_question_auto_select() {
    let (service, _mock_llm) = setup_test_inspiration_service().await;
    inject_test_notes(&service.memory_service).await;

    let result = service
        .get_inspiration(Some("reverse_question"), None)
        .await
        .unwrap();

    match result {
        InspirationResult::ReverseQuestion { questions, .. } => {
            assert!(questions.len() <= 3);
            assert!(!questions.is_empty());
        }
        _ => panic!("期望 ReverseQuestion 结果"),
    }
}

/// 集成测试：counterpoint 必须指定笔记
#[tokio::test]
async fn test_counterpoint_requires_note_path() {
    let (service, _) = setup_test_inspiration_service().await;

    let result = service.get_inspiration(Some("counterpoint"), None).await;
    assert!(result.is_err());
}
```

### 11.3 测试覆盖目标

| 模块 | 覆盖目标 | 重点场景 |
|---|---|---|
| ConceptPoolBuilder | > 80% | TF-IDF 计算正确性、距离矩阵准确性、边界条件（空输入） |
| ConceptSelector | > 85% | 距离阈值过滤、去重逻辑、降级策略 |
| LlmCreativeGenerator | > 70% | Prompt 结构完整性、JSON 解析容错 |
| ResultFormatter | > 90% | URI 编码正确性、各模式格式化 |
| HistoryManager | > 85% | CRUD 操作、去重查询、过期清理 |
| InspirationService | > 75% | 三种模式完整流程、错误处理、缓存刷新 |

---

## 12. 依赖清单

### 12.1 Cargo.toml 依赖

```toml
[dependencies]
# 已有依赖（项目级）
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v4", "serde"] }
tracing = "0.1"
rusqlite = { version = "0.31", features = ["bundled"] }
rand = "0.8"

# 灵感模块新增依赖
jieba-rs = "0.7"           # 中文分词
urlencoding = "2"          # Obsidian URI 编码

# 测试依赖
[dev-dependencies]
tokio-test = "0.4"
mockall = "0.12"           # Mock LLM Client
```

### 12.2 内部模块依赖

| 依赖模块 | 用途 | 必需 |
|---|---|---|
| `core::memory` (MemoryService) | 获取标签、搜索笔记、读取笔记内容 | 是 |
| `core::timeline` (TimelineService) | 获取近期修改的笔记列表 | 是 |
| `core::code_repo` (CodeRepoService) | 获取仓库列表和技术栈 | 否（可降级为纯笔记模式） |
| `infra::llm_client` (LlmClient) | 调用 LLM 生成创意内容 | 是 |
| `infra::sqlite_store` | SQLite 连接管理 | 是 |
| `models::note` (Note) | 笔记数据模型 | 是 |
| `error` (BrainError) | 统一错误类型 | 是 |

### 12.3 外部服务依赖

| 服务 | 用途 | 降级方案 |
|---|---|---|
| LLM API (OpenAI / Claude / Ollama) | 生成创意内容 | 无替代，返回错误提示用户 |
| SQLite | 存储灵感历史 | 可降级为不记录历史（不影响核心功能） |

---

## 13. SQLite Schema 迁移

### 13.1 迁移脚本

```sql
-- migrations/006_inspiration_history.sql
-- 灵感历史记录表

CREATE TABLE IF NOT EXISTS inspiration_history (
    id          TEXT PRIMARY KEY,
    type        TEXT NOT NULL,   -- "concept_combo" | "reverse_question" | "counterpoint"
    input_refs  JSON,            -- 输入的笔记/仓库引用
    output      TEXT NOT NULL,   -- LLM 生成的完整输出
    created_at  DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- 查询加速索引
CREATE INDEX IF NOT EXISTS idx_inspiration_type_created
    ON inspiration_history (type, created_at);

-- 去重查询索引
CREATE INDEX IF NOT EXISTS idx_inspiration_combo_recent
    ON inspiration_history (created_at)
    WHERE type = 'concept_combo';
```

---

## 14. 实施检查清单

- [ ] 定义 `models/inspiration.rs` 数据模型
- [ ] 实现 `ConceptPoolBuilder`（标签提取 + TF-IDF + 关键词 + 仓库概念）
- [ ] 实现距离矩阵计算（Jaccard 距离 + 稀疏存储优化）
- [ ] 实现 `ConceptSelector`（加权随机 + 距离阈值 + 去重）
- [ ] 实现 `LlmCreativeGenerator`（三种 prompt 模板 + JSON 解析）
- [ ] 实现 `ResultFormatter`（Obsidian URI + 结构化输出）
- [ ] 实现 `HistoryManager`（SQLite CRUD + 去重查询）
- [ ] 实现 `InspirationService` 统一入口
- [ ] 实现 API Handler (`api/handlers/inspiration.rs`)
- [ ] 注册工具 Schema (`tools/definitions.rs`)
- [ ] 添加 `[inspiration]` 配置段
- [ ] SQLite 迁移脚本
- [ ] 单元测试 + 集成测试
- [ ] 性能基准测试（概念池构建耗时、距离矩阵查询耗时）
- [ ] 文档更新（README 使用说明）
