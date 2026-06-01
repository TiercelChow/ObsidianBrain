//! 概念选择器

use rand::Rng;
use std::collections::HashMap;

use crate::models::inspiration::{Concept, ConceptPool, InspirationConfig};

/// 概念选择器
pub struct ConceptSelector {
    config: InspirationConfig,
}

impl ConceptSelector {
    pub fn new(config: InspirationConfig) -> Self {
        Self { config }
    }

    /// 从概念池中选择两个距离较远的概念
    pub fn select_pair(
        &self,
        pool: &ConceptPool,
        recent_pairs: &[(String, String)],
    ) -> Option<(usize, usize)> {
        if pool.concepts.len() < 2 {
            return None;
        }

        let mut rng = rand::rng();

        // 第一个概念：按权重加权随机选择
        let weights: Vec<f64> = pool.concepts.iter().map(|c| c.weight).collect();
        let idx_a = weighted_random(&weights, &mut rng)?;

        // 第二个概念：过滤候选，按距离加权选择
        let concept_a = &pool.concepts[idx_a];
        let mut candidates: Vec<(usize, f64)> = Vec::new();

        for (i, concept_b) in pool.concepts.iter().enumerate() {
            if i == idx_a {
                continue;
            }

            // 检查是否在最近的配对中
            let pair_key = if concept_a.term < concept_b.term {
                (concept_a.term.clone(), concept_b.term.clone())
            } else {
                (concept_b.term.clone(), concept_a.term.clone())
            };
            if recent_pairs.contains(&pair_key) {
                continue;
            }

            // 计算距离
            let distance = crate::core::inspiration::concept_pool::ConceptPoolBuilder::compute_distance(concept_a, concept_b);
            if distance >= self.config.min_distance && distance <= self.config.max_distance {
                let weight = distance.powf(2.0); // 距离^2 加权
                candidates.push((i, weight));
            }
        }

        // 如果没有候选，放宽条件
        if candidates.is_empty() {
            for (i, concept_b) in pool.concepts.iter().enumerate() {
                if i == idx_a {
                    continue;
                }
                let distance = crate::core::inspiration::concept_pool::ConceptPoolBuilder::compute_distance(concept_a, concept_b);
                if distance > 0.1 { // 排除几乎相同的
                    candidates.push((i, distance));
                }
            }
        }

        if candidates.is_empty() {
            return None;
        }

        let candidate_weights: Vec<f64> = candidates.iter().map(|(_, w)| *w).collect();
        let selected = weighted_random(&candidate_weights, &mut rng)?;
        let idx_b = candidates[selected].0;

        Some((idx_a, idx_b))
    }
}

/// 加权随机选择
fn weighted_random(weights: &[f64], rng: &mut impl Rng) -> Option<usize> {
    let total: f64 = weights.iter().sum();
    if total <= 0.0 {
        return None;
    }

    let mut rand_val = rng.random_range(0.0..total);
    for (i, &w) in weights.iter().enumerate() {
        rand_val -= w;
        if rand_val <= 0.0 {
            return Some(i);
        }
    }
    weights.len().checked_sub(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::inspiration::{Concept, ConceptPool, ConceptSource};
    use chrono::Utc;

    fn create_test_pool() -> ConceptPool {
        ConceptPool {
            concepts: vec![
                Concept {
                    term: "rust".to_string(),
                    weight: 2.0,
                    source: ConceptSource::NoteTag,
                    note_paths: vec!["note1.md".to_string(), "note2.md".to_string()],
                },
                Concept {
                    term: "python".to_string(),
                    weight: 1.5,
                    source: ConceptSource::NoteTag,
                    note_paths: vec!["note3.md".to_string()],
                },
                Concept {
                    term: "ai".to_string(),
                    weight: 1.0,
                    source: ConceptSource::NoteKeyword,
                    note_paths: vec!["note1.md".to_string(), "note4.md".to_string()],
                },
            ],
            built_at: Utc::now(),
        }
    }

    #[test]
    fn test_select_pair_returns_indices() {
        let config = InspirationConfig::default();
        let selector = ConceptSelector::new(config);
        let pool = create_test_pool();
        let recent = vec![];

        let result = selector.select_pair(&pool, &recent);
        assert!(result.is_some());
        let (a, b) = result.unwrap();
        assert!(a < pool.concepts.len());
        assert!(b < pool.concepts.len());
        assert_ne!(a, b);
    }

    #[test]
    fn test_select_pair_excludes_recent() {
        let config = InspirationConfig::default();
        let selector = ConceptSelector::new(config);
        let pool = create_test_pool();
        let recent = vec![("rust".to_string(), "ai".to_string())];

        let result = selector.select_pair(&pool, &recent);
        assert!(result.is_some());
        let (a, b) = result.unwrap();
        let pair = if pool.concepts[a].term < pool.concepts[b].term {
            (pool.concepts[a].term.clone(), pool.concepts[b].term.clone())
        } else {
            (pool.concepts[b].term.clone(), pool.concepts[a].term.clone())
        };
        assert_ne!(pair, ("rust".to_string(), "ai".to_string()));
    }

    #[test]
    fn test_select_pair_empty_pool() {
        let config = InspirationConfig::default();
        let selector = ConceptSelector::new(config);
        let pool = ConceptPool { concepts: vec![], built_at: Utc::now() };
        let recent = vec![];

        assert!(selector.select_pair(&pool, &recent).is_none());
    }
}
