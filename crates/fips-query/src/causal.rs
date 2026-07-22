use fips_artifact::LedgerEntry;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CausalStage {
    pub stage: String,
    pub count: u64,
    pub entries: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CausalSummary {
    pub stages: Vec<CausalStage>,
    pub roots: Vec<String>,
    pub critical_path: Vec<String>,
    pub source_entries: usize,
}

pub fn summarize_causal(entries: &[LedgerEntry]) -> CausalSummary {
    let mut stages = BTreeMap::<String, (u64, usize)>::new();
    let mut children = BTreeMap::<String, Vec<String>>::new();
    let ids = entries
        .iter()
        .map(|entry| entry.causal_id.clone())
        .collect::<BTreeSet<_>>();
    for entry in entries {
        let stage = stages.entry(entry.stage.clone()).or_default();
        stage.0 = stage.0.saturating_add(entry.count);
        stage.1 += 1;
        if let Some(parent) = &entry.causal_parent {
            children
                .entry(parent.clone())
                .or_default()
                .push(entry.causal_id.clone());
        }
    }
    for values in children.values_mut() {
        values.sort();
    }
    let mut roots = entries
        .iter()
        .filter(|entry| {
            entry
                .causal_parent
                .as_ref()
                .is_none_or(|parent| !ids.contains(parent))
        })
        .map(|entry| entry.causal_id.clone())
        .collect::<Vec<_>>();
    roots.sort();
    roots.dedup();
    let critical_path = roots
        .iter()
        .map(|root| longest_path(root, &children))
        .max_by_key(Vec::len)
        .unwrap_or_default();
    CausalSummary {
        stages: stages
            .into_iter()
            .map(|(stage, (count, entries))| CausalStage {
                stage,
                count,
                entries,
            })
            .collect(),
        roots,
        critical_path,
        source_entries: entries.len(),
    }
}

fn longest_path(root: &str, children: &BTreeMap<String, Vec<String>>) -> Vec<String> {
    let mut path = vec![root.to_owned()];
    let mut cursor = root;
    let mut seen = BTreeSet::from([root.to_owned()]);
    while let Some(next) = children.get(cursor).and_then(|items| items.first()) {
        if !seen.insert(next.clone()) {
            break;
        }
        path.push(next.clone());
        cursor = next;
    }
    path
}
