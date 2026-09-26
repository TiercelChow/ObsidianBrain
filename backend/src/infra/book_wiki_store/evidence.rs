//! Immutable, numbered evidence shared by answers, saved conclusions and reports.

use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;

use super::BookWikiStore;
use crate::error::BrainError;
use crate::models::book_wiki::{KnowledgeCitation, KnowledgeEntrySummary};

#[derive(Clone, Debug, Serialize, Deserialize)]
struct VisibleSegment {
    offset_chars: usize,
    content: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CitationSnapshot {
    entry: KnowledgeEntrySummary,
    citations: Vec<KnowledgeCitation>,
    source_span_ids: Vec<String>,
    segments: Vec<VisibleSegment>,
}

#[derive(Clone, Debug, Serialize)]
pub struct EvidenceReadRange {
    pub offset_chars: usize,
    pub returned_chars: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct AgentRunCitation {
    pub run_id: String,
    pub citation_index: usize,
    pub kind: String,
    pub object_id: String,
    pub version_id: String,
    pub entry: KnowledgeEntrySummary,
    pub content_md: String,
    pub citations: Vec<KnowledgeCitation>,
    pub historical: bool,
    pub read_ranges: Vec<EvidenceReadRange>,
    #[serde(skip)]
    source_span_ids: Vec<String>,
}

impl BookWikiStore {
    /// Caller supplies only the text it actually exposes. Validate it against
    /// the scoped version; catalog/search snippets do not call this method.
    pub fn record_visible_agent_evidence(
        &self,
        run_id: &str,
        kind: &str,
        object_id: &str,
        version_id: &str,
        visible: &Value,
    ) -> Result<usize, BrainError> {
        let run = self.get_agent_run(run_id)?;
        if run.status != "running" {
            return Err(BrainError::KnowledgeValidation(
                "只能记录运行中的已读证据".into(),
            ));
        }
        let base_id = run
            .knowledge_base_id
            .as_deref()
            .ok_or_else(|| BrainError::KnowledgeValidation("证据运行缺少知识库".into()))?;
        let content = visible
            .get("content_md")
            .and_then(Value::as_str)
            .ok_or_else(|| BrainError::KnowledgeValidation("证据缺少实际可见正文".into()))?;
        if content.trim().is_empty() {
            return Err(BrainError::KnowledgeValidation(
                "本次未返回有效正文，不能登记为已读证据".into(),
            ));
        }
        let offset = visible
            .get("offset_chars")
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize;
        if kind != "external" && content.chars().count() > 12_000 {
            return Err(BrainError::KnowledgeValidation(
                "单次已读证据超过分页长度".into(),
            ));
        }
        let now = Utc::now().to_rfc3339();
        let (entry, citations, source_span_ids, actual) = match kind {
            "entry" => {
                let detail = self.get_entry(object_id)?;
                if detail.entry.knowledge_base_id != base_id
                    || detail.revision.to_string() != version_id
                {
                    return Err(BrainError::KnowledgeValidation(
                        "证据条目不属于当前书籍或版本不匹配".into(),
                    ));
                }
                let spans = self.db.with_connection(|conn| {
                    let mut stmt = conn.prepare("SELECT DISTINCT source_span_id FROM knowledge_citations WHERE entry_id = ?1 ORDER BY source_span_id")?;
                    let rows = stmt.query_map([object_id], |row| row.get::<_, String>(0))?;
                    Ok(rows.collect::<Result<Vec<_>, _>>()?)
                })?;
                (detail.entry, detail.citations, spans, detail.content_md)
            }
            "source_span" => {
                let span = self.get_current_source_span(base_id, object_id)?;
                if span.source_version_id != version_id {
                    return Err(BrainError::KnowledgeValidation("证据来源版本不匹配".into()));
                }
                let entry = self
                    .source_section_for_span(base_id, object_id)?
                    .ok_or_else(|| {
                        BrainError::KnowledgeValidation("来源片段缺少对应章节索引".into())
                    })?;
                let citations = vec![KnowledgeCitation {
                    id: object_id.into(),
                    source_path: span.source_path.clone(),
                    heading: span.heading,
                    line_start: span.line_start,
                    line_end: span.line_end,
                    quote_text: None,
                }];
                (entry, citations, vec![object_id.into()], span.content)
            }
            "external" => {
                let hash = super::stable_id("external", object_id);
                let entry = KnowledgeEntrySummary {
                    id: hash.clone(),
                    knowledge_base_id: base_id.into(),
                    entry_type: "external".into(),
                    slug: hash,
                    title: object_id.into(),
                    summary: content.chars().take(160).collect(),
                    status: "external".into(),
                    confidence: None,
                    source_path: Some(object_id.into()),
                    updated_at: now.clone(),
                };
                (entry, Vec::new(), Vec::new(), content.into())
            }
            _ => return Err(BrainError::KnowledgeValidation("未知证据类型".into())),
        };
        let expected: String = actual
            .chars()
            .skip(offset)
            .take(content.chars().count())
            .collect();
        if expected != content || (kind == "external" && offset != 0) {
            return Err(BrainError::KnowledgeValidation(
                "证据正文与所读版本或范围不匹配".into(),
            ));
        }
        let index = self.db.transaction(|conn| {
            let running: bool = conn.query_row("SELECT status = 'running' FROM agent_runs WHERE id = ?1", [run_id], |row| row.get(0))?;
            if !running { return Err(BrainError::KnowledgeValidation("Agent 已结束，拒绝追加证据".into())); }
            // Recheck the live revision inside the write transaction. Source
            // versions are immutable; changed current pointers also reject.
            let valid = match kind {
                "entry" => conn.query_row("SELECT EXISTS(SELECT 1 FROM knowledge_entries WHERE id=?1 AND knowledge_base_id=?2 AND CAST(revision AS TEXT)=?3)", params![object_id,base_id,version_id], |row| row.get::<_,bool>(0))?,
                "source_span" => conn.query_row("SELECT EXISTS(SELECT 1 FROM source_spans ss JOIN source_versions sv ON sv.id=ss.source_version_id JOIN source_documents sd ON sd.id=sv.source_document_id WHERE ss.id=?1 AND ss.knowledge_base_id=?2 AND ss.source_version_id=?3 AND sd.current_version_id=sv.id AND sd.sync_status='current')",params![object_id,base_id,version_id], |row| row.get::<_,bool>(0))?,
                _ => true,
            };
            if !valid { return Err(BrainError::KnowledgeValidation("记录证据时来源版本已变化".into())); }
            let old = conn.query_row("SELECT citation_index,snapshot_json FROM agent_run_citations WHERE run_id=?1 AND kind=?2 AND object_id=?3 AND version_id=?4", params![run_id,kind,object_id,version_id], |row| Ok((row.get::<_,usize>(0)?, row.get::<_,String>(1)?))).optional()?;
            let (index, mut snapshot) = if let Some((index, raw)) = old {
                (index, serde_json::from_str::<CitationSnapshot>(&raw).map_err(|e| BrainError::Internal(format!("证据快照解析失败: {e}")))?)
            } else {
                let index = conn.query_row("SELECT COALESCE(MAX(citation_index),0)+1 FROM agent_run_citations WHERE run_id=?1", [run_id], |row| row.get::<_,usize>(0))?;
                (index, CitationSnapshot { entry, citations, source_span_ids, segments: Vec::new() })
            };
            if !snapshot.segments.iter().any(|segment| segment.offset_chars == offset && segment.content == content) {
                snapshot.segments.push(VisibleSegment { offset_chars: offset, content: content.into() });
                snapshot.segments.sort_by_key(|segment| segment.offset_chars);
            }
            let raw = serde_json::to_string(&snapshot).map_err(|e| BrainError::Internal(format!("证据快照序列化失败: {e}")))?;
            if raw.len() > 8 * 1024 * 1024 { return Err(BrainError::KnowledgeValidation("证据快照超过存储上限".into())); }
            conn.execute("INSERT INTO agent_run_citations(run_id,citation_index,kind,object_id,version_id,snapshot_json,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(run_id,citation_index) DO UPDATE SET snapshot_json=excluded.snapshot_json", params![run_id,index,kind,object_id,version_id,raw,now])?;
            let metadata = json!({"citation_index":index,"title":snapshot.entry.title,"source_path":snapshot.entry.source_path,"offset_chars":offset,"returned_chars":content.chars().count(),"origin":visible.get("origin")});
            conn.execute("INSERT INTO agent_run_evidence(run_id,kind,object_id,version_id,snapshot_json,created_at) VALUES(?1,?2,?3,?4,?5,?6)",params![run_id,kind,object_id,version_id,metadata.to_string(),now])?;
            Ok(index)
        })?;
        Ok(index)
    }

    pub fn list_agent_run_citations(
        &self,
        run_id: &str,
    ) -> Result<Vec<AgentRunCitation>, BrainError> {
        self.get_agent_run(run_id)?;
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare("SELECT citation_index,kind,object_id,version_id,snapshot_json FROM agent_run_citations WHERE run_id=?1 ORDER BY citation_index")?;
            let rows = stmt.query_map([run_id], |row| Ok((row.get::<_,usize>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?,row.get::<_,String>(3)?,row.get::<_,String>(4)?)))?;
            rows.map(|row| decode_citation(run_id, row?)).collect()
        })
    }

    /// Source cards do not need to deserialize every evidence body.
    pub fn list_agent_run_citation_entries(
        &self,
        run_id: &str,
    ) -> Result<Vec<KnowledgeEntrySummary>, BrainError> {
        self.get_agent_run(run_id)?;
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare("SELECT json_extract(snapshot_json,'$.entry') FROM agent_run_citations WHERE run_id=?1 ORDER BY citation_index")?;
            let rows = stmt.query_map([run_id], |row| row.get::<_, String>(0))?;
            rows.map(|row| serde_json::from_str(&row?).map_err(|e|BrainError::Internal(format!("证据来源快照解析失败: {e}")))).collect()
        })
    }

    pub fn get_agent_run_citation(
        &self,
        run_id: &str,
        source_index: usize,
    ) -> Result<AgentRunCitation, BrainError> {
        self.get_agent_run(run_id)?;
        let index = source_index
            .checked_add(1)
            .and_then(|value| i64::try_from(value).ok())
            .ok_or_else(|| BrainError::KnowledgeValidation("引用编号超出范围".into()))?;
        let raw = self.db.with_connection(|conn| {
            conn.query_row("SELECT citation_index,kind,object_id,version_id,snapshot_json FROM agent_run_citations WHERE run_id=?1 AND citation_index=?2",params![run_id,index],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?))).optional().map_err(Into::into)
        })?.ok_or_else(|| {
                BrainError::KnowledgeValidation(
                    "历史运行未记录编号证据快照，请在 Wiki 工作台查看当前版本".into(),
                )
            })?;
        decode_citation(run_id, raw)
    }

    /// Read-only validation before proposing saved knowledge. Never substitute
    /// a newer entry revision or source version into an old answer's citations.
    pub fn current_run_citation_span_ids(&self, run_id: &str) -> Result<Vec<String>, BrainError> {
        let run = self.get_agent_run(run_id)?;
        let base_id = run
            .knowledge_base_id
            .as_deref()
            .ok_or_else(|| BrainError::KnowledgeValidation("证据缺少知识库".into()))?;
        let citations = self.list_agent_run_citations(run_id)?;
        let answer = run
            .output
            .as_ref()
            .and_then(|out| out.get("answer"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let has_markers = answer.contains("[S");
        let mut seen = HashSet::new();
        let mut spans = Vec::new();
        for citation in citations {
            if has_markers && !answer.contains(&format!("[S{}]", citation.citation_index)) {
                continue;
            }
            if citation.kind == "entry" {
                let detail = self.get_entry(&citation.object_id)?;
                if detail.entry.knowledge_base_id != base_id
                    || detail.revision.to_string() != citation.version_id
                {
                    return Err(BrainError::KnowledgeValidation(
                        "回答引用的实体版本已变化，请重新核验后保存".into(),
                    ));
                }
            }
            for id in citation.source_span_ids {
                self.get_current_source_span(base_id, &id).map_err(|_| {
                    BrainError::KnowledgeValidation("回答引用的原文已变化，请重新核验后保存".into())
                })?;
                if seen.insert(id.clone()) {
                    spans.push(id);
                }
            }
        }
        Ok(spans)
    }
}

type CitationRow = (usize, String, String, String, String);

fn decode_citation(run_id: &str, row: CitationRow) -> Result<AgentRunCitation, BrainError> {
    let (citation_index, kind, object_id, version_id, raw) = row;
    let snapshot: CitationSnapshot = serde_json::from_str(&raw)
        .map_err(|e| BrainError::Internal(format!("证据快照解析失败: {e}")))?;
    let read_ranges = snapshot
        .segments
        .iter()
        .map(|s| EvidenceReadRange {
            offset_chars: s.offset_chars,
            returned_chars: s.content.chars().count(),
        })
        .collect();
    let mut content_md = String::new();
    let mut end = 0;
    for segment in &snapshot.segments {
        if segment.offset_chars > end {
            content_md.push_str("\n\n> 未读取的区间已省略\n\n");
        }
        let skip = end.saturating_sub(segment.offset_chars);
        content_md.extend(segment.content.chars().skip(skip));
        end = end.max(segment.offset_chars + segment.content.chars().count());
    }
    Ok(AgentRunCitation {
        run_id: run_id.into(),
        citation_index,
        kind,
        object_id,
        version_id,
        entry: snapshot.entry,
        content_md,
        citations: snapshot.citations,
        historical: true,
        read_ranges,
        source_span_ids: snapshot.source_span_ids,
    })
}

#[cfg(test)]
mod tests {
    use crate::infra::book_wiki_store::{BookWikiStore, MarkdownSourceDraft, SourceSectionDraft};
    use crate::models::book_wiki::{BookKind, ReaderBook};
    use serde_json::json;

    fn fixture() -> (BookWikiStore, tempfile::TempDir, String, String) {
        let (ctx, dir, vault) = crate::AppContext::for_test();
        let store = ctx.book_wiki_service.store().clone();
        store
            .save_reader_books(&[ReaderBook {
                id: "citation-book".into(),
                path: vault.to_string_lossy().into_owned(),
                kind: BookKind::Folder,
                name: "引用测试".into(),
                description: String::new(),
                category: String::new(),
                added_at: 1,
                progress: None,
            }])
            .unwrap();
        let base = store.initialize_base("citation-book").unwrap();
        let source = MarkdownSourceDraft {
            id: "citation-source".into(),
            version_id: "citation-version".into(),
            original_path: vault.join("source.md").to_string_lossy().into_owned(),
            relative_path: "source.md".into(),
            title: "来源".into(),
            ordinal: 0,
            size_bytes: 20,
            content_hash: "citation-hash".into(),
            modified_at: None,
            sections: vec![SourceSectionDraft {
                id: "citation-span".into(),
                entry_id: "citation-entry".into(),
                slug: "citation".into(),
                title: "原始标题".into(),
                summary: "原始摘要".into(),
                content_md: "原始正文第一段第二段".into(),
                line_start: 1,
                line_end: 2,
                content_hash: "span-hash".into(),
            }],
        };
        store.sync_markdown_sources(&base.id, &[source]).unwrap();
        let run = store
            .start_agent_run(&base.id, "deepseek_harness", "knowledge_qa", &json!({}))
            .unwrap();
        (store, dir, base.id, run.id)
    }

    #[test]
    fn test_numbered_evidence_is_stable_and_preserves_read_ranges() {
        let (store, _dir, _base, run) = fixture();
        let first = store
            .record_visible_agent_evidence(
                &run,
                "source_span",
                "citation-span",
                "citation-version",
                &json!({"content_md":"原始正文", "offset_chars":0}),
            )
            .unwrap();
        let second = store
            .record_visible_agent_evidence(
                &run,
                "source_span",
                "citation-span",
                "citation-version",
                &json!({"content_md":"第一段", "offset_chars":4}),
            )
            .unwrap();
        assert_eq!(first, 1);
        assert_eq!(second, first);
        let citation = store.get_agent_run_citation(&run, 0).unwrap();
        assert_eq!(citation.citation_index, 1);
        assert_eq!(citation.read_ranges.len(), 2);
        assert!(citation.content_md.contains("第一段"));
        assert!(!citation.content_md.contains("第二段"));
        assert_eq!(store.list_agent_run_citations(&run).unwrap().len(), 1);
    }

    #[test]
    fn test_numbered_entry_snapshot_survives_updates_and_rejects_unread_versions() {
        let (store, _dir, _base, run) = fixture();
        let detail = store.get_entry("citation-entry").unwrap();
        store
            .record_visible_agent_evidence(
                &run,
                "entry",
                "citation-entry",
                &detail.revision.to_string(),
                &json!({"content_md":"原始正文", "offset_chars":0}),
            )
            .unwrap();
        store.db.with_connection(|conn| {
            conn.execute("UPDATE knowledge_entries SET title = '新标题', content_md = '新正文', revision = revision + 1 WHERE id = 'citation-entry'", [])?;
            Ok(())
        }).unwrap();
        let citation = store.get_agent_run_citation(&run, 0).unwrap();
        assert_eq!(citation.entry.title, "原始标题");
        assert_eq!(citation.content_md, "原始正文");
        assert!(store.current_run_citation_span_ids(&run).is_err());
        assert!(store.get_agent_run_citation(&run, 1).is_err());
        assert!(store
            .record_visible_agent_evidence(
                &run,
                "entry",
                "citation-entry",
                "999",
                &json!({"content_md":"新正文"})
            )
            .is_err());
    }

    #[test]
    fn test_numbered_evidence_rejects_fabricated_body_and_late_reads() {
        let (store, _dir, _base, run) = fixture();
        assert!(store
            .record_visible_agent_evidence(
                &run,
                "entry",
                "citation-entry",
                "1",
                &json!({"content_md":"不存在的事实", "offset_chars":0})
            )
            .is_err());
        assert!(store.list_agent_run_citations(&run).unwrap().is_empty());
        store
            .complete_agent_run(&run, &json!({"answer":"完成"}))
            .unwrap();
        assert!(store
            .record_visible_agent_evidence(
                &run,
                "source_span",
                "citation-span",
                "citation-version",
                &json!({"content_md":"原始正文"})
            )
            .is_err());
    }

    #[test]
    fn test_conversation_reads_numbered_snapshot_after_live_entity_changes() {
        let (store, _dir, base, run) = fixture();
        let detail = store.get_entry("citation-entry").unwrap();
        store
            .record_visible_agent_evidence(
                &run,
                "entry",
                "citation-entry",
                &detail.revision.to_string(),
                &json!({"content_md":"原始正文"}),
            )
            .unwrap();
        store
            .complete_agent_run(&run, &json!({"answer":"有依据的回答。[S1]"}))
            .unwrap();
        let conversation = store
            .save_conversation_exchange(
                &base,
                None,
                "问题",
                "有依据的回答。[S1]",
                &run,
                &[detail.entry],
            )
            .unwrap();
        store.db.with_connection(|conn| {
            conn.execute("UPDATE knowledge_entries SET title='新名称', summary='新概述', content_md='新正文', revision=revision+1 WHERE id='citation-entry'", [])?;
            Ok(())
        }).unwrap();
        let history = store.get_conversation(&conversation).unwrap();
        assert_eq!(history.messages[1].evidence[0].title, "原始标题");
        assert_eq!(
            store.get_agent_run_citation(&run, 0).unwrap().content_md,
            "原始正文"
        );
        assert!(store.current_run_citation_span_ids(&run).is_err());
    }

    #[test]
    fn test_numbered_citation_does_not_cross_books_or_claim_empty_reads() {
        let (store, _dir, _base, run) = fixture();
        assert!(store
            .record_visible_agent_evidence(
                &run,
                "entry",
                "citation-entry",
                "1",
                &json!({"content_md":""})
            )
            .is_err());
        let mut books = store.list_reader_books().unwrap();
        let mut second = books[0].clone();
        second.id = "other-citation-book".into();
        second.path.push_str("-other");
        books.push(second);
        store.save_reader_books(&books).unwrap();
        let other_base = store.initialize_base("other-citation-book").unwrap();
        let other_run = store
            .start_agent_run(
                &other_base.id,
                "deepseek_harness",
                "knowledge_qa",
                &json!({}),
            )
            .unwrap();
        assert!(store
            .record_visible_agent_evidence(
                &other_run.id,
                "entry",
                "citation-entry",
                "1",
                &json!({"content_md":"原始正文"})
            )
            .is_err());
        assert!(store
            .list_agent_run_citation_entries(&other_run.id)
            .unwrap()
            .is_empty());
        assert!(store.get_agent_run_citation(&run, usize::MAX).is_err());
    }
}
