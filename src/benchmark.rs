//! Benchmark plug-in surface.
//!
//! A benchmark supplies three things that actually differ per benchmark — a **loader** (parses its
//! dataset into the shared haystack-QA record), its **identity** (dispatch id, manifest tag, default
//! dataset), and its **grade targets** (per-question gold answer + optional grader spec). Everything
//! else — ingest, recall, the run-flow, gold-coverage — is benchmark-agnostic over the shared record
//! (`LongMemEvalRecord`: a question + a haystack of sessions-of-turns + gold).
//!
//! The record shape generalizes cleanly: a LongMemEval-v1 chat session and a LongMemEval-v2
//! web/enterprise *trajectory* are both "a session"; a v1 message and a v2 *state* (a11y tree +
//! thought + action) are both "a turn". Multimodal screenshots are deferred to the `Content` spine
//! (see `docs/redesign/06-v2-multimodal-recall-experiment.md`); this surface covers the
//! text-projection lanes that v1, v2 and (next) LoCoMo share.
//!
//! Grading lives partly here (the deterministic v2 `eval_function` DSL — pure + unit-tested) and
//! partly in the `membench` bin (LLM judge execution), because the judge machinery + provider wiring
//! live with the score loop there. `grade_v2` returns whether a deterministic verdict is available or
//! a judge is required.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde::Deserialize;
use serde_json::Value;

use crate::symbiotic_memory_adapter::{
    LongMemEvalMessage, LongMemEvalRecord, SharedCorpusQuestion, load_longmemeval,
};

/// Default dataset location when `--dataset` is omitted.
pub enum DatasetSource {
    /// A path (file for v1, directory for v2) under the repo, plus an optional single-file HF URL to
    /// auto-download to it when absent.
    Path {
        rel_path: &'static str,
        download_url: Option<&'static str>,
    },
}

/// How a benchmark's haystack relates to its questions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HaystackScope {
    /// Each question carries its own haystack, ingested into its own vault (LongMemEval-v1, LoCoMo).
    PerQuestion,
    /// A corpus (keyed per question by `corpus_key`) is ingested ONCE and every question with that key
    /// is answered against the shared store (LongMemEval-v2 by `domain`; document-memory tests).
    SharedCorpus,
}

/// Per-question grading target, produced by a benchmark from its dataset. `gold_answer` is the
/// reference; `eval_function` is v2's grader DSL (`None` for v1, which grades via the model judge
/// routed on `question_type`).
#[derive(Clone, Debug)]
pub struct GradeTarget {
    pub question_type: Option<String>,
    pub gold_answer: Option<Value>,
    pub eval_function: Option<String>,
}

/// The plug-in contract. One implementation per benchmark; selected by [`loader_for`].
pub trait BenchmarkLoader: Send + Sync {
    /// Dispatch id (`--benchmark`, reports, run-params).
    fn id(&self) -> &'static str;
    /// Tag stamped into each vault's `MemoryRunManifest` (identity for the answer-only staleness gate).
    fn manifest_tag(&self) -> &'static str;
    /// Default dataset when `--dataset` is not supplied.
    fn default_dataset(&self) -> DatasetSource;
    /// Parse the dataset into the shared record shape (question + haystack + gold). Used by the
    /// per-question run mode; `SharedCorpus` benchmarks use [`shared_questions`]+[`corpus_record`].
    fn load(&self, path: &Path, limit: Option<usize>) -> anyhow::Result<Vec<LongMemEvalRecord>>;
    /// Per-question grade targets keyed by `question_id` (gold answer + optional grader spec).
    fn grade_targets(&self, path: &Path) -> anyhow::Result<HashMap<String, GradeTarget>>;

    /// Haystack topology. Default `PerQuestion` (the historical behavior).
    fn haystack_scope(&self) -> HaystackScope {
        HaystackScope::PerQuestion
    }
    /// `SharedCorpus` only: the questions (lightweight; no haystack), each tagged with its `corpus_key`.
    fn shared_questions(
        &self,
        _path: &Path,
        _limit: Option<usize>,
    ) -> anyhow::Result<Vec<SharedCorpusQuestion>> {
        anyhow::bail!("benchmark does not support shared-corpus mode")
    }
    /// `SharedCorpus` only: build the corpus for `corpus_key` as a haystack record (the run converts it
    /// to a `SourceDocument` and ingests it once). `question`/`answer` fields are empty.
    fn corpus_record(&self, _path: &Path, _corpus_key: &str) -> anyhow::Result<LongMemEvalRecord> {
        anyhow::bail!("benchmark does not support shared-corpus mode")
    }
}

/// Registry: resolve a benchmark id to its loader. New benchmarks register here.
pub fn loader_for(id: &str) -> Option<Box<dyn BenchmarkLoader>> {
    match id {
        "long-mem-eval" => Some(Box::new(LongMemEvalV1)),
        "longmemeval-v2" => Some(Box::new(LongMemEvalV2)),
        _ => None,
    }
}

// ============================================================================
// LongMemEval v1 — the existing benchmark, now behind the trait (behavior-identical).
// ============================================================================

pub struct LongMemEvalV1;

impl BenchmarkLoader for LongMemEvalV1 {
    fn id(&self) -> &'static str {
        "long-mem-eval"
    }
    fn manifest_tag(&self) -> &'static str {
        "longmemeval-v1"
    }
    fn default_dataset(&self) -> DatasetSource {
        DatasetSource::Path {
            rel_path: "runs/inputs/longmemeval-cleaned/longmemeval_s_cleaned.json",
            download_url: Some(
                "https://huggingface.co/datasets/xiaowu0162/longmemeval-cleaned/resolve/main/longmemeval_s_cleaned.json",
            ),
        }
    }
    fn load(&self, path: &Path, limit: Option<usize>) -> anyhow::Result<Vec<LongMemEvalRecord>> {
        load_longmemeval(path, limit)
    }
    fn grade_targets(&self, path: &Path) -> anyhow::Result<HashMap<String, GradeTarget>> {
        let rows = load_longmemeval(path, None)?;
        Ok(rows
            .into_iter()
            .map(|r| {
                (
                    r.question_id.clone(),
                    GradeTarget {
                        question_type: r.question_type.clone(),
                        gold_answer: r.answer.clone(),
                        eval_function: None,
                    },
                )
            })
            .collect())
    }
}

// ============================================================================
// LongMemEval v2 — web/enterprise agent trajectories + screenshots.
// Dataset dir: questions.jsonl, trajectories.jsonl, haystacks/lme_v2_{small,medium}.json.
// The text-projection lane maps each trajectory -> a "session" and each state -> a "turn"
// (a11y tree + thought + action). Screenshots are ignored here (native lane is future work).
// ============================================================================

pub struct LongMemEvalV2;

#[derive(Debug, Deserialize)]
#[allow(dead_code)] // domain/environment/image carried for the future native-image lane (docs/redesign/06)
struct V2Question {
    id: String,
    #[serde(default)]
    domain: Option<String>,
    #[serde(default)]
    environment: Option<String>,
    #[serde(default)]
    question_type: Option<String>,
    question: String,
    #[serde(default)]
    image: Option<String>,
    #[serde(default)]
    answer: Option<Value>,
    #[serde(default)]
    eval_function: Option<String>,
}

#[derive(Debug, Deserialize)]
struct V2Trajectory {
    id: String,
    #[serde(default)]
    goal: Option<String>,
    #[serde(default)]
    states: Vec<V2State>,
}

#[derive(Debug, Deserialize)]
struct V2State {
    #[serde(default)]
    step: Option<Value>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    action: Option<String>,
    #[serde(default)]
    thought: Option<String>,
    #[serde(default)]
    accessibility_tree: Option<String>,
}

impl LongMemEvalV2 {
    /// Which haystack tier to use (distractor scale). `MEMBENCH_V2_HAYSTACK=small|medium`, default small.
    fn haystack_file() -> &'static str {
        match std::env::var("MEMBENCH_V2_HAYSTACK").ok().as_deref() {
            Some("medium") => "lme_v2_medium.json",
            _ => "lme_v2_small.json",
        }
    }

    fn read_questions(dir: &Path, limit: Option<usize>) -> anyhow::Result<Vec<V2Question>> {
        let raw = std::fs::read_to_string(dir.join("questions.jsonl"))?;
        let mut out = Vec::new();
        for line in raw.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            out.push(serde_json::from_str::<V2Question>(line)?);
            if let Some(limit) = limit {
                if out.len() >= limit {
                    break;
                }
            }
        }
        Ok(out)
    }

    fn read_haystack_map(dir: &Path) -> anyhow::Result<HashMap<String, Vec<String>>> {
        let path = dir.join("haystacks").join(Self::haystack_file());
        let raw = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&raw)?)
    }

    /// Stream trajectories.jsonl (large), keeping only the trajectories in `needed`. Uses a cheap
    /// id prefix-scan to skip full JSON parsing of unneeded lines.
    fn read_trajectories(
        dir: &Path,
        needed: &HashSet<String>,
    ) -> anyhow::Result<HashMap<String, V2Trajectory>> {
        use std::io::BufRead;
        let file = std::fs::File::open(dir.join("trajectories.jsonl"))?;
        let reader = std::io::BufReader::new(file);
        let mut out = HashMap::new();
        for line in reader.lines() {
            let line = line?;
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            // Cheap id extraction: lines look like {"id": "abcd1234", ...}. Only full-parse if needed.
            let Some(id) = extract_json_id(line) else {
                continue;
            };
            if !needed.contains(id) {
                continue;
            }
            let traj: V2Trajectory = serde_json::from_str(line)?;
            out.insert(traj.id.clone(), traj);
        }
        Ok(out)
    }

    /// Render a single trajectory state as a turn's text (the text-projection of the observation).
    fn state_text(state: &V2State) -> String {
        let mut parts = Vec::new();
        let step = state
            .step
            .as_ref()
            .map(|s| match s {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            })
            .unwrap_or_default();
        let mut header = String::new();
        if !step.is_empty() {
            header.push_str(&format!("[step {step}] "));
        }
        if let Some(url) = &state.url {
            header.push_str(url);
        }
        if !header.is_empty() {
            parts.push(header);
        }
        if let Some(thought) = &state.thought {
            if !thought.trim().is_empty() {
                parts.push(format!("thought: {thought}"));
            }
        }
        if let Some(action) = &state.action {
            if !action.trim().is_empty() {
                parts.push(format!("action: {action}"));
            }
        }
        if let Some(tree) = &state.accessibility_tree {
            if !tree.trim().is_empty() {
                parts.push(format!("observation:\n{tree}"));
            }
        }
        parts.join("\n")
    }

    /// Render a trajectory as a "session" of turns (one per state, capped by `max_states`), with the
    /// trajectory goal prepended as scene-setting. Empty if the trajectory yields no text.
    fn build_session(traj: &V2Trajectory, max_states: Option<usize>) -> Vec<LongMemEvalMessage> {
        let mut session = Vec::with_capacity(traj.states.len());
        for state in traj.states.iter().take(max_states.unwrap_or(usize::MAX)) {
            let text = Self::state_text(state);
            if text.trim().is_empty() {
                continue;
            }
            session.push(LongMemEvalMessage {
                role: "observation".to_string(),
                content: text,
                has_answer: false, // v2 ships no gold-evidence labels (answer-graded)
            });
        }
        if session.is_empty() {
            return session;
        }
        if let Some(goal) = &traj.goal {
            if !goal.trim().is_empty() {
                session.insert(
                    0,
                    LongMemEvalMessage {
                        role: "goal".to_string(),
                        content: format!("goal: {goal}"),
                        has_answer: false,
                    },
                );
            }
        }
        session
    }
}

impl BenchmarkLoader for LongMemEvalV2 {
    fn id(&self) -> &'static str {
        "longmemeval-v2"
    }
    fn manifest_tag(&self) -> &'static str {
        "longmemeval-v2"
    }
    fn default_dataset(&self) -> DatasetSource {
        DatasetSource::Path {
            rel_path: "runs/inputs/longmemeval-v2",
            download_url: None, // multi-file dataset; pulled out-of-band (see docs/redesign/06)
        }
    }

    fn load(&self, path: &Path, limit: Option<usize>) -> anyhow::Result<Vec<LongMemEvalRecord>> {
        let questions = Self::read_questions(path, limit)?;
        let haystack_map = Self::read_haystack_map(path)?;
        // COST KNOBS. A v2 "small" haystack is ~100 trajectories / ~3.3k states / ~20M tokens PER
        // question (the corpus is shared across questions), so a full-haystack run is a 20M-token
        // embed per question. These caps bound trajectories/states for feasible pipeline-smoke runs.
        // Unset by default (the real benchmark). CAPPING IS NOT THE REAL SCORE: it may also drop the
        // (currently unmarked) gold trajectory, so a capped run is a pipeline smoke, not a benchmark
        // number. Logged when active.
        let max_traj = env_cap("MEMBENCH_V2_MAX_TRAJ");
        let max_states = env_cap("MEMBENCH_V2_MAX_STATES");
        if max_traj.is_some() || max_states.is_some() {
            eprintln!(
                "[longmemeval-v2] HAYSTACK CAPPED (pipeline smoke, not a benchmark score): max_traj={:?} max_states={:?}",
                max_traj, max_states
            );
        }
        // Collect exactly the (possibly capped) trajectories the selected questions reference.
        let mut needed: HashSet<String> = HashSet::new();
        for q in &questions {
            if let Some(ids) = haystack_map.get(&q.id) {
                let take = max_traj.unwrap_or(ids.len());
                needed.extend(ids.iter().take(take).cloned());
            }
        }
        let trajectories = Self::read_trajectories(path, &needed)?;

        let mut records = Vec::with_capacity(questions.len());
        for q in questions {
            let mut traj_ids = haystack_map.get(&q.id).cloned().unwrap_or_default();
            if let Some(cap) = max_traj {
                traj_ids.truncate(cap);
            }
            let mut haystack_sessions: Vec<Vec<LongMemEvalMessage>> = Vec::new();
            let mut haystack_session_ids: Vec<String> = Vec::new();
            for tid in &traj_ids {
                let Some(traj) = trajectories.get(tid) else {
                    continue;
                };
                let session = Self::build_session(traj, max_states);
                if session.is_empty() {
                    continue;
                }
                haystack_sessions.push(session);
                haystack_session_ids.push(tid.clone());
            }
            records.push(LongMemEvalRecord {
                question_id: q.id.clone(),
                question_type: q.question_type.clone(),
                question: q.question.clone(),
                question_date: None, // v2 has no timestamps
                answer: q.answer.clone(),
                answer_session_ids: Vec::new(), // gold-trajectory marking deferred
                haystack_dates: Vec::new(),
                haystack_session_ids,
                haystack_sessions,
            });
        }
        Ok(records)
    }

    fn grade_targets(&self, path: &Path) -> anyhow::Result<HashMap<String, GradeTarget>> {
        let questions = Self::read_questions(path, None)?;
        Ok(questions
            .into_iter()
            .map(|q| {
                (
                    q.id.clone(),
                    GradeTarget {
                        question_type: q.question_type.clone(),
                        gold_answer: q.answer.clone(),
                        eval_function: q.eval_function.clone(),
                    },
                )
            })
            .collect())
    }

    fn haystack_scope(&self) -> HaystackScope {
        HaystackScope::SharedCorpus
    }

    fn shared_questions(
        &self,
        path: &Path,
        limit: Option<usize>,
    ) -> anyhow::Result<Vec<SharedCorpusQuestion>> {
        let questions = Self::read_questions(path, limit)?;
        Ok(questions
            .into_iter()
            .map(|q| SharedCorpusQuestion {
                id: q.id,
                question: q.question,
                question_type: q.question_type,
                reference_date: None, // v2 has no timestamps
                // The corpus is shared per domain ("within each domain, all questions share one
                // 100-trajectory haystack" — SCHEMA.md). Fall back to a single corpus if absent.
                corpus_key: q.domain.unwrap_or_else(|| "all".to_string()),
            })
            .collect())
    }

    fn corpus_record(&self, path: &Path, corpus_key: &str) -> anyhow::Result<LongMemEvalRecord> {
        let questions = Self::read_questions(path, None)?;
        let haystack_map = Self::read_haystack_map(path)?;
        let max_traj = env_cap("MEMBENCH_V2_MAX_TRAJ");
        let max_states = env_cap("MEMBENCH_V2_MAX_STATES");
        if max_traj.is_some() || max_states.is_some() {
            eprintln!(
                "[longmemeval-v2] CORPUS CAPPED (pipeline smoke, not a benchmark score): max_traj={max_traj:?} max_states={max_states:?}"
            );
        }
        // Union of trajectory ids across every question in this corpus (domain). Per SCHEMA.md these
        // are the same shared ~100 trajectories, so the union IS the corpus.
        let mut traj_ids: Vec<String> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        for q in &questions {
            if q.domain.as_deref() != Some(corpus_key) {
                continue;
            }
            if let Some(ids) = haystack_map.get(&q.id) {
                for id in ids {
                    if seen.insert(id.clone()) {
                        traj_ids.push(id.clone());
                    }
                }
            }
        }
        if let Some(cap) = max_traj {
            traj_ids.truncate(cap);
        }
        let needed: HashSet<String> = traj_ids.iter().cloned().collect();
        let trajectories = Self::read_trajectories(path, &needed)?;

        let mut haystack_sessions = Vec::new();
        let mut haystack_session_ids = Vec::new();
        for tid in &traj_ids {
            let Some(traj) = trajectories.get(tid) else {
                continue;
            };
            let session = Self::build_session(traj, max_states);
            if session.is_empty() {
                continue;
            }
            haystack_sessions.push(session);
            haystack_session_ids.push(tid.clone());
        }
        Ok(LongMemEvalRecord {
            question_id: format!("corpus:{corpus_key}"),
            question_type: None,
            question: String::new(),
            question_date: None,
            answer: None,
            answer_session_ids: Vec::new(),
            haystack_dates: Vec::new(),
            haystack_session_ids,
            haystack_sessions,
        })
    }
}

/// Parse a positive `usize` cap from an env var; `None` if unset/zero/invalid.
fn env_cap(var: &str) -> Option<usize> {
    std::env::var(var)
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|n| *n > 0)
}

/// Cheaply extract the top-level `"id"` string from a JSONL trajectory line without parsing the whole
/// object. Returns a slice into `line`. Assumes the compact form the dataset uses (`{"id": "...",`).
fn extract_json_id(line: &str) -> Option<&str> {
    let key = line.find("\"id\"")?;
    let after = &line[key + 4..];
    let colon = after.find(':')?;
    let rest = &after[colon + 1..];
    let q1 = rest.find('"')?;
    let rest2 = &rest[q1 + 1..];
    let q2 = rest2.find('"')?;
    Some(&rest2[..q2])
}

// ============================================================================
// LongMemEval-v2 grading — the `eval_function` DSL (deterministic matchers here; LLM checkers
// signalled back to the bin, which owns the judge).
// ============================================================================

/// What grading an item requires.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GradeOutcome {
    /// A deterministic verdict was computed here.
    Deterministic(bool),
    /// The eval_function needs an LLM judge; the bin runs it. Carries the checker kind.
    NeedsJudge(JudgeKind),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JudgeKind {
    Abstention,
    Gotchas,
    /// Unknown/other eval_function head — fall back to a generic contains-judge.
    Generic,
}

/// Grade a v2 hypothesis against gold using the question's `eval_function` DSL string.
/// Deterministic heads (`norm_phrase_set_match[_ordered]`, `mc_choice_match`) are decided here;
/// `llm_*_checker` heads return [`GradeOutcome::NeedsJudge`].
pub fn grade_v2(eval_function: &str, gold: &str, hypothesis: &str) -> GradeOutcome {
    let mut parts = eval_function.split('|');
    let head = parts.next().unwrap_or("").trim();
    let opts = parse_opts(parts);
    let answer = extract_boxed(hypothesis);

    match head {
        "norm_phrase_set_match" => {
            GradeOutcome::Deterministic(phrase_set_match(gold, &answer, &opts, false))
        }
        "norm_phrase_set_match_ordered" => {
            GradeOutcome::Deterministic(phrase_set_match(gold, &answer, &opts, true))
        }
        "mc_choice_match" => GradeOutcome::Deterministic(mc_choice_match(gold, &answer, &opts)),
        "llm_abstention_checker" => GradeOutcome::NeedsJudge(JudgeKind::Abstention),
        "llm_gotchas_checker" => GradeOutcome::NeedsJudge(JudgeKind::Gotchas),
        _ => GradeOutcome::NeedsJudge(JudgeKind::Generic),
    }
}

#[derive(Default, Debug)]
struct Opts {
    lower: bool,
    normalize_hyphen: bool,
    strip_punct: bool,
    separators: String,
    require_non_empty: bool,
}

fn parse_opts<'a>(parts: impl Iterator<Item = &'a str>) -> Opts {
    let mut opts = Opts {
        separators: ",;".to_string(),
        ..Default::default()
    };
    let mut saw_sep = false;
    for part in parts {
        let part = part.trim();
        let Some((k, v)) = part.split_once('=') else {
            continue;
        };
        let truthy = matches!(v.trim(), "true" | "1" | "yes" | "on");
        match k.trim() {
            "lower" => opts.lower = truthy,
            "normalize_hyphen" => opts.normalize_hyphen = truthy,
            "strip_punct" => opts.strip_punct = truthy,
            "require_non_empty" => opts.require_non_empty = truthy,
            "separators" => {
                opts.separators = v.trim().to_string();
                saw_sep = true;
            }
            _ => {}
        }
    }
    if !saw_sep {
        opts.separators = ",;".to_string();
    }
    opts
}

/// Extract the final `\boxed{...}` payload from a model answer; falls back to the whole (trimmed)
/// string when no box is present. Handles one level of nested braces.
pub fn extract_boxed(text: &str) -> String {
    let marker = "\\boxed{";
    if let Some(start) = text.rfind(marker) {
        let rest = &text[start + marker.len()..];
        let mut depth = 1usize;
        let mut end = None;
        for (i, ch) in rest.char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(i);
                        break;
                    }
                }
                _ => {}
            }
        }
        if let Some(end) = end {
            return rest[..end].trim().to_string();
        }
    }
    text.trim().to_string()
}

fn normalize(s: &str, opts: &Opts) -> String {
    let mut out = s.to_string();
    if opts.normalize_hyphen {
        // Map the Unicode hyphen/dash block (U+2010..=U+2015) to ASCII '-'.
        out = out
            .chars()
            .map(|c| if ('\u{2010}'..='\u{2015}').contains(&c) { '-' } else { c })
            .collect();
    }
    if opts.lower {
        out = out.to_lowercase();
    }
    if opts.strip_punct {
        out = out
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c.is_whitespace() || c == '-' {
                    c
                } else {
                    ' '
                }
            })
            .collect();
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn split_phrases(s: &str, opts: &Opts) -> Vec<String> {
    let seps: Vec<char> = opts.separators.chars().collect();
    s.split(|c| seps.contains(&c))
        .map(|p| normalize(p, opts))
        .filter(|p| !p.is_empty())
        .collect()
}

fn phrase_set_match(gold: &str, answer: &str, opts: &Opts, ordered: bool) -> bool {
    let gold_phrases = split_phrases(gold, opts);
    let answer_phrases = split_phrases(answer, opts);
    if opts.require_non_empty && answer_phrases.is_empty() {
        return false;
    }
    if ordered {
        gold_phrases == answer_phrases
    } else {
        let g: HashSet<&String> = gold_phrases.iter().collect();
        let a: HashSet<&String> = answer_phrases.iter().collect();
        g == a
    }
}

fn mc_choice_match(gold: &str, answer: &str, opts: &Opts) -> bool {
    let g = normalize(gold, opts);
    let a = normalize(answer, opts);
    if opts.require_non_empty && a.is_empty() {
        return false;
    }
    if g.is_empty() {
        return false;
    }
    // Accept exact normalized match, or the gold choice appearing as a standalone token in the answer.
    g == a || a.split_whitespace().any(|tok| tok == g.as_str()) || a.contains(g.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boxed_extraction() {
        assert_eq!(extract_boxed("blah \\boxed{Incident Portal}"), "Incident Portal");
        assert_eq!(extract_boxed("no box here"), "no box here");
        assert_eq!(
            extract_boxed("first \\boxed{a} then \\boxed{b, c}"),
            "b, c"
        );
    }

    #[test]
    fn norm_phrase_set_match_unordered() {
        let ef = "norm_phrase_set_match|lower=true|normalize_hyphen=true|strip_punct=true|separators=,;|require_non_empty=true";
        // gold order differs from answer order -> still matches (set).
        assert_eq!(
            grade_v2(ef, "Incident Mobile, Incident Portal, My Open Incidents", "\\boxed{My Open Incidents; Incident Portal; Incident Mobile}"),
            GradeOutcome::Deterministic(true)
        );
        // missing one phrase -> fail.
        assert_eq!(
            grade_v2(ef, "Incident Mobile, Incident Portal, My Open Incidents", "\\boxed{Incident Portal, Incident Mobile}"),
            GradeOutcome::Deterministic(false)
        );
    }

    #[test]
    fn ordered_match_respects_order() {
        let ef = "norm_phrase_set_match_ordered|lower=true|strip_punct=true|separators=;|require_non_empty=true";
        assert_eq!(
            grade_v2(ef, "alpha; beta", "\\boxed{alpha; beta}"),
            GradeOutcome::Deterministic(true)
        );
        assert_eq!(
            grade_v2(ef, "alpha; beta", "\\boxed{beta; alpha}"),
            GradeOutcome::Deterministic(false)
        );
    }

    #[test]
    fn empty_answer_fails_require_non_empty() {
        let ef = "norm_phrase_set_match|require_non_empty=true";
        assert_eq!(
            grade_v2(ef, "something", "\\boxed{}"),
            GradeOutcome::Deterministic(false)
        );
    }

    #[test]
    fn llm_heads_need_judge() {
        assert_eq!(
            grade_v2("llm_abstention_checker|require_non_empty=true", "x", "y"),
            GradeOutcome::NeedsJudge(JudgeKind::Abstention)
        );
        assert_eq!(
            grade_v2("llm_gotchas_checker|require_non_empty=true", "x", "y"),
            GradeOutcome::NeedsJudge(JudgeKind::Gotchas)
        );
    }
}
