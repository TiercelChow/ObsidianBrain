//! Bounded identity hints, chosen per source batch; no model facts are inferred.

use super::qa_policy::estimated_tokens;
use crate::infra::book_wiki_store::CompileCatalogEntry;

pub(super) fn identity_row(entry: &CompileCatalogEntry) -> String {
    serde_json::json!({"id":entry.id,"entry_type":entry.entry_type,"slug":entry.slug,
        "title":entry.title.chars().take(240).collect::<String>(),
        "summary":entry.summary.chars().take(320).collect::<String>(),
        "aliases":entry.aliases.iter().take(8).map(|a|a.chars().take(100).collect::<String>()).collect::<Vec<_>>(),
        "status":entry.status,"revision":entry.revision}).to_string()
}

pub(super) fn select_identities(
    source: &str,
    catalog: &[CompileCatalogEntry],
    budget: u64,
) -> Vec<CompileCatalogEntry> {
    let source = source.to_lowercase();
    let mut ranked = catalog
        .iter()
        .filter_map(|entry| {
            let score = std::iter::once((&entry.title, 100))
                .chain(entry.aliases.iter().map(|alias| (alias, 80)))
                .filter(|(label, _)| label.chars().count() >= 2)
                .filter(|(label, _)| source.contains(&label.to_lowercase()))
                .map(|(label, weight)| weight + label.chars().count().min(32))
                .max()
                .unwrap_or(0);
            let slug_score = entry
                .slug
                .split('-')
                .filter(|term| term.len() >= 3 && source.contains(&term.to_lowercase()))
                .count()
                * 4;
            let score = score + slug_score;
            (score > 0).then_some((score, entry))
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|(a, x), (b, y)| b.cmp(a).then_with(|| x.id.cmp(&y.id)));
    let mut used = 0;
    let mut result = vec![];
    for (_, entry) in ranked {
        let tokens = estimated_tokens(&identity_row(entry)) + 8;
        if used + tokens <= budget {
            used += tokens;
            result.push(entry.clone());
        }
    }
    result
}

pub(super) fn candidate_catalog(candidates: &[serde_json::Value]) -> Vec<CompileCatalogEntry> {
    candidates
        .iter()
        .map(|candidate| {
            let text = |key| candidate[key].as_str().unwrap_or("").to_string();
            CompileCatalogEntry {
                id: text("slug"),
                entry_type: text("entry_type"),
                slug: text("slug"),
                title: text("title"),
                summary: text("summary"),
                aliases: candidate["aliases"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|alias| alias.as_str().map(str::to_string))
                    .collect(),
                status: "proposed".into(),
                revision: 0,
            }
        })
        .collect()
}

pub(super) fn validate_identity_types(
    entries: &[serde_json::Value],
    catalog: &[CompileCatalogEntry],
) -> Result<(), crate::error::BrainError> {
    for entry in entries {
        let slug = entry["slug"].as_str().unwrap_or("");
        let kind = entry["entry_type"].as_str().unwrap_or("");
        let existing = catalog
            .iter()
            .filter(|item| item.slug == slug)
            .collect::<Vec<_>>();
        if !existing.is_empty() && !existing.iter().any(|item| item.entry_type == kind) {
            return Err(crate::error::BrainError::KnowledgeValidation(format!("主题 {slug} 已有不同的 entry_type；相同主题应沿用身份，不同对象请使用有区别的稳定 slug")));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::book_wiki_store::CompileCatalogEntry;

    #[test]
    fn test_compile_identity_finds_late_alias_and_keeps_complete_keys() {
        let mut catalog = (0..1100)
            .map(|index| CompileCatalogEntry {
                id: format!("entry-{index}"),
                entry_type: "concept".into(),
                slug: format!("unrelated-{index}"),
                title: format!("无关主题-{index}"),
                summary: "其他内容".into(),
                aliases: vec![],
                status: "verified".into(),
                revision: 1,
            })
            .collect::<Vec<_>>();
        catalog[1088].slug = "paged-attention".into();
        catalog[1088].title = "分页注意力".into();
        catalog[1088].aliases = vec!["PagedAttention".into()];
        let selected = select_identities("## PagedAttention\n公式与操作条件。", &catalog, 2000);
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].id, "entry-1088");
        assert_eq!(selected[0].slug, "paged-attention");
        let row = identity_row(&selected[0]);
        assert!(row.contains("PagedAttention"));
        assert!(serde_json::from_str::<serde_json::Value>(&row).is_ok());
        assert!(select_identities("PagedAttention", &catalog, 1).is_empty());
        assert!(validate_identity_types(
            &[serde_json::json!({"slug":"paged-attention","entry_type":"method"})],
            &catalog
        )
        .is_err());
        assert!(validate_identity_types(
            &[serde_json::json!({"slug":"paged-attention","entry_type":"concept"})],
            &catalog
        )
        .is_ok());
    }
}
