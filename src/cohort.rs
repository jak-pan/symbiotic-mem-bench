//! Cohort identity: what makes two runs comparable.
//!
//! A leaderboard is only fair within a *cohort* — runs over the same benchmark,
//! the same size, ideally the same questions and the same judge. We capture
//! fingerprints:
//!
//! - [`dataset_fingerprint`] hashes the exact set of question ids a run covered,
//!   so "same questions" is verifiable rather than assumed.
//! - [`config_signature`] hashes the comparable configuration (distiller,
//!   embedder, store, routing, planner, and the answer/distill/embed models) so
//!   the explorer can group "same params" and spot which knob changed.

use crate::artifacts;
use crate::stable_hash;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

/// The concrete models behind a run's roles, when known.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Models {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub answer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distill: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embed: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub judge: Option<String>,
}

impl Models {
    /// Derive role models from a model-trace role map (`bench.answer` etc.).
    pub fn from_roles(roles: &BTreeMap<String, String>) -> Self {
        let pick = |needle: &str| {
            roles
                .iter()
                .find(|(role, _)| role.to_lowercase().contains(needle))
                .map(|(_, model)| model.clone())
        };
        Self {
            answer: pick("answer"),
            distill: pick("distill"),
            embed: pick("embed"),
            judge: pick("judge"),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.answer.is_none()
            && self.distill.is_none()
            && self.embed.is_none()
            && self.judge.is_none()
    }
}

/// Hash of the sorted question-id set the run covered. `None` when no artifacts
/// expose question ids.
pub fn dataset_fingerprint(run_root: &Path) -> Option<String> {
    let ids = artifacts::question_ids(run_root);
    if ids.is_empty() {
        return None;
    }
    Some(stable_hash(ids.join(",").as_bytes()))
}

/// Stable label/id for a `(benchmark, limit, dataset, judge, prompt mode)` cohort.
pub fn cohort_id(
    benchmark: &str,
    limit: Option<u64>,
    dataset_fingerprint: Option<&str>,
    judge_model: Option<&str>,
    judge_prompt_mode: Option<&str>,
) -> String {
    let seed = format!(
        "{benchmark}|{}|{}|{}|{}",
        limit.map(|value| value.to_string()).unwrap_or_default(),
        dataset_fingerprint.unwrap_or(""),
        judge_model.unwrap_or(""),
        judge_prompt_mode.unwrap_or(""),
    );
    stable_hash(seed.as_bytes())
}

/// Hash of the comparable configuration knobs. Reads the tunable fields from
/// run params plus the resolved role models so that two runs that differ only
/// in, say, the query planner produce different signatures.
pub fn config_signature(params: &Value, models: &Models) -> String {
    let mut parts: BTreeMap<&str, String> = BTreeMap::new();
    for key in [
        "distiller",
        "embedder",
        "store",
        "answerer",
        "routed",
        "answer_only",
        "query_planner",
        "scorer",
    ] {
        if let Some(value) = params.get(key)
            && !value.is_null()
        {
            parts.insert(key, scalar(value));
        }
    }
    if let Some(answer) = &models.answer {
        parts.insert("answer_model", answer.clone());
    }
    if let Some(distill) = &models.distill {
        parts.insert("distill_model", distill.clone());
    }
    if let Some(embed) = &models.embed {
        parts.insert("embed_model", embed.clone());
    }
    let seed = parts
        .into_iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join(";");
    stable_hash(seed.as_bytes())
}

fn scalar(value: &Value) -> String {
    value
        .as_str()
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn dataset_fingerprint_is_order_independent() {
        let dir_a = tempfile::tempdir().unwrap();
        let dir_b = tempfile::tempdir().unwrap();
        let verdicts_a = dir_a.path().join("artifacts").join("verdicts.jsonl");
        let verdicts_b = dir_b.path().join("artifacts").join("verdicts.jsonl");
        std::fs::create_dir_all(verdicts_a.parent().unwrap()).unwrap();
        std::fs::create_dir_all(verdicts_b.parent().unwrap()).unwrap();
        std::fs::write(
            &verdicts_a,
            "{\"question_id\":\"q1\"}\n{\"question_id\":\"q2\"}\n",
        )
        .unwrap();
        std::fs::write(
            &verdicts_b,
            "{\"question_id\":\"q2\"}\n{\"question_id\":\"q1\"}\n",
        )
        .unwrap();

        assert_eq!(
            dataset_fingerprint(dir_a.path()),
            dataset_fingerprint(dir_b.path())
        );
    }

    #[test]
    fn config_signature_changes_with_planner() {
        let models = Models::default();
        let base = json!({"distiller": "heuristic", "query_planner": "off"});
        let changed = json!({"distiller": "heuristic", "query_planner": "scripted"});
        assert_ne!(
            config_signature(&base, &models),
            config_signature(&changed, &models)
        );
    }

    #[test]
    fn models_from_roles_match_substrings() {
        let mut roles = BTreeMap::new();
        roles.insert("bench.judge".to_string(), "deepseek-v4-flash".to_string());
        roles.insert("bench.answer".to_string(), "deepseek-v4-pro".to_string());
        roles.insert(
            "memory.distill".to_string(),
            "deepseek-v4-flash".to_string(),
        );
        let models = Models::from_roles(&roles);
        assert_eq!(models.judge.as_deref(), Some("deepseek-v4-flash"));
        assert_eq!(models.distill.as_deref(), Some("deepseek-v4-flash"));
    }
}
