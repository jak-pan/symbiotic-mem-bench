//! Benchmark loader and grading plug-in surface.
//!
//! The v0.1 LongMemEval path remains the default `PerQuestion` implementation. LongMemEval-v2 uses
//! the same normalized record at the adapter seam, but declares `SharedCorpus` so its domain corpus
//! is ingested once and queried by many questions.

use std::collections::{HashMap, HashSet};
use std::io::BufRead;
use std::path::Path;

use anyhow::Context;
use serde::Deserialize;
use serde_json::Value;

use crate::symbiotic_memory_adapter::{
    LongMemEvalMessage, LongMemEvalRecord, SharedCorpusQuestion, load_longmemeval,
};

pub enum DatasetSource {
    Path {
        rel_path: &'static str,
        download_url: Option<&'static str>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HaystackScope {
    PerQuestion,
    SharedCorpus,
}

#[derive(Clone, Debug)]
pub struct GradeTarget {
    pub question_type: Option<String>,
    pub gold_answer: Option<Value>,
    pub eval_function: Option<String>,
}

pub trait BenchmarkLoader: Send + Sync {
    fn id(&self) -> &'static str;
    fn manifest_tag(&self) -> &'static str;
    fn default_dataset(&self) -> DatasetSource;
    fn load(&self, path: &Path, limit: Option<usize>) -> anyhow::Result<Vec<LongMemEvalRecord>>;
    fn grade_targets(&self, path: &Path) -> anyhow::Result<HashMap<String, GradeTarget>>;

    fn haystack_scope(&self) -> HaystackScope {
        HaystackScope::PerQuestion
    }

    fn shared_questions(
        &self,
        _path: &Path,
        _limit: Option<usize>,
    ) -> anyhow::Result<Vec<SharedCorpusQuestion>> {
        anyhow::bail!("benchmark '{}' does not support shared corpora", self.id())
    }

    fn corpus_record(&self, _path: &Path, _corpus_key: &str) -> anyhow::Result<LongMemEvalRecord> {
        anyhow::bail!("benchmark '{}' does not support shared corpora", self.id())
    }
}

pub fn loader_for(id: &str) -> Option<Box<dyn BenchmarkLoader>> {
    match id {
        "long-mem-eval" => Some(Box::new(LongMemEvalV1)),
        "longmemeval-v2" => Some(Box::new(LongMemEvalV2)),
        _ => None,
    }
}

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
        Ok(load_longmemeval(path, None)?
            .into_iter()
            .map(|row| {
                (
                    row.question_id,
                    GradeTarget {
                        question_type: row.question_type,
                        gold_answer: row.answer,
                        eval_function: None,
                    },
                )
            })
            .collect())
    }
}

pub struct LongMemEvalV2;

#[derive(Debug, Deserialize)]
struct V2Question {
    id: String,
    #[serde(default)]
    domain: Option<String>,
    #[serde(default)]
    question_type: Option<String>,
    question: String,
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
struct V2TrajectoryId {
    id: String,
}

#[derive(Debug, Deserialize)]
struct V2State {
    #[serde(default)]
    step: Option<Value>,
    #[serde(default)]
    state_index: Option<Value>,
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
    fn haystack_file() -> anyhow::Result<&'static str> {
        match std::env::var("MEMBENCH_V2_HAYSTACK").ok().as_deref() {
            None | Some("") | Some("small") => Ok("lme_v2_small.json"),
            Some("medium") => Ok("lme_v2_medium.json"),
            Some(other) => {
                anyhow::bail!("MEMBENCH_V2_HAYSTACK must be 'small' or 'medium', got '{other}'")
            }
        }
    }

    fn read_questions(dir: &Path, limit: Option<usize>) -> anyhow::Result<Vec<V2Question>> {
        let path = dir.join("questions.jsonl");
        let file = std::fs::File::open(&path)
            .with_context(|| format!("open v2 questions {}", path.display()))?;
        let mut questions = Vec::new();
        let mut ids = HashSet::new();
        for (index, line) in std::io::BufReader::new(file).lines().enumerate() {
            let line =
                line.with_context(|| format!("read {} line {}", path.display(), index + 1))?;
            if line.trim().is_empty() {
                continue;
            }
            let question: V2Question = serde_json::from_str(&line)
                .with_context(|| format!("parse {} line {}", path.display(), index + 1))?;
            if !ids.insert(question.id.clone()) {
                anyhow::bail!("duplicate v2 question id '{}'", question.id);
            }
            questions.push(question);
            if limit.is_some_and(|limit| questions.len() >= limit) {
                break;
            }
        }
        Ok(questions)
    }

    fn read_haystack_map(dir: &Path) -> anyhow::Result<HashMap<String, Vec<String>>> {
        let path = dir.join("haystacks").join(Self::haystack_file()?);
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("read v2 haystack map {}", path.display()))?;
        serde_json::from_str(&raw)
            .with_context(|| format!("parse v2 haystack map {}", path.display()))
    }

    fn read_trajectories(
        dir: &Path,
        needed: &HashSet<String>,
    ) -> anyhow::Result<HashMap<String, V2Trajectory>> {
        let path = dir.join("trajectories.jsonl");
        let file = std::fs::File::open(&path)
            .with_context(|| format!("open v2 trajectories {}", path.display()))?;
        let mut trajectories = HashMap::new();
        for (index, line) in std::io::BufReader::new(file).lines().enumerate() {
            let line =
                line.with_context(|| format!("read {} line {}", path.display(), index + 1))?;
            if line.trim().is_empty() {
                continue;
            }
            let header: V2TrajectoryId = serde_json::from_str(&line).with_context(|| {
                format!("parse trajectory id at {}:{}", path.display(), index + 1)
            })?;
            if !needed.contains(&header.id) {
                continue;
            }
            let trajectory: V2Trajectory = serde_json::from_str(&line).with_context(|| {
                format!(
                    "parse trajectory '{}' at {}:{}",
                    header.id,
                    path.display(),
                    index + 1
                )
            })?;
            if trajectories
                .insert(trajectory.id.clone(), trajectory)
                .is_some()
            {
                anyhow::bail!("duplicate v2 trajectory id '{}'", header.id);
            }
        }

        let mut missing: Vec<_> = needed
            .iter()
            .filter(|id| !trajectories.contains_key(*id))
            .cloned()
            .collect();
        missing.sort();
        if !missing.is_empty() {
            anyhow::bail!(
                "v2 haystack references missing trajectories: {}",
                missing.join(", ")
            );
        }
        Ok(trajectories)
    }

    fn state_text(state: &V2State) -> String {
        let mut parts = Vec::new();
        let step = state
            .step
            .as_ref()
            .or(state.state_index.as_ref())
            .map(value_text)
            .unwrap_or_default();
        let mut header = String::new();
        if !step.is_empty() {
            header.push_str(&format!("[step {step}]"));
        }
        if let Some(url) = non_empty(state.url.as_deref()) {
            if !header.is_empty() {
                header.push(' ');
            }
            header.push_str(url);
        }
        if !header.is_empty() {
            parts.push(header);
        }
        if let Some(thought) = non_empty(state.thought.as_deref()) {
            parts.push(format!("thought: {thought}"));
        }
        if let Some(action) = non_empty(state.action.as_deref()) {
            parts.push(format!("action: {action}"));
        }
        if let Some(tree) = non_empty(state.accessibility_tree.as_deref()) {
            parts.push(format!("observation:\n{tree}"));
        }
        parts.join("\n")
    }

    fn build_session(
        trajectory: &V2Trajectory,
        max_states: Option<usize>,
    ) -> Vec<LongMemEvalMessage> {
        let mut session = Vec::new();
        if let Some(goal) = non_empty(trajectory.goal.as_deref()) {
            session.push(LongMemEvalMessage {
                role: "goal".to_string(),
                content: format!("goal: {goal}"),
                has_answer: false,
            });
        }
        for state in trajectory
            .states
            .iter()
            .take(max_states.unwrap_or(usize::MAX))
        {
            let content = Self::state_text(state);
            if !content.is_empty() {
                session.push(LongMemEvalMessage {
                    role: "observation".to_string(),
                    content,
                    has_answer: false,
                });
            }
        }
        session
    }

    fn trajectory_ids_for_questions(
        questions: &[V2Question],
        haystacks: &HashMap<String, Vec<String>>,
        max_trajectories: Option<usize>,
    ) -> anyhow::Result<Vec<String>> {
        let mut ids = Vec::new();
        let mut seen = HashSet::new();
        for question in questions {
            let question_ids = haystacks.get(&question.id).ok_or_else(|| {
                anyhow::anyhow!(
                    "v2 haystack map has no entry for question '{}'",
                    question.id
                )
            })?;
            for id in question_ids
                .iter()
                .take(max_trajectories.unwrap_or(usize::MAX))
            {
                if seen.insert(id.clone()) {
                    ids.push(id.clone());
                }
            }
        }
        Ok(ids)
    }

    fn shared_trajectory_ids(
        questions: &[V2Question],
        haystacks: &HashMap<String, Vec<String>>,
        max_trajectories: Option<usize>,
    ) -> anyhow::Result<Vec<String>> {
        if Self::haystack_file()? != "lme_v2_small.json" {
            anyhow::bail!(
                "LongMemEval-v2 medium haystacks are per-question scoped and are not supported by \
                 the shared-corpus runner"
            );
        }
        let first = questions
            .first()
            .ok_or_else(|| anyhow::anyhow!("shared corpus has no questions"))?;
        let first_ids = haystacks.get(&first.id).ok_or_else(|| {
            anyhow::anyhow!("v2 haystack map has no entry for question '{}'", first.id)
        })?;
        let expected: HashSet<_> = first_ids.iter().collect();
        for question in &questions[1..] {
            let ids = haystacks.get(&question.id).ok_or_else(|| {
                anyhow::anyhow!(
                    "v2 haystack map has no entry for question '{}'",
                    question.id
                )
            })?;
            let actual: HashSet<_> = ids.iter().collect();
            if actual != expected {
                anyhow::bail!(
                    "v2 small corpus '{}' is not shared consistently: question '{}' differs from '{}'",
                    question.domain.as_deref().unwrap_or("<missing>"),
                    question.id,
                    first.id
                );
            }
        }
        Ok(first_ids
            .iter()
            .take(max_trajectories.unwrap_or(usize::MAX))
            .cloned()
            .collect())
    }

    fn record_from_trajectory_ids(
        question_id: String,
        question_type: Option<String>,
        question: String,
        answer: Option<Value>,
        trajectory_ids: Vec<String>,
        trajectories: &HashMap<String, V2Trajectory>,
        max_states: Option<usize>,
    ) -> LongMemEvalRecord {
        let mut session_ids = Vec::new();
        let mut sessions = Vec::new();
        for id in trajectory_ids {
            let trajectory = trajectories
                .get(&id)
                .expect("trajectory completeness checked before record construction");
            let session = Self::build_session(trajectory, max_states);
            if session.is_empty() {
                continue;
            }
            session_ids.push(id);
            sessions.push(session);
        }
        LongMemEvalRecord {
            question_id,
            question_type,
            question,
            question_date: None,
            answer,
            answer_session_ids: Vec::new(),
            haystack_dates: Vec::new(),
            haystack_session_ids: session_ids,
            haystack_sessions: sessions,
        }
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
            download_url: None,
        }
    }

    fn load(&self, path: &Path, limit: Option<usize>) -> anyhow::Result<Vec<LongMemEvalRecord>> {
        let questions = Self::read_questions(path, limit)?;
        let haystacks = Self::read_haystack_map(path)?;
        let max_trajectories = env_cap("MEMBENCH_V2_MAX_TRAJ")?;
        let max_states = env_cap("MEMBENCH_V2_MAX_STATES")?;
        warn_if_capped(max_trajectories, max_states);
        let needed_ids =
            Self::trajectory_ids_for_questions(&questions, &haystacks, max_trajectories)?;
        let needed: HashSet<_> = needed_ids.into_iter().collect();
        let trajectories = Self::read_trajectories(path, &needed)?;

        questions
            .into_iter()
            .map(|question| {
                let ids = haystacks
                    .get(&question.id)
                    .expect("haystack completeness checked above")
                    .iter()
                    .take(max_trajectories.unwrap_or(usize::MAX))
                    .cloned()
                    .collect();
                Ok(Self::record_from_trajectory_ids(
                    question.id,
                    question.question_type,
                    question.question,
                    question.answer,
                    ids,
                    &trajectories,
                    max_states,
                ))
            })
            .collect()
    }

    fn grade_targets(&self, path: &Path) -> anyhow::Result<HashMap<String, GradeTarget>> {
        Ok(Self::read_questions(path, None)?
            .into_iter()
            .map(|question| {
                (
                    question.id,
                    GradeTarget {
                        question_type: question.question_type,
                        gold_answer: question.answer,
                        eval_function: question.eval_function,
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
        Self::read_questions(path, limit)?
            .into_iter()
            .map(|question| {
                let corpus_key = question.domain.ok_or_else(|| {
                    anyhow::anyhow!("v2 question '{}' has no domain/corpus key", question.id)
                })?;
                Ok(SharedCorpusQuestion {
                    id: question.id,
                    question: question.question,
                    question_type: question.question_type,
                    reference_date: None,
                    corpus_key,
                })
            })
            .collect()
    }

    fn corpus_record(&self, path: &Path, corpus_key: &str) -> anyhow::Result<LongMemEvalRecord> {
        let questions: Vec<_> = Self::read_questions(path, None)?
            .into_iter()
            .filter(|question| question.domain.as_deref() == Some(corpus_key))
            .collect();
        if questions.is_empty() {
            anyhow::bail!("v2 dataset has no questions for corpus '{corpus_key}'");
        }
        let haystacks = Self::read_haystack_map(path)?;
        let max_trajectories = env_cap("MEMBENCH_V2_MAX_TRAJ")?;
        let max_states = env_cap("MEMBENCH_V2_MAX_STATES")?;
        warn_if_capped(max_trajectories, max_states);
        let ids = Self::shared_trajectory_ids(&questions, &haystacks, max_trajectories)?;
        let needed: HashSet<_> = ids.iter().cloned().collect();
        let trajectories = Self::read_trajectories(path, &needed)?;
        Ok(Self::record_from_trajectory_ids(
            format!("corpus:{corpus_key}"),
            None,
            String::new(),
            None,
            ids,
            &trajectories,
            max_states,
        ))
    }
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn value_text(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        other => other.to_string(),
    }
}

fn env_cap(name: &str) -> anyhow::Result<Option<usize>> {
    match std::env::var(name) {
        Ok(value) => parse_cap_value(name, &value).map(Some),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => {
            anyhow::bail!("{name} must be valid UTF-8")
        }
    }
}

fn parse_cap_value(name: &str, value: &str) -> anyhow::Result<usize> {
    let parsed = value
        .trim()
        .parse::<usize>()
        .with_context(|| format!("{name} must be a positive integer, got '{value}'"))?;
    anyhow::ensure!(parsed > 0, "{name} must be greater than zero");
    Ok(parsed)
}

fn warn_if_capped(max_trajectories: Option<usize>, max_states: Option<usize>) {
    if max_trajectories.is_some() || max_states.is_some() {
        eprintln!(
            "[longmemeval-v2] corpus capped for pipeline smoke only; max_traj={max_trajectories:?} max_states={max_states:?}"
        );
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GradeOutcome {
    Deterministic(bool),
    Unsupported(JudgeKind),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JudgeKind {
    Abstention,
    Gotchas,
    Generic,
}

impl JudgeKind {
    pub fn id(&self) -> &'static str {
        match self {
            Self::Abstention => "llm_abstention_checker",
            Self::Gotchas => "llm_gotchas_checker",
            Self::Generic => "unknown_eval_function",
        }
    }
}

pub fn grade_v2(eval_function: &str, gold: &str, hypothesis: &str) -> GradeOutcome {
    let mut parts = eval_function.split('|');
    let head = parts.next().unwrap_or_default().trim();
    let options = GraderOptions::parse(parts);
    let answer = extract_boxed(hypothesis);
    match head {
        "norm_phrase_set_match" => {
            GradeOutcome::Deterministic(phrase_set_match(gold, &answer, &options, false))
        }
        "norm_phrase_set_match_ordered" => {
            GradeOutcome::Deterministic(phrase_set_match(gold, &answer, &options, true))
        }
        "mc_choice_match" => GradeOutcome::Deterministic(mc_choice_match(gold, &answer, &options)),
        "mc_choice_set_match" => {
            GradeOutcome::Deterministic(phrase_set_match(gold, &answer, &options, false))
        }
        "llm_abstention_checker" => GradeOutcome::Unsupported(JudgeKind::Abstention),
        "llm_gotchas_checker" => GradeOutcome::Unsupported(JudgeKind::Gotchas),
        _ => GradeOutcome::Unsupported(JudgeKind::Generic),
    }
}

#[derive(Debug)]
struct GraderOptions {
    lower: bool,
    normalize_hyphen: bool,
    strip_punct: bool,
    separators: String,
    require_non_empty: bool,
}

impl GraderOptions {
    fn parse<'a>(parts: impl Iterator<Item = &'a str>) -> Self {
        let mut options = Self {
            lower: false,
            normalize_hyphen: false,
            strip_punct: false,
            separators: ",;".to_string(),
            require_non_empty: false,
        };
        for part in parts {
            let Some((key, value)) = part.trim().split_once('=') else {
                continue;
            };
            let truthy = matches!(value.trim(), "true" | "1" | "yes" | "on");
            match key.trim() {
                "lower" => options.lower = truthy,
                "normalize_hyphen" => options.normalize_hyphen = truthy,
                "strip_punct" => options.strip_punct = truthy,
                "separators" => options.separators = value.trim().to_string(),
                "require_non_empty" => options.require_non_empty = truthy,
                _ => {}
            }
        }
        options
    }
}

pub fn extract_boxed(text: &str) -> String {
    let marker = r"\boxed{";
    let Some(start) = text.rfind(marker) else {
        return text.trim().to_string();
    };
    let rest = &text[start + marker.len()..];
    let mut depth = 1usize;
    for (index, character) in rest.char_indices() {
        match character {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return rest[..index].trim().to_string();
                }
            }
            _ => {}
        }
    }
    text.trim().to_string()
}

fn normalize(value: &str, options: &GraderOptions) -> String {
    let mut normalized: String = value
        .chars()
        .map(|character| {
            if options.normalize_hyphen && ('\u{2010}'..='\u{2015}').contains(&character) {
                '-'
            } else if options.strip_punct
                && !character.is_alphanumeric()
                && !character.is_whitespace()
                && character != '-'
            {
                ' '
            } else {
                character
            }
        })
        .collect();
    if options.lower {
        normalized = normalized.to_lowercase();
    }
    normalized.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn phrases(value: &str, options: &GraderOptions) -> Vec<String> {
    value
        .split(|character| options.separators.contains(character))
        .map(|phrase| normalize(phrase, options))
        .filter(|phrase| !phrase.is_empty())
        .collect()
}

fn phrase_set_match(gold: &str, answer: &str, options: &GraderOptions, ordered: bool) -> bool {
    let gold = phrases(gold, options);
    let answer = phrases(answer, options);
    if options.require_non_empty && answer.is_empty() {
        return false;
    }
    if ordered {
        return gold == answer;
    }
    let gold: HashSet<_> = gold.iter().collect();
    let answer: HashSet<_> = answer.iter().collect();
    gold == answer
}

fn mc_choice_match(gold: &str, answer: &str, options: &GraderOptions) -> bool {
    let gold = normalize(gold, options);
    let answer = normalize(answer, options);
    if gold.is_empty() || (options.require_non_empty && answer.is_empty()) {
        return false;
    }
    gold == answer
        || answer
            .split(|character: char| !character.is_alphanumeric())
            .any(|token| token == gold)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boxed_extraction_uses_last_complete_box_and_handles_nesting() {
        assert_eq!(
            extract_boxed(r"first \boxed{a} then \boxed{b {c}}"),
            "b {c}"
        );
        assert_eq!(extract_boxed("plain"), "plain");
    }

    #[test]
    fn invalid_caps_fail_closed() {
        assert_eq!(parse_cap_value("CAP", "2").unwrap(), 2);
        for invalid in ["", "0", "-1", "not-a-number"] {
            assert!(parse_cap_value("CAP", invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn unknown_grader_is_typed_as_unsupported() {
        assert_eq!(
            grade_v2("future_checker", "gold", "answer"),
            GradeOutcome::Unsupported(JudgeKind::Generic)
        );
    }
}
