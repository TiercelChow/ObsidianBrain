use std::collections::HashSet;

use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use rusqlite::{params, Connection, OptionalExtension};

use super::BookWikiStore;
use crate::error::BrainError;
use crate::models::wiki_topics::{
    TopicBookBuild, TopicWikiBuildEvent, TopicWikiIndex, TopicWikiIndexPage, TopicWikiIssue,
    TopicWikiPublication,
};

impl BookWikiStore {
    pub(crate) fn topic_index_pages(
        &self,
        base_id: &str,
    ) -> Result<Vec<TopicWikiIndexPage>, BrainError> {
        self.db
            .with_connection(|conn| read_topic_index_pages(conn, base_id))
    }

    pub fn topic_wiki_index(&self, base_id: &str) -> Result<TopicWikiIndex, BrainError> {
        self.get_base(base_id)?;
        self.db
            .with_connection(|conn| read_topic_wiki_index(conn, base_id))
    }
}

fn read_topic_index_pages(
    conn: &Connection,
    base_id: &str,
) -> Result<Vec<TopicWikiIndexPage>, BrainError> {
    let mut query = conn.prepare(
        "WITH plan AS (
            SELECT p.key AS ordinal, json_extract(p.value,'$.page_id') AS page_id,
                   json_extract(p.value,'$.revision_id') AS revision_id,
                   json_extract(p.value,'$.focus') AS scope
            FROM wiki_topic_book_builds b, json_each(b.state_json,'$.pages') p
            WHERE b.knowledge_base_id=?1
         )
         SELECT page.id, page.title, CASE WHEN plan.revision_id=revision.id THEN plan.scope END, revision.markdown,
                EXISTS(SELECT 1 FROM wiki_topic_stale_revisions stale WHERE stale.revision_id=revision.id)
         FROM wiki_topic_pages page
         JOIN wiki_topic_revisions revision ON revision.id=page.published_id
         LEFT JOIN plan ON plan.page_id=page.id
         WHERE page.knowledge_base_id=?1
         ORDER BY plan.ordinal IS NULL, plan.ordinal, page.title, page.id",
    )?;
    let rows = query.query_map([base_id], |row| {
        Ok(TopicWikiIndexPage {
            page_id: row.get(0)?,
            title: row.get(1)?,
            scope: row.get(2)?,
            summary: opening_paragraph(&row.get::<_, String>(3)?),
            needs_review: row.get(4)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub(crate) fn read_topic_wiki_index(
    conn: &Connection,
    base_id: &str,
) -> Result<TopicWikiIndex, BrainError> {
    let pages = read_topic_index_pages(conn, base_id)?;
    let overview = conn
        .query_row(
            "SELECT json_extract(b.state_json,'$.overview')
         FROM wiki_topic_book_builds b WHERE b.knowledge_base_id=?1
           AND b.status='ready' AND length(trim(json_extract(b.state_json,'$.overview'))) > 0
           AND NOT EXISTS (
               SELECT 1 FROM json_each(b.state_json,'$.pages') plan
               LEFT JOIN wiki_topic_pages p ON p.id=json_extract(plan.value,'$.page_id')
                   AND p.knowledge_base_id=b.knowledge_base_id
               WHERE p.published_id IS NULL OR p.published_id IS NOT json_extract(plan.value,'$.revision_id')
           )",
            [base_id],
            |row| row.get(0),
        )
        .optional()?;
    let issues = structural_issues(conn, base_id, &pages)?;
    let mut builds = conn.prepare(
        "SELECT attempt_id,status,source_count,linked_sources,written_pages,planned_pages,recorded_at,maintenance_json
         FROM wiki_topic_build_log WHERE knowledge_base_id=?1 ORDER BY recorded_at DESC,attempt_id LIMIT 24",
    )?;
    let builds = builds
        .query_map([base_id], |row| {
            Ok(TopicWikiBuildEvent {
                attempt_id: row.get(0)?,
                status: row.get(1)?,
                source_count: row.get(2)?,
                linked_sources: row.get(3)?,
                written_pages: row.get(4)?,
                planned_pages: row.get(5)?,
                recorded_at: row.get(6)?,
                maintenance: row
                    .get::<_, Option<String>>(7)?
                    .as_deref()
                    .map(serde_json::from_str)
                    .transpose()
                    .map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            7,
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut publications = conn.prepare(
        "SELECT p.id,p.title,r.id,r.published_at FROM wiki_topic_revisions r
         JOIN wiki_topic_pages p ON p.id=r.page_id
         WHERE p.knowledge_base_id=?1 AND r.published_at IS NOT NULL
         ORDER BY r.published_at DESC, r.id LIMIT 48",
    )?;
    let publications = publications
        .query_map([base_id], |row| {
            Ok(TopicWikiPublication {
                page_id: row.get(0)?,
                title: row.get(1)?,
                revision_id: row.get(2)?,
                published_at: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(TopicWikiIndex {
        overview,
        pages,
        issues,
        builds,
        publications,
    })
}

pub(super) fn record_topic_book_build(
    conn: &Connection,
    base_id: &str,
    build: &TopicBookBuild,
    at: &str,
) -> Result<(), BrainError> {
    let maintenance = build
        .maintenance
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|error| BrainError::Internal(format!("Wiki 维护记录保存失败: {error}")))?;
    let ready = build
        .pages
        .iter()
        .filter(|page| page.revision_id.is_some())
        .count();
    let written = if build.maintenance.is_none() && build.error.is_some() {
        0
    } else {
        ready.saturating_sub(
            build
                .maintenance
                .as_ref()
                .map_or(0, |result| result.reused_pages),
        )
    };
    conn.execute(
        "INSERT INTO wiki_topic_build_log(attempt_id,knowledge_base_id,status,source_count,linked_sources,
                                          written_pages,planned_pages,recorded_at,maintenance_json)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9) ON CONFLICT(attempt_id) DO NOTHING",
        params![build.id,base_id,build.status,build.sources.len(),
                build.sources.iter().filter(|source| source.status == "linked").count(),
                written,build.pages.len(),at,maintenance],
    )?;
    Ok(())
}

fn structural_issues(
    conn: &Connection,
    base_id: &str,
    pages: &[TopicWikiIndexPage],
) -> Result<Vec<TopicWikiIssue>, BrainError> {
    let ids = pages
        .iter()
        .map(|page| page.page_id.as_str())
        .collect::<HashSet<_>>();
    let mut connected_pages = HashSet::new();
    let mut issues = Vec::new();
    let mut query = conn.prepare(
        "SELECT page.id,json_extract(rel.value,'$.page_id'),json_extract(rel.value,'$.title')
         FROM wiki_topic_pages page JOIN wiki_topic_revisions revision ON revision.id=page.published_id
         CROSS JOIN json_each(revision.links_json) AS rel WHERE page.knowledge_base_id=?1",
    )?;
    let mut rows = query.query([base_id])?;
    while let Some(row) = rows.next()? {
        let page_id: String = row.get(0)?;
        let target: String = row.get(1)?;
        if ids.contains(target.as_str()) {
            connected_pages.insert(page_id);
            connected_pages.insert(target);
        } else {
            let title: String = row.get(2)?;
            issues.push(TopicWikiIssue {
                page_id,
                code: "unpublished_link",
                detail: format!("关联页「{title}」待发布"),
            });
        }
    }
    for page in pages {
        if page.needs_review {
            issues.push(TopicWikiIssue {
                page_id: page.page_id.clone(),
                code: "stale_sources",
                detail: "原文版本已变化，需要复核".into(),
            });
        }
        if pages.len() > 1 && !connected_pages.contains(&page.page_id) {
            issues.push(TopicWikiIssue {
                page_id: page.page_id.clone(),
                code: "isolated_page",
                detail: "本页与其他已发布主题之间没有阅读链接".into(),
            });
        }
    }
    Ok(issues)
}

fn opening_paragraph(markdown: &str) -> String {
    let mut in_paragraph = false;
    let mut prose = String::new();
    for event in Parser::new(markdown) {
        match event {
            Event::Start(Tag::Paragraph) => in_paragraph = true,
            Event::Text(value) | Event::Code(value) if in_paragraph => prose.push_str(&value),
            Event::SoftBreak | Event::HardBreak if in_paragraph => prose.push(' '),
            Event::End(TagEnd::Paragraph) if !prose.trim().is_empty() => break,
            Event::End(TagEnd::Paragraph) => in_paragraph = false,
            _ => {}
        }
    }
    let mut summary = String::new();
    let mut rest = prose.as_str();
    while let Some((head, suffix)) = rest.split_once("[S") {
        summary.push_str(head);
        if let Some((_, after)) = suffix
            .split_once(']')
            .filter(|(n, _)| !n.is_empty() && n.bytes().all(|ch| ch.is_ascii_digit()))
        {
            rest = after;
        } else {
            summary.push_str("[S");
            rest = suffix;
        }
    }
    summary.push_str(rest);
    summary.trim().chars().take(200).collect()
}
