#![cfg(feature = "symbiotic-memory-adapter")]

use std::fs;

use membench::benchmark::{
    BenchmarkLoader, GradeOutcome, HaystackScope, JudgeKind, LongMemEvalV2, grade_v2, loader_for,
};
use serde_json::json;

fn write_fixture() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("haystacks")).unwrap();
    fs::write(
        root.path().join("questions.jsonl"),
        [
            json!({
                "id": "web-1",
                "domain": "web",
                "question_type": "list",
                "question": "Which portals?",
                "answer": ["Alpha", "Beta"],
                "eval_function": "norm_phrase_set_match|lower=true|separators=,;"
            })
            .to_string(),
            json!({
                "id": "enterprise-1",
                "domain": "enterprise",
                "question_type": "choice",
                "question": "Which option?",
                "answer": "B",
                "eval_function": "mc_choice_match|lower=true"
            })
            .to_string(),
            json!({
                "id": "web-2",
                "domain": "web",
                "question_type": "list",
                "question": "Which mobile portal?",
                "answer": "Mobile Portal",
                "eval_function": "norm_phrase_set_match|lower=true"
            })
            .to_string(),
        ]
        .join("\n")
            + "\n",
    )
    .unwrap();
    fs::write(
        root.path().join("haystacks/lme_v2_small.json"),
        serde_json::to_string_pretty(&json!({
            "web-1": ["web-t1", "web-t2"],
            "enterprise-1": ["enterprise-t1"],
            "web-2": ["web-t1", "web-t2"]
        }))
        .unwrap(),
    )
    .unwrap();
    fs::write(
        root.path().join("trajectories.jsonl"),
        [
            json!({
                "id": "web-t1",
                "goal": "open incidents",
                "states": [{
                    "step": 1,
                    "url": "https://example.test/incidents",
                    "thought": "inspect the list",
                    "action": "click Open",
                    "accessibility_tree": "heading Incident Portal"
                }]
            })
            .to_string(),
            json!({
                "id": "web-t2",
                "states": [{"step": "2", "accessibility_tree": "heading Mobile Portal"}]
            })
            .to_string(),
            json!({
                "id": "enterprise-t1",
                "states": [{"step": 1, "accessibility_tree": "radio B selected"}]
            })
            .to_string(),
        ]
        .join("\n")
            + "\n",
    )
    .unwrap();
    root
}

#[test]
fn registry_preserves_v1_and_adds_v2() {
    assert_eq!(loader_for("long-mem-eval").unwrap().id(), "long-mem-eval");
    let v2 = loader_for("longmemeval-v2").unwrap();
    assert_eq!(v2.id(), "longmemeval-v2");
    assert_eq!(v2.haystack_scope(), HaystackScope::SharedCorpus);
    assert!(loader_for("unknown").is_none());
}

#[test]
fn v2_loader_projects_shared_domain_corpus_into_text_turns() {
    let fixture = write_fixture();
    let loader = LongMemEvalV2;
    let questions = loader.shared_questions(fixture.path(), Some(1)).unwrap();
    assert_eq!(questions.len(), 1);
    assert_eq!(questions[0].id, "web-1");
    assert_eq!(questions[0].corpus_key, "web");

    let corpus = loader.corpus_record(fixture.path(), "web").unwrap();
    assert_eq!(corpus.question_id, "corpus:web");
    assert_eq!(corpus.haystack_session_ids, ["web-t1", "web-t2"]);
    assert_eq!(corpus.haystack_sessions.len(), 2);
    let first = &corpus.haystack_sessions[0];
    assert_eq!(first[0].role, "goal");
    assert!(first[1].content.contains("heading Incident Portal"));
    assert!(first[1].content.contains("action: click Open"));
}

#[test]
fn v2_loader_fails_closed_when_haystack_trajectory_is_missing() {
    let fixture = write_fixture();
    let path = fixture.path().join("trajectories.jsonl");
    let raw = fs::read_to_string(&path).unwrap();
    fs::write(
        &path,
        raw.lines()
            .filter(|line| !line.contains("\"web-t2\""))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n",
    )
    .unwrap();

    let err = LongMemEvalV2
        .corpus_record(fixture.path(), "web")
        .unwrap_err();
    assert!(err.to_string().contains("web-t2"));
}

#[test]
fn v2_loader_rejects_a_non_shared_small_domain_haystack() {
    let fixture = write_fixture();
    fs::write(
        fixture.path().join("haystacks/lme_v2_small.json"),
        serde_json::to_string_pretty(&json!({
            "web-1": ["web-t1", "web-t2"],
            "enterprise-1": ["enterprise-t1"],
            "web-2": ["web-t2"]
        }))
        .unwrap(),
    )
    .unwrap();

    let err = LongMemEvalV2
        .corpus_record(fixture.path(), "web")
        .unwrap_err();
    assert!(err.to_string().contains("not shared consistently"));
}

#[test]
fn v2_deterministic_graders_and_judge_routing_are_typed() {
    let set =
        "norm_phrase_set_match|lower=true|strip_punct=true|separators=,;|require_non_empty=true";
    assert_eq!(
        grade_v2(set, "Alpha, Beta", r"\boxed{beta; alpha}"),
        GradeOutcome::Deterministic(true)
    );
    assert_eq!(
        grade_v2(
            "norm_phrase_set_match_ordered|lower=true|separators=;",
            "alpha; beta",
            r"\boxed{beta; alpha}",
        ),
        GradeOutcome::Deterministic(false)
    );
    assert_eq!(
        grade_v2("mc_choice_match|lower=true", "B", r"\boxed{Option B}"),
        GradeOutcome::Deterministic(true)
    );
    assert_eq!(
        grade_v2("mc_choice_set_match", "A,B,F", r"\boxed{F, A, B}"),
        GradeOutcome::Deterministic(true)
    );
    assert_eq!(
        grade_v2("mc_choice_set_match", "A,B,F", r"\boxed{A, B}"),
        GradeOutcome::Deterministic(false)
    );
    assert_eq!(
        grade_v2("llm_gotchas_checker", "expected", "candidate"),
        GradeOutcome::Unsupported(JudgeKind::Gotchas)
    );
    assert_eq!(
        grade_v2("future_checker", "expected", "candidate"),
        GradeOutcome::Unsupported(JudgeKind::Generic)
    );
}
