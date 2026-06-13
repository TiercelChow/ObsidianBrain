//! 时光机小记管理器

use chrono::{Datelike, Local, Utc};
use std::sync::Arc;
use uuid::Uuid;

use crate::error::BrainError;
use crate::infra::obsidian_client::ObsidianClient;
use crate::infra::sqlite_store::SqliteStore;
use crate::models::timeline::{BrowseTimelineRequest, Memo, MemoCreateRequest, MemoQuery};

/// 时光机小记管理器
pub struct MemoManager {
    db: Arc<SqliteStore>,
    obsidian: Option<Arc<ObsidianClient>>,
}

impl MemoManager {
    pub fn new(db: Arc<SqliteStore>, obsidian: Option<Arc<ObsidianClient>>) -> Self {
        Self { db, obsidian }
    }

    /// 创建小记
    pub async fn create_memo(&self, request: MemoCreateRequest) -> Result<Memo, BrainError> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let date = now.format("%Y-%m-%d").to_string();
        let time = now.format("%H:%M:%S").to_string();

        // 生成文件路径
        let file_path = format!("Timeline/{}.md", now.format("%Y-%m"));

        // 格式化 Markdown 内容
        let mut md_content = format!("### {}\n{}\n\n", time, request.content);
        for img in &request.images {
            md_content.push_str(&format!("![[{}]]\n", img));
        }
        if !request.tags.is_empty() {
            md_content.push_str(&format!(
                "\n{}\n",
                request
                    .tags
                    .iter()
                    .map(|t| format!("#{}", t))
                    .collect::<Vec<_>>()
                    .join(" ")
            ));
        }
        md_content.push_str("\n---\n\n");

        // 写入 Obsidian 文件（如果 Obsidian 可用）
        if let Some(ref obsidian) = self.obsidian {
            let month_title = format!(
                "# {}年{}月 时光机\n\n## {}\n\n",
                now.format("%Y"),
                now.format("%m"),
                now.format("%Y-%m-%d")
            );

            // 尝试读取文件判断是否存在
            match obsidian.read_file(&file_path).await {
                Ok(existing) => {
                    // 文件存在，检查是否包含今天的日期标题
                    let today_header = format!("## {}", now.format("%Y-%m-%d"));
                    if !existing.contains(&today_header) {
                        let date_header = format!("\n## {}\n\n", now.format("%Y-%m-%d"));
                        obsidian
                            .append_file(&file_path, &date_header)
                            .await
                            .map_err(|e| {
                                BrainError::Internal(format!("追加日期标题失败: {e}"))
                            })?;
                    }
                }
                Err(_) => {
                    // 文件不存在，先写入文件头和日期标题
                    obsidian
                        .write_file(&file_path, &month_title)
                        .await
                        .map_err(|e| {
                            BrainError::Internal(format!("创建月份文件失败: {e}"))
                        })?;
                }
            }

            // 追加小记内容
            obsidian
                .append_file(&file_path, &md_content)
                .await
                .map_err(|e| BrainError::Internal(format!("写入小记文件失败: {e}")))?;
        } else {
            tracing::warn!("Obsidian API 不可用，小记仅存储到 SQLite");
        }

        // 序列化 images 和 tags 为 JSON
        let images_json =
            serde_json::to_string(&request.images).unwrap_or_else(|_| "[]".to_string());
        let tags_json =
            serde_json::to_string(&request.tags).unwrap_or_else(|_| "[]".to_string());

        // 写入 SQLite 元数据
        self.db.insert_memo(
            &id,
            &now.to_rfc3339(),
            &date,
            &request.content,
            &images_json,
            &tags_json,
            &file_path,
        )?;

        tracing::info!(id = %id, date = %date, time = %time, "小记创建成功");

        Ok(Memo {
            id,
            timestamp: now,
            date,
            content: request.content,
            images: request.images,
            tags: request.tags,
            file_path,
            created_at: now,
        })
    }

    /// 浏览时间线
    pub async fn browse_timeline(
        &self,
        request: BrowseTimelineRequest,
    ) -> Result<Vec<Memo>, BrainError> {
        let mut sql = String::from(
            "SELECT id, timestamp, date, content, images, tags, file_path, created_at FROM memos WHERE 1=1",
        );
        let mut params = Vec::new();

        if let Some(ref start) = request.start_date {
            sql.push_str(" AND date >= ?");
            params.push(start.clone());
        }
        if let Some(ref end) = request.end_date {
            sql.push_str(" AND date <= ?");
            params.push(end.clone());
        }

        sql.push_str(" ORDER BY timestamp DESC LIMIT ? OFFSET ?");
        params.push(request.limit.to_string());
        params.push(request.offset.to_string());

        let rows = self.db.query_memos(&sql, &params)?;
        let memos = rows.into_iter().map(|row| self.row_to_memo(row)).collect();

        Ok(memos)
    }

    /// 搜索小记
    pub async fn search_memos(&self, query: MemoQuery) -> Result<Vec<Memo>, BrainError> {
        let mut sql = String::from(
            "SELECT id, timestamp, date, content, images, tags, file_path, created_at FROM memos WHERE content LIKE ?",
        );
        let mut params = vec![format!("%{}%", query.query.unwrap_or_default())];

        if let Some(ref start) = query.start_date {
            sql.push_str(" AND date >= ?");
            params.push(start.clone());
        }
        if let Some(ref end) = query.end_date {
            sql.push_str(" AND date <= ?");
            params.push(end.clone());
        }
        if let Some(ref tags) = query.tags {
            for tag in tags {
                sql.push_str(" AND tags LIKE ?");
                params.push(format!("%{}%", tag));
            }
        }

        sql.push_str(" ORDER BY timestamp DESC LIMIT ? OFFSET ?");
        params.push(query.limit.to_string());
        params.push(query.offset.to_string());

        let rows = self.db.query_memos(&sql, &params)?;
        let memos = rows.into_iter().map(|row| self.row_to_memo(row)).collect();

        Ok(memos)
    }

    /// 将数据库行转换为 Memo 对象
    fn row_to_memo(
        &self,
        (id, timestamp, date, content, images, tags, file_path, created_at): (
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            String,
        ),
    ) -> Memo {
        let images: Vec<String> = serde_json::from_str(&images).unwrap_or_default();
        let tags: Vec<String> = serde_json::from_str(&tags).unwrap_or_default();

        Memo {
            id,
            timestamp: chrono::DateTime::parse_from_rfc3339(&timestamp)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            date,
            content,
            images,
            tags,
            file_path,
            created_at: chrono::DateTime::parse_from_rfc3339(&created_at)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
        }
    }

    /// 从 Obsidian 文件同步小记到数据库
    pub async fn sync_from_obsidian(&self, months: u32) -> Result<u32, BrainError> {
        let obsidian = self
            .obsidian
            .as_ref()
            .ok_or_else(|| BrainError::Internal("Obsidian API 不可用".to_string()))?;

        // 计算需要同步的月份列表
        let now = Local::now();
        let mut month_files = Vec::new();
        for i in 0..months {
            let target = now - chrono::Duration::days(30 * i as i64);
            month_files.push(format!("Timeline/{:04}-{:02}.md", target.year(), target.month()));
        }

        let mut total_synced = 0u32;

        for file_path in &month_files {
            let content = match obsidian.read_file(file_path).await {
                Ok(c) => c,
                Err(_) => {
                    tracing::debug!(path = %file_path, "月份文件不存在，跳过");
                    continue;
                }
            };

            let memos = self.parse_month_file(&content, file_path);
            for memo in memos {
                let images_json =
                    serde_json::to_string(&memo.images).unwrap_or_else(|_| "[]".to_string());
                let tags_json =
                    serde_json::to_string(&memo.tags).unwrap_or_else(|_| "[]".to_string());

                self.db.upsert_memo(
                    &memo.id,
                    &memo.timestamp.to_rfc3339(),
                    &memo.date,
                    &memo.content,
                    &images_json,
                    &tags_json,
                    &memo.file_path,
                )?;
                total_synced += 1;
            }
        }

        tracing::info!(
            months = months,
            synced = total_synced,
            "Obsidian 小记同步完成"
        );
        Ok(total_synced)
    }

    /// 解析月份 Markdown 文件，提取小记
    fn parse_month_file(&self, content: &str, file_path: &str) -> Vec<Memo> {
        let mut memos = Vec::new();
        let mut current_date = String::new();
        let mut current_time = String::new();
        let mut current_content = String::new();
        let mut current_images: Vec<String> = Vec::new();
        let mut current_tags: Vec<String> = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();

            // ## YYYY-MM-DD
            if trimmed.starts_with("## ") && trimmed.len() >= 13 {
                let date_part = trimmed[3..].trim();
                if date_part.len() >= 10 && date_part.chars().nth(4) == Some('-') {
                    self.flush_memo(
                        &mut memos,
                        &current_date,
                        &current_time,
                        &current_content,
                        &current_images,
                        &current_tags,
                        file_path,
                    );
                    current_date = date_part[..10].to_string();
                    current_time.clear();
                    current_content.clear();
                    current_images.clear();
                    current_tags.clear();
                    continue;
                }
            }

            // ### HH:MM:SS
            if trimmed.starts_with("### ") && trimmed.len() >= 12 {
                let time_part = trimmed[4..].trim();
                if time_part.len() >= 8 && time_part.chars().nth(2) == Some(':') {
                    self.flush_memo(
                        &mut memos,
                        &current_date,
                        &current_time,
                        &current_content,
                        &current_images,
                        &current_tags,
                        file_path,
                    );
                    current_time = time_part[..8].to_string();
                    current_content.clear();
                    current_images.clear();
                    current_tags.clear();
                    continue;
                }
            }

            // --- separator
            if trimmed == "---" {
                self.flush_memo(
                    &mut memos,
                    &current_date,
                    &current_time,
                    &current_content,
                    &current_images,
                    &current_tags,
                    file_path,
                );
                continue;
            }

            // ![[image.png]]
            if trimmed.starts_with("![") && trimmed.contains("]]") {
                if let Some(start) = trimmed.find("[[") {
                    if let Some(end) = trimmed.find("]]") {
                        let img_path = &trimmed[start + 2..end];
                        current_images.push(img_path.to_string());
                        continue;
                    }
                }
            }

            // #tag
            if trimmed.starts_with('#') && !trimmed.starts_with("# ") {
                for word in trimmed.split_whitespace() {
                    if let Some(tag) = word.strip_prefix('#') {
                        let tag = tag.trim();
                        if !tag.is_empty() {
                            current_tags.push(tag.to_string());
                        }
                    }
                }
                continue;
            }

            // Skip # title line
            if trimmed.starts_with("# ") {
                continue;
            }

            // Regular content
            if !current_time.is_empty() && !trimmed.is_empty() {
                if !current_content.is_empty() {
                    current_content.push('\n');
                }
                current_content.push_str(trimmed);
            }
        }

        // Flush last memo
        self.flush_memo(
            &mut memos,
            &current_date,
            &current_time,
            &current_content,
            &current_images,
            &current_tags,
            file_path,
        );

        memos
    }

    fn flush_memo(
        &self,
        memos: &mut Vec<Memo>,
        date: &str,
        time: &str,
        content: &str,
        images: &[String],
        tags: &[String],
        file_path: &str,
    ) {
        if date.is_empty() || time.is_empty() || content.is_empty() {
            return;
        }

        let id = format!("sync:{}:{}:{}", file_path, date, time);
        let timestamp_str = format!("{}T{}+08:00", date, time);
        let timestamp = chrono::DateTime::parse_from_rfc3339(&timestamp_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        memos.push(Memo {
            id,
            timestamp,
            date: date.to_string(),
            content: content.to_string(),
            images: images.to_vec(),
            tags: tags.to_vec(),
            file_path: file_path.to_string(),
            created_at: timestamp,
        });
    }
}
