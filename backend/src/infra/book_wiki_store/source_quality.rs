//! Bounded, hit-centered source candidates and exact Unicode source pages.

use super::{knowledge_query_candidates, source_locator_from_row, BookWikiStore};
use crate::error::BrainError;
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};

impl BookWikiStore {
    pub fn read_source_span_page(
        &self,
        base_id: &str,
        span_id: &str,
        offset: usize,
        max_chars: usize,
    ) -> Result<Value, BrainError> {
        self.get_base(base_id)?;
        let offset = offset.min(i64::MAX as usize) as i64;
        let length = max_chars.clamp(1, 4000) as i64;
        self.db.with_connection(|conn|{
            conn.query_row("SELECT ss.id,sd.id,ss.source_version_id,sd.relative_path,ss.heading,
                ss.line_start,ss.line_end,substr(ss.content,min(?3,length(ss.content))+1,?4),length(ss.content),
                COALESCE((SELECT locator_json FROM source_span_structures WHERE source_span_id=ss.id),'{}'),
                min(?3,length(ss.content)),
                length(substr(ss.content,1,min(?3,length(ss.content))))-length(replace(substr(ss.content,1,min(?3,length(ss.content))),char(10),''))
                FROM source_spans ss JOIN source_versions sv ON sv.id=ss.source_version_id
                JOIN source_documents sd ON sd.id=sv.source_document_id
                WHERE ss.id=?2 AND ss.knowledge_base_id=?1 AND sd.knowledge_base_id=?1
                    AND sd.sync_status='current' AND sd.current_version_id=ss.source_version_id",
                params![base_id,span_id,offset,length],|row|{
                    let content:String=row.get(7)?;
                    let total:i64=row.get(8)?;
                    let offset:i64=row.get(10)?;
                    let line_start:Option<i64>=row.get(5)?;
                    let read_line_start=line_start.map(|line|row.get::<_,i64>(11).map(|newlines|line+newlines)).transpose()?;
                    let newlines=content.bytes().filter(|ch|*ch==b'\n').count() as i64;
                    let read_line_end=read_line_start.map(|line|(line+newlines-i64::from(content.ends_with('\n'))).max(line));
                    Ok(json!({"span":{
                        "id":row.get::<_,String>(0)?,"source_document_id":row.get::<_,String>(1)?,"source_version_id":row.get::<_,String>(2)?,
                        "source_path":row.get::<_,String>(3)?,"heading":row.get::<_,Option<String>>(4)?,
                        "line_start":line_start,"line_end":row.get::<_,Option<i64>>(6)?,"read_line_start":read_line_start,"read_line_end":read_line_end,
                        "content":content,"locator":source_locator_from_row(row,9)?,"offset_chars":offset,"total_chars":total,"has_more":offset.saturating_add(length)<total,
                    }}))
                }).optional().map_err(Into::into)
        })?.ok_or_else(||BrainError::KnowledgeNotFound(span_id.into()))
    }

    pub fn search_source_span_previews(
        &self,
        base_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<Value>, BrainError> {
        // Rank using the existing FTS/CJK index, but never load every candidate
        // chapter's full body into memory merely to display a short preview.
        let headers = self.search_source_spans_inner(base_id, query, limit, false)?;
        let mut terms = knowledge_query_candidates(Some(query));
        terms.extend(
            query
                .split(|ch: char| !ch.is_alphanumeric() && ch != '_')
                .filter(|part| part.chars().count() >= 2)
                .take(24)
                .map(str::to_string),
        );
        terms.sort_by_key(|term| std::cmp::Reverse(term.chars().count()));
        let mut seen = std::collections::HashSet::new();
        terms.retain(|term| seen.insert(term.clone()));
        terms.truncate(32);
        let terms = serde_json::to_string(&terms)
            .map_err(|error| BrainError::Internal(format!("检索词序列化失败:{error}")))?;
        let mut result = Vec::with_capacity(headers.len());
        for span in headers {
            let preview=self.db.with_connection(|conn|{
                conn.query_row("WITH candidate AS (
                    SELECT ss.content,ss.line_start,
                        COALESCE((SELECT instr(lower(ss.content),lower(value)) FROM json_each(?3)
                            WHERE instr(lower(ss.content),lower(value))>0 ORDER BY length(value) DESC LIMIT 1),1) AS hit
                    FROM source_spans ss JOIN source_versions sv ON sv.id=ss.source_version_id
                    JOIN source_documents sd ON sd.id=sv.source_document_id
                    WHERE ss.id=?2 AND ss.knowledge_base_id=?1 AND sd.knowledge_base_id=?1
                        AND sd.sync_status='current' AND sd.current_version_id=ss.source_version_id
                ), positioned AS (SELECT content,line_start,max(0,hit-65) AS offset FROM candidate)
                SELECT substr(content,offset+1,240),offset,length(content),
                    line_start+length(substr(content,1,offset))-length(replace(substr(content,1,offset),char(10),''))
                FROM positioned",params![base_id,span.id,terms],|row|{
                    Ok((row.get::<_,String>(0)?,row.get::<_,i64>(1)?,row.get::<_,i64>(2)?,row.get::<_,Option<i64>>(3)?))
                }).optional().map_err(Into::into)
            })?;
            if let Some((preview, offset, total, line)) = preview {
                result.push(json!({"id":span.id,"source_version_id":span.source_version_id,"source_path":span.source_path,"heading":span.heading,
                    "line_start":span.line_start,"line_end":span.line_end,"locator":span.locator,
                    "preview":preview,"preview_offset_chars":offset,"preview_line_start":line,"total_chars":total,"candidate_only":true}));
            }
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use serde_json::json;

    fn fixture() -> (BookWikiStore, tempfile::TempDir, String, String) {
        let dir = tempfile::tempdir().unwrap();
        let store = BookWikiStore::new(Arc::new(
            SqliteStore::new(&dir.path().join("source-quality.db")).unwrap(),
        ));
        let book:ReaderBook=serde_json::from_value(json!({"id":"quality-book","name":"测试书","path":"/tmp/quality-book","kind":"folder","description":"","category":"","addedAt":1,"progress":null})).unwrap();
        store.save_reader_books(&[book]).unwrap();
        let base = store.initialize_base("quality-book").unwrap();
        let content = format!(
            "# 机制\n{}\n中文命中 needle 位于尾部，公式 $x_i$。\n尾行",
            "前文🙂\n".repeat(4000)
        );
        let source = MarkdownSourceDraft {
            id: "quality-source".into(),
            version_id: "quality-version".into(),
            original_path: "/tmp/quality-book/chapter.md".into(),
            relative_path: "chapter.md".into(),
            title: "机制".into(),
            ordinal: 0,
            content_hash: "quality-content".into(),
            size_bytes: content.len() as i64,
            modified_at: None,
            sections: vec![SourceSectionDraft {
                id: "quality-span".into(),
                entry_id: "quality-section".into(),
                slug: "quality-section".into(),
                title: "机制".into(),
                summary: "起始概述".into(),
                content_md: content.clone(),
                line_start: 10,
                line_end: 4014,
                content_hash: "quality-span-hash".into(),
            }],
        };
        let locators = HashMap::from([(
            "quality-span".into(),
            json!({"heading_path":["机制"],"previous_span_id":null,"next_span_id":null}),
        )]);
        store
            .sync_markdown_sources_with_locators(&base.id, &[source], &locators)
            .unwrap();
        (store, dir, base.id, content)
    }

    #[test]
    fn test_source_preview_is_near_tail_match_with_exact_offset_and_bounded_payload() {
        let (store, _dir, base, content) = fixture();
        let previews = store
            .search_source_span_previews(&base, "needle", 5)
            .unwrap();
        assert_eq!(previews.len(), 1);
        let preview = &previews[0];
        let text = preview["preview"].as_str().unwrap();
        assert!(text.contains("needle"));
        assert!(text.chars().count() <= 240);
        let offset = preview["preview_offset_chars"].as_u64().unwrap() as usize;
        assert!(offset > 15000);
        assert_eq!(
            text,
            content.chars().skip(offset).take(240).collect::<String>()
        );
        assert!(preview.get("citation").is_none());
        assert_eq!(preview["locator"]["heading_path"], json!(["机制"]));
    }

    #[test]
    fn test_source_page_preserves_unicode_and_reports_actual_read_line_location() {
        let (store, _dir, base, content) = fixture();
        let page = store
            .read_source_span_page(&base, "quality-span", 9, 8)
            .unwrap();
        assert_eq!(
            page["span"]["content"],
            content.chars().skip(9).take(8).collect::<String>()
        );
        assert_eq!(page["span"]["offset_chars"], 9);
        let prefix = content.chars().take(9).collect::<String>();
        assert_eq!(
            page["span"]["read_line_start"],
            10 + prefix.matches('\n').count()
        );
        assert_eq!(page["span"]["total_chars"], content.chars().count());
        assert_eq!(page["span"]["has_more"], true);
        let empty = store
            .read_source_span_page(&base, "quality-span", usize::MAX, 8)
            .unwrap();
        assert_eq!(empty["span"]["content"], "");
        assert_eq!(empty["span"]["offset_chars"], content.chars().count());
        assert_eq!(empty["span"]["has_more"], false);
        assert!(store
            .read_source_span_page("foreign-book", "quality-span", 0, 8)
            .is_err());
    }
}
