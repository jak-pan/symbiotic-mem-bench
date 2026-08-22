//! Provider-neutral multimodal recall experiment contract.
//!
//! The harness owns fixtures, experiment arms, scoring, provenance, and proof that a requested
//! arm actually executed. A memory-system adapter owns ingestion and recall. This boundary keeps
//! benchmark code from copying a product's parser, blob store, index, or recall implementation.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Component, Path};

use anyhow::Context;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const FIXTURE_SCHEMA: &str = "membench.multimodal_fixture.v1";
pub const ANNOTATION_SCHEMA: &str = "membench.longmemeval_v2_image_annotations.v1";
pub const RESULT_SCHEMA: &str = "membench.multimodal_result.v1";

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MultimodalFixtureSet {
    pub schema: String,
    pub fixture_set_id: String,
    pub split: FixtureSplit,
    pub source: DatasetProvenance,
    pub cases: Vec<MultimodalCase>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FixtureSplit {
    Development,
    HeldOut,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DatasetProvenance {
    pub dataset_id: String,
    pub version: String,
    /// SHA-256 over the source files used to build this fixture set.
    pub source_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_uri: Option<String>,
    /// Whether this fixture is identical to an official benchmark release and scoring protocol.
    pub official_equivalent: bool,
    /// Subset, oracle, synthetic, and apparatus-only fixtures must set this false.
    pub leaderboard_eligible: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MultimodalCase {
    pub case_id: String,
    pub origin: FixtureOrigin,
    pub corpus_id: String,
    pub question: ContentSpec,
    pub gold: GoldAnswer,
    /// Human-reviewed label. The harness never infers this from question text or file extension.
    pub media_dependent: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_dependence_review: Option<MediaDependenceReview>,
    pub oracle_lane: RecallLane,
    pub oracle_evidence: Vec<EvidenceRegion>,
    pub evidence: Vec<EvidenceItem>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MediaDependenceReview {
    pub reviewed_by: String,
    pub rationale: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FixtureOrigin {
    LongMemEvalV2,
    HeldOutPdf,
    HeldOutSpreadsheet,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContentSpec {
    pub text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub media: Vec<MediaRef>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Ord, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MediaRef {
    pub asset_id: String,
    /// Dataset-relative locator or a content-addressed adapter locator. Never inline base64.
    pub locator: String,
    pub sha256: String,
    pub media_type: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceItem {
    pub evidence_id: String,
    pub source_media: MediaRef,
    pub region: EvidenceRegion,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_projection: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum EvidenceRegion {
    Screenshot {
        trajectory_id: String,
        state_index: u32,
        locator: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bounds: Option<NormalizedBounds>,
    },
    Page {
        document_id: String,
        page_number: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bounds: Option<NormalizedBounds>,
    },
    Sheet {
        workbook_id: String,
        sheet_name: String,
        range: String,
    },
    Cell {
        workbook_id: String,
        sheet_name: String,
        cell: String,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NormalizedBounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GoldAnswer {
    pub value: String,
    pub scoring: ScoringRule,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LongMemEvalV2ImageAnnotations {
    pub schema: String,
    pub fixture_set_id: String,
    pub dataset_version: String,
    pub haystack_file: String,
    pub annotations: Vec<LongMemEvalV2ImageAnnotation>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LongMemEvalV2ImageAnnotation {
    pub question_id: String,
    /// Must be explicitly true. The loader never guesses image dependence from file names or text.
    pub image_dependent: bool,
    pub reviewed_by: String,
    pub rationale: String,
    pub oracle_lane: RecallLane,
    pub evidence: Vec<LongMemEvalV2ScreenshotEvidence>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LongMemEvalV2ScreenshotEvidence {
    pub trajectory_id: String,
    pub state_index: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounds: Option<NormalizedBounds>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ScoringRule {
    ExactNormalized,
    ContainsAll {
        #[serde(default)]
        case_sensitive: bool,
    },
    /// An upstream benchmark-owned evaluator. Core refuses this unless a scorer plug-in supports it.
    External {
        evaluator: String,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecallLane {
    TextProjection,
    Native,
    HybridCollapsed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExperimentCell {
    ATextControl,
    BNative,
    CHybridCollapsed,
    DOracleLane,
    EReaderModality,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReaderMode {
    TextProjection,
    SourceBlob,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExperimentArm {
    pub cell: ExperimentCell,
    pub reader_mode: ReaderMode,
}

impl ExperimentArm {
    pub fn text_control() -> Self {
        Self {
            cell: ExperimentCell::ATextControl,
            reader_mode: ReaderMode::TextProjection,
        }
    }

    pub fn native() -> Self {
        Self {
            cell: ExperimentCell::BNative,
            reader_mode: ReaderMode::TextProjection,
        }
    }

    pub fn hybrid() -> Self {
        Self {
            cell: ExperimentCell::CHybridCollapsed,
            reader_mode: ReaderMode::TextProjection,
        }
    }

    pub fn oracle() -> Self {
        Self {
            cell: ExperimentCell::DOracleLane,
            reader_mode: ReaderMode::TextProjection,
        }
    }

    pub fn reader_modality(reader_mode: ReaderMode) -> Self {
        Self {
            cell: ExperimentCell::EReaderModality,
            reader_mode,
        }
    }

    fn validate(&self) -> anyhow::Result<()> {
        if self.cell != ExperimentCell::EReaderModality {
            anyhow::ensure!(
                self.reader_mode == ReaderMode::TextProjection,
                "source-blob reader mode is only valid for cell E"
            );
        }
        Ok(())
    }

    fn requested_lane(&self, case: &MultimodalCase) -> RecallLane {
        match self.cell {
            ExperimentCell::ATextControl => RecallLane::TextProjection,
            ExperimentCell::BNative => RecallLane::Native,
            ExperimentCell::CHybridCollapsed | ExperimentCell::EReaderModality => {
                RecallLane::HybridCollapsed
            }
            ExperimentCell::DOracleLane => case.oracle_lane,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MultimodalCapabilities {
    pub text_projection_recall: bool,
    pub native_image_recall: bool,
    pub native_document_recall: bool,
    pub hybrid_collapse: bool,
    pub text_projection_reader: bool,
    pub source_blob_reader: bool,
    pub oracle_region_filter: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterDescriptor {
    pub adapter_id: String,
    pub adapter_version: String,
    pub capabilities: MultimodalCapabilities,
}

pub trait MultimodalRecallAdapter {
    fn descriptor(&self) -> AdapterDescriptor;

    /// The request contains no gold answer. Cell D contains gold *evidence regions* and is stamped
    /// as oracle-only in provenance; it can never be mistaken for a normal system score.
    fn recall(&mut self, request: &RecallRequest) -> Result<RecallResponse, AdapterFailure>;
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecallRequest {
    pub case_id: String,
    pub corpus_id: String,
    pub question: ContentSpec,
    pub evidence: Vec<EvidenceItem>,
    pub lane: RecallLane,
    pub reader_mode: ReaderMode,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub oracle_regions: Vec<EvidenceRegion>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecallResponse {
    pub answer: String,
    pub retrieved: Vec<RetrievedEvidence>,
    pub execution: AdapterExecutionProof,
    pub provider_calls: u64,
    pub cost_micro_usd: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RetrievedEvidence {
    pub evidence_id: String,
    pub lane: RecallLane,
    pub region: EvidenceRegion,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rendered_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_media: Option<MediaRef>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterExecutionProof {
    pub effective_lane: RecallLane,
    pub effective_reader_mode: ReaderMode,
    pub text_branch_candidates: usize,
    pub native_branch_candidates: usize,
    pub collapsed_duplicates: usize,
    pub oracle_regions_applied: Vec<EvidenceRegion>,
    pub reader_media_parts: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdapterFailure {
    CapabilityUnavailable(String),
    Failed(String),
}

impl std::fmt::Display for AdapterFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CapabilityUnavailable(message) => {
                write!(formatter, "adapter capability unavailable: {message}")
            }
            Self::Failed(message) => write!(formatter, "adapter recall failed: {message}"),
        }
    }
}

impl std::error::Error for AdapterFailure {}

pub trait MultimodalScorer {
    fn scorer_id(&self) -> &str;
    fn score(&self, rule: &ScoringRule, gold: &str, answer: &str) -> anyhow::Result<bool>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DeterministicScorer;

impl MultimodalScorer for DeterministicScorer {
    fn scorer_id(&self) -> &str {
        "membench-deterministic-v1"
    }

    fn score(&self, rule: &ScoringRule, gold: &str, answer: &str) -> anyhow::Result<bool> {
        match rule {
            ScoringRule::ExactNormalized => Ok(normalize_text(gold) == normalize_text(answer)),
            ScoringRule::ContainsAll { case_sensitive } => {
                let (gold, answer) = if *case_sensitive {
                    (gold.to_string(), answer.to_string())
                } else {
                    (gold.to_lowercase(), answer.to_lowercase())
                };
                let phrases: Vec<_> = gold
                    .split([';', ','])
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .collect();
                Ok(!phrases.is_empty()
                    && phrases
                        .iter()
                        .all(|phrase| contains_word_bounded(&answer, phrase)))
            }
            ScoringRule::External { evaluator } => anyhow::bail!(
                "scorer '{}' does not support external evaluator '{evaluator}'",
                self.scorer_id()
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CostLadderStep {
    ValidateApparatus,
    ProjectionControl,
    OfflineNative,
    ProviderPilot,
    StratifiedMedium,
    FullBenchmark,
}

impl CostLadderStep {
    pub fn permits_provider_calls(self) -> bool {
        self >= Self::ProviderPilot
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionBudget {
    pub max_step: CostLadderStep,
    pub max_provider_calls: u64,
    pub max_cost_micro_usd: u64,
}

impl ExecutionBudget {
    pub fn offline() -> Self {
        Self {
            max_step: CostLadderStep::OfflineNative,
            max_provider_calls: 0,
            max_cost_micro_usd: 0,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PreregisteredHypothesis {
    pub statement: String,
    pub mechanism: String,
    pub decision_gate: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MultimodalRunPlan {
    pub arm: ExperimentArm,
    pub budget: ExecutionBudget,
    pub hypothesis: PreregisteredHypothesis,
    /// Caller-controlled timestamp. Excluded from the deterministic run id.
    pub started_at: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MultimodalRunResult {
    pub schema: String,
    pub run_id: String,
    pub fixture_set_id: String,
    pub fixture_digest: String,
    pub arm: ExperimentArm,
    pub adapter: AdapterDescriptor,
    pub scorer_id: String,
    pub hypothesis: PreregisteredHypothesis,
    pub provenance: RunProvenance,
    pub metrics: MultimodalMetrics,
    pub cases: Vec<CaseResult>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RunProvenance {
    pub git_sha: String,
    pub started_at: String,
    pub provider_calls: u64,
    pub cost_micro_usd: u64,
    pub oracle_gold: bool,
    pub leaderboard_eligible: bool,
    pub budget: ExecutionBudget,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MultimodalMetrics {
    pub correct: usize,
    pub total: usize,
    pub media_dependent_correct: usize,
    pub media_dependent_total: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CaseResult {
    pub case_id: String,
    pub correct: bool,
    pub answer: String,
    pub requested_lane: RecallLane,
    pub fired_proof: FiredProof,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FiredProof {
    pub request_sha256: String,
    pub response_sha256: String,
    pub effective_lane: RecallLane,
    pub effective_reader_mode: ReaderMode,
    pub text_branch_candidates: usize,
    pub native_branch_candidates: usize,
    pub collapsed_duplicates: usize,
    pub oracle_region_count: usize,
    pub reader_media_parts: usize,
    pub retrieved_region_fingerprint: String,
}

pub fn run_experiment<A: MultimodalRecallAdapter, S: MultimodalScorer>(
    fixture: &MultimodalFixtureSet,
    plan: &MultimodalRunPlan,
    adapter: &mut A,
    scorer: &S,
) -> anyhow::Result<MultimodalRunResult> {
    validate_fixture(fixture)?;
    plan.arm.validate()?;
    chrono::DateTime::parse_from_rfc3339(&plan.started_at)
        .with_context(|| "started_at must be an RFC3339 timestamp")?;
    for (name, value) in [
        ("hypothesis statement", &plan.hypothesis.statement),
        ("hypothesis mechanism", &plan.hypothesis.mechanism),
        ("hypothesis decision gate", &plan.hypothesis.decision_gate),
    ] {
        anyhow::ensure!(!value.trim().is_empty(), "{name} must be non-empty");
    }

    let descriptor = adapter.descriptor();
    validate_descriptor(&descriptor)?;
    preflight_capabilities(fixture, &plan.arm, &descriptor.capabilities)?;

    let fixture_digest = stable_json_hash(fixture)?;
    let run_identity = serde_json::json!({
        "fixture_digest": fixture_digest,
        "arm": plan.arm,
        "adapter": descriptor,
        "scorer": scorer.scorer_id(),
        "hypothesis": plan.hypothesis,
        "budget": plan.budget,
    });
    let run_id = format!("mm-{}", &stable_json_hash(&run_identity)?[..16]);
    let mut provider_calls = 0u64;
    let mut cost_micro_usd = 0u64;
    let mut results = Vec::with_capacity(fixture.cases.len());

    for case in &fixture.cases {
        let lane = plan.arm.requested_lane(case);
        let oracle_regions = if plan.arm.cell == ExperimentCell::DOracleLane {
            case.oracle_evidence.clone()
        } else {
            Vec::new()
        };
        let request = RecallRequest {
            case_id: case.case_id.clone(),
            corpus_id: case.corpus_id.clone(),
            question: case.question.clone(),
            evidence: case.evidence.clone(),
            lane,
            reader_mode: plan.arm.reader_mode,
            oracle_regions,
        };
        let response = adapter
            .recall(&request)
            .map_err(|error| anyhow::anyhow!("case '{}': {error}", case.case_id))?;
        validate_response(case, &request, &response)?;
        provider_calls = provider_calls.saturating_add(response.provider_calls);
        cost_micro_usd = cost_micro_usd.saturating_add(response.cost_micro_usd);
        enforce_budget(&plan.budget, provider_calls, cost_micro_usd)?;
        let correct = scorer
            .score(&case.gold.scoring, &case.gold.value, &response.answer)
            .with_context(|| format!("score case '{}'", case.case_id))?;
        results.push(CaseResult {
            case_id: case.case_id.clone(),
            correct,
            answer: response.answer.clone(),
            requested_lane: lane,
            fired_proof: fired_proof(&request, &response)?,
        });
    }

    let media_ids: HashSet<_> = fixture
        .cases
        .iter()
        .filter(|case| case.media_dependent)
        .map(|case| case.case_id.as_str())
        .collect();
    let metrics = MultimodalMetrics {
        correct: results.iter().filter(|result| result.correct).count(),
        total: results.len(),
        media_dependent_correct: results
            .iter()
            .filter(|result| result.correct && media_ids.contains(result.case_id.as_str()))
            .count(),
        media_dependent_total: media_ids.len(),
    };
    let oracle_gold = plan.arm.cell == ExperimentCell::DOracleLane;
    Ok(MultimodalRunResult {
        schema: RESULT_SCHEMA.to_string(),
        run_id,
        fixture_set_id: fixture.fixture_set_id.clone(),
        fixture_digest,
        arm: plan.arm.clone(),
        adapter: descriptor,
        scorer_id: scorer.scorer_id().to_string(),
        hypothesis: plan.hypothesis.clone(),
        provenance: RunProvenance {
            git_sha: option_env!("GIT_SHA").unwrap_or("unknown").to_string(),
            started_at: plan.started_at.clone(),
            provider_calls,
            cost_micro_usd,
            oracle_gold,
            leaderboard_eligible: !oracle_gold && fixture.source.leaderboard_eligible,
            budget: plan.budget.clone(),
        },
        metrics,
        cases: results,
    })
}

pub fn load_fixture_file(path: &Path) -> anyhow::Result<MultimodalFixtureSet> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("read multimodal fixture {}", path.display()))?;
    let fixture: MultimodalFixtureSet = serde_json::from_slice(&bytes)
        .with_context(|| format!("parse multimodal fixture {}", path.display()))?;
    validate_fixture(&fixture)
        .with_context(|| format!("validate multimodal fixture {}", path.display()))?;
    Ok(fixture)
}

/// Builds a media-dependent LongMemEval-v2 subset from explicit human-reviewed annotations.
///
/// The loader verifies every screenshot against the selected question's haystack and state. It
/// hashes source bytes into `MediaRef`; it does not OCR, transcribe, embed, or call a provider.
/// Official evaluator strings remain `ScoringRule::External` so a missing evaluator fails closed.
pub fn load_longmemeval_v2_image_subset(
    dataset_root: &Path,
    annotations_path: &Path,
) -> anyhow::Result<MultimodalFixtureSet> {
    let annotation_bytes = std::fs::read(annotations_path)
        .with_context(|| format!("read annotations {}", annotations_path.display()))?;
    let annotations: LongMemEvalV2ImageAnnotations = serde_json::from_slice(&annotation_bytes)
        .with_context(|| format!("parse annotations {}", annotations_path.display()))?;
    anyhow::ensure!(
        annotations.schema == ANNOTATION_SCHEMA,
        "unsupported annotation schema '{}'; expected '{ANNOTATION_SCHEMA}'",
        annotations.schema
    );
    validate_id("fixture_set_id", &annotations.fixture_set_id)?;
    non_empty("dataset version", &annotations.dataset_version)?;
    anyhow::ensure!(
        !annotations.annotations.is_empty(),
        "image annotation set is empty"
    );
    let haystack_rel = Path::new("haystacks").join(&annotations.haystack_file);
    ensure_safe_relative(&haystack_rel, "haystack file")?;
    let questions_path = dataset_root.join("questions.jsonl");
    let trajectories_path = dataset_root.join("trajectories.jsonl");
    let haystack_path = dataset_root.join(&haystack_rel);
    let question_bytes = std::fs::read(&questions_path)
        .with_context(|| format!("read {}", questions_path.display()))?;
    let trajectory_bytes = std::fs::read(&trajectories_path)
        .with_context(|| format!("read {}", trajectories_path.display()))?;
    let haystack_bytes = std::fs::read(&haystack_path)
        .with_context(|| format!("read {}", haystack_path.display()))?;
    let questions = read_jsonl_map::<LmeV2Question>(&question_bytes, "question", |row| &row.id)?;
    let trajectories =
        read_jsonl_map::<LmeV2Trajectory>(&trajectory_bytes, "trajectory", |row| &row.id)?;
    let haystacks: BTreeMap<String, Vec<String>> = serde_json::from_slice(&haystack_bytes)
        .with_context(|| format!("parse {}", haystack_path.display()))?;

    let mut seen = HashSet::new();
    let mut cases = Vec::with_capacity(annotations.annotations.len());
    for annotation in &annotations.annotations {
        validate_id("question_id", &annotation.question_id)?;
        anyhow::ensure!(
            seen.insert(annotation.question_id.as_str()),
            "duplicate image annotation for question '{}'",
            annotation.question_id
        );
        anyhow::ensure!(
            annotation.image_dependent,
            "annotation '{}' must explicitly set image_dependent=true",
            annotation.question_id
        );
        non_empty("annotation rationale", &annotation.rationale)?;
        non_empty("annotation reviewer", &annotation.reviewed_by)?;
        anyhow::ensure!(
            matches!(
                annotation.oracle_lane,
                RecallLane::Native | RecallLane::HybridCollapsed
            ),
            "image-dependent annotation '{}' cannot use a text-only oracle lane",
            annotation.question_id
        );
        anyhow::ensure!(
            !annotation.evidence.is_empty(),
            "annotation '{}' has no screenshot evidence",
            annotation.question_id
        );
        let question = questions.get(&annotation.question_id).ok_or_else(|| {
            anyhow::anyhow!(
                "annotation references missing question '{}'",
                annotation.question_id
            )
        })?;
        non_empty("question text", &question.question)?;
        non_empty("question answer", &question.answer)?;
        non_empty("question evaluator", &question.eval_function)?;
        let allowed_trajectories: HashSet<_> = haystacks
            .get(&question.id)
            .ok_or_else(|| anyhow::anyhow!("haystack has no question '{}'", question.id))?
            .iter()
            .map(String::as_str)
            .collect();
        let mut question_media = Vec::new();
        if let Some(locator) = question.image.as_deref() {
            question_media.push(media_ref_from_dataset(
                dataset_root,
                &format!("question:{}", question.id),
                locator,
            )?);
        }
        let mut evidence = Vec::with_capacity(annotation.evidence.len());
        let mut oracle_evidence = Vec::with_capacity(annotation.evidence.len());
        for (index, labeled) in annotation.evidence.iter().enumerate() {
            validate_id("trajectory_id", &labeled.trajectory_id)?;
            anyhow::ensure!(
                allowed_trajectories.contains(labeled.trajectory_id.as_str()),
                "question '{}' annotation references trajectory '{}' outside its haystack",
                question.id,
                labeled.trajectory_id
            );
            let trajectory = trajectories.get(&labeled.trajectory_id).ok_or_else(|| {
                anyhow::anyhow!(
                    "annotation references missing trajectory '{}'",
                    labeled.trajectory_id
                )
            })?;
            anyhow::ensure!(
                trajectory.domain == question.domain,
                "question '{}' domain '{}' does not match trajectory '{}' domain '{}'",
                question.id,
                question.domain,
                trajectory.id,
                trajectory.domain
            );
            let state = trajectory
                .states
                .iter()
                .find(|state| state_index(state).is_some_and(|value| value == labeled.state_index))
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "trajectory '{}' has no state_index {}",
                        trajectory.id,
                        labeled.state_index
                    )
                })?;
            let region = EvidenceRegion::Screenshot {
                trajectory_id: trajectory.id.clone(),
                state_index: labeled.state_index,
                locator: state.screenshot.clone(),
                bounds: labeled.bounds.clone(),
            };
            let source_media = media_ref_from_dataset(
                dataset_root,
                &format!("trajectory:{}:{}", trajectory.id, labeled.state_index),
                &state.screenshot,
            )?;
            oracle_evidence.push(region.clone());
            evidence.push(EvidenceItem {
                evidence_id: format!("{}:screenshot:{index}", question.id),
                source_media,
                region,
                text_projection: state_text_projection(state),
            });
        }
        cases.push(MultimodalCase {
            case_id: question.id.clone(),
            origin: FixtureOrigin::LongMemEvalV2,
            corpus_id: question.domain.clone(),
            question: ContentSpec {
                text: question.question.clone(),
                media: question_media,
            },
            gold: GoldAnswer {
                value: question.answer.clone(),
                scoring: ScoringRule::External {
                    evaluator: question.eval_function.clone(),
                },
            },
            media_dependent: true,
            media_dependence_review: Some(MediaDependenceReview {
                reviewed_by: annotation.reviewed_by.clone(),
                rationale: annotation.rationale.clone(),
            }),
            oracle_lane: annotation.oracle_lane,
            oracle_evidence,
            evidence,
        });
    }

    let source_digest = sha256_parts(&[
        ("questions.jsonl", &question_bytes),
        ("trajectories.jsonl", &trajectory_bytes),
        (annotations.haystack_file.as_str(), &haystack_bytes),
        ("annotations.json", &annotation_bytes),
    ]);
    let fixture = MultimodalFixtureSet {
        schema: FIXTURE_SCHEMA.to_string(),
        fixture_set_id: annotations.fixture_set_id,
        split: FixtureSplit::HeldOut,
        source: DatasetProvenance {
            dataset_id: "longmemeval-v2-image-dependent".to_string(),
            version: annotations.dataset_version,
            source_digest,
            source_uri: Some(
                "https://huggingface.co/datasets/xiaowu0162/LongMemEval-v2".to_string(),
            ),
            official_equivalent: false,
            leaderboard_eligible: false,
        },
        cases,
    };
    validate_fixture(&fixture)?;
    Ok(fixture)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LmeV2Question {
    id: String,
    domain: String,
    #[serde(rename = "environment")]
    _environment: String,
    #[serde(rename = "question_type")]
    _question_type: String,
    question: String,
    image: Option<String>,
    answer: String,
    eval_function: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LmeV2Trajectory {
    id: String,
    domain: String,
    #[serde(rename = "environment")]
    _environment: String,
    #[serde(rename = "goal")]
    _goal: String,
    #[serde(rename = "outcome")]
    _outcome: String,
    #[serde(rename = "start_url")]
    _start_url: String,
    states: Vec<LmeV2State>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LmeV2State {
    #[serde(rename = "step")]
    _step: serde_json::Value,
    state_index: serde_json::Value,
    url: String,
    action: Option<String>,
    thought: Option<String>,
    accessibility_tree: String,
    screenshot: String,
}

fn state_text_projection(state: &LmeV2State) -> Option<String> {
    let mut parts = Vec::new();
    if !state.url.trim().is_empty() {
        parts.push(format!("url: {}", state.url.trim()));
    }
    if let Some(thought) = state
        .thought
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        parts.push(format!("thought: {thought}"));
    }
    if let Some(action) = state
        .action
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        parts.push(format!("action: {action}"));
    }
    if !state.accessibility_tree.trim().is_empty() {
        parts.push(format!("observation:\n{}", state.accessibility_tree.trim()));
    }
    (!parts.is_empty()).then(|| parts.join("\n"))
}

fn state_index(state: &LmeV2State) -> Option<u32> {
    match &state.state_index {
        serde_json::Value::Number(value) => value.as_u64().and_then(|value| value.try_into().ok()),
        serde_json::Value::String(value) => value.parse().ok(),
        _ => None,
    }
}

fn media_ref_from_dataset(
    dataset_root: &Path,
    asset_id: &str,
    locator: &str,
) -> anyhow::Result<MediaRef> {
    let relative = Path::new(locator);
    ensure_safe_relative(relative, "media locator")?;
    let canonical_root = std::fs::canonicalize(dataset_root)
        .with_context(|| format!("canonicalize dataset root {}", dataset_root.display()))?;
    let unresolved = dataset_root.join(relative);
    let canonical_media = std::fs::canonicalize(&unresolved)
        .with_context(|| format!("canonicalize media {}", unresolved.display()))?;
    anyhow::ensure!(
        canonical_media.starts_with(&canonical_root),
        "media locator '{locator}' resolves outside the dataset root"
    );
    let extension = relative
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let media_type = match extension.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        other => {
            anyhow::bail!("unsupported LongMemEval-v2 media extension '{other}' for '{locator}'")
        }
    };
    Ok(MediaRef {
        asset_id: asset_id.to_string(),
        locator: locator.to_string(),
        sha256: sha256_file(&canonical_media)?,
        media_type: media_type.to_string(),
    })
}

fn sha256_file(path: &Path) -> anyhow::Result<String> {
    use std::io::Read;

    let mut file = std::fs::File::open(path)
        .with_context(|| format!("open media for hashing {}", path.display()))?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 1024 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .with_context(|| format!("hash media {}", path.display()))?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn read_jsonl_map<'a, T: Deserialize<'a>>(
    bytes: &'a [u8],
    kind: &str,
    id: impl Fn(&T) -> &String,
) -> anyhow::Result<BTreeMap<String, T>> {
    let text = std::str::from_utf8(bytes).with_context(|| format!("{kind} JSONL is not UTF-8"))?;
    let mut rows = BTreeMap::new();
    for (index, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let row: T = serde_json::from_str(line)
            .with_context(|| format!("parse {kind} JSONL line {}", index + 1))?;
        let row_id = id(&row).clone();
        anyhow::ensure!(
            rows.insert(row_id.clone(), row).is_none(),
            "duplicate {kind} id '{row_id}'"
        );
    }
    anyhow::ensure!(!rows.is_empty(), "{kind} JSONL is empty");
    Ok(rows)
}

fn ensure_safe_relative(path: &Path, name: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        !path.as_os_str().is_empty() && !path.is_absolute(),
        "{name} must be a non-empty relative path"
    );
    anyhow::ensure!(
        path.components()
            .all(|component| matches!(component, Component::Normal(_))),
        "{name} contains an unsafe path component"
    );
    Ok(())
}

fn sha256_parts(parts: &[(&str, &[u8])]) -> String {
    let mut digest = Sha256::new();
    for (name, bytes) in parts {
        digest.update(name.as_bytes());
        digest.update([0]);
        digest.update(bytes);
        digest.update([0]);
    }
    format!("{:x}", digest.finalize())
}

pub fn validate_fixture(fixture: &MultimodalFixtureSet) -> anyhow::Result<()> {
    anyhow::ensure!(
        fixture.schema == FIXTURE_SCHEMA,
        "unsupported fixture schema '{}'; expected '{FIXTURE_SCHEMA}'",
        fixture.schema
    );
    validate_id("fixture_set_id", &fixture.fixture_set_id)?;
    validate_id("dataset_id", &fixture.source.dataset_id)?;
    non_empty("dataset version", &fixture.source.version)?;
    validate_sha256("source digest", &fixture.source.source_digest)?;
    anyhow::ensure!(
        !fixture.source.leaderboard_eligible
            || (fixture.source.official_equivalent && fixture.split == FixtureSplit::HeldOut),
        "leaderboard eligibility requires an official-equivalent held-out fixture"
    );
    anyhow::ensure!(!fixture.cases.is_empty(), "fixture set has no cases");
    let mut case_ids = HashSet::new();
    let mut fixture_media = BTreeMap::new();
    for case in &fixture.cases {
        validate_case(case)?;
        anyhow::ensure!(
            case_ids.insert(case.case_id.as_str()),
            "duplicate case id '{}'",
            case.case_id
        );
        for media in case
            .question
            .media
            .iter()
            .chain(case.evidence.iter().map(|item| &item.source_media))
        {
            if let Some(previous) = fixture_media.insert(media.asset_id.as_str(), media) {
                anyhow::ensure!(
                    previous == media,
                    "fixture asset id '{}' resolves to different media refs across cases",
                    media.asset_id
                );
            }
        }
    }
    Ok(())
}

fn validate_case(case: &MultimodalCase) -> anyhow::Result<()> {
    validate_id("case_id", &case.case_id)?;
    validate_id("corpus_id", &case.corpus_id)?;
    non_empty("question text", &case.question.text)?;
    non_empty("gold answer", &case.gold.value)?;
    match (&case.media_dependence_review, case.media_dependent) {
        (Some(review), true) => {
            non_empty("media-dependence reviewer", &review.reviewed_by)?;
            non_empty("media-dependence rationale", &review.rationale)?;
        }
        (None, true) => anyhow::bail!(
            "case '{}' is media-dependent but has no human review",
            case.case_id
        ),
        (Some(_), false) => anyhow::bail!(
            "case '{}' has a media-dependence review but media_dependent=false",
            case.case_id
        ),
        (None, false) => {}
    }
    anyhow::ensure!(
        !case.evidence.is_empty(),
        "case '{}' has no evidence",
        case.case_id
    );
    anyhow::ensure!(
        !case.oracle_evidence.is_empty(),
        "case '{}' has no oracle evidence",
        case.case_id
    );
    let mut media_by_id = BTreeMap::new();
    for media in case
        .question
        .media
        .iter()
        .chain(case.evidence.iter().map(|item| &item.source_media))
    {
        validate_media(media)?;
        if let Some(previous) = media_by_id.insert(media.asset_id.as_str(), media) {
            anyhow::ensure!(
                previous == media,
                "asset id '{}' resolves to different media refs",
                media.asset_id
            );
        }
    }
    let mut evidence_ids = HashSet::new();
    let evidence_regions: Vec<_> = case
        .evidence
        .iter()
        .map(|item| item.region.clone())
        .collect();
    for item in &case.evidence {
        validate_id("evidence_id", &item.evidence_id)?;
        anyhow::ensure!(
            evidence_ids.insert(item.evidence_id.as_str()),
            "duplicate evidence id '{}'",
            item.evidence_id
        );
        validate_region(&item.region)?;
        if let Some(text) = &item.text_projection {
            non_empty("text projection", text)?;
        }
    }
    for region in &case.oracle_evidence {
        validate_region(region)?;
        anyhow::ensure!(
            evidence_regions.contains(region),
            "case '{}' oracle region is absent from evidence",
            case.case_id
        );
    }
    Ok(())
}

fn validate_media(media: &MediaRef) -> anyhow::Result<()> {
    validate_id("asset_id", &media.asset_id)?;
    non_empty("media locator", &media.locator)?;
    non_empty("media type", &media.media_type)?;
    validate_sha256("media sha256", &media.sha256)?;
    if media.locator.contains("://") {
        let expected = format!("cas://sha256/{}", media.sha256);
        anyhow::ensure!(
            media.locator == expected,
            "adapter-addressed media locator must be its exact content-addressed '{expected}'"
        );
    } else {
        let path = Path::new(&media.locator);
        anyhow::ensure!(
            !path.is_absolute(),
            "media locator must be dataset-relative or adapter-addressed"
        );
        anyhow::ensure!(
            path.components().all(|component| !matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )),
            "media locator contains an unsafe path component"
        );
    }
    Ok(())
}

fn validate_region(region: &EvidenceRegion) -> anyhow::Result<()> {
    match region {
        EvidenceRegion::Screenshot {
            trajectory_id,
            locator,
            bounds,
            ..
        } => {
            validate_id("trajectory_id", trajectory_id)?;
            non_empty("screenshot locator", locator)?;
            if let Some(bounds) = bounds {
                validate_bounds(bounds)?;
            }
        }
        EvidenceRegion::Page {
            document_id,
            page_number,
            bounds,
        } => {
            validate_id("document_id", document_id)?;
            anyhow::ensure!(*page_number > 0, "page numbers are one-based");
            if let Some(bounds) = bounds {
                validate_bounds(bounds)?;
            }
        }
        EvidenceRegion::Sheet {
            workbook_id,
            sheet_name,
            range,
        } => {
            validate_id("workbook_id", workbook_id)?;
            non_empty("sheet name", sheet_name)?;
            validate_a1_range(range)?;
        }
        EvidenceRegion::Cell {
            workbook_id,
            sheet_name,
            cell,
        } => {
            validate_id("workbook_id", workbook_id)?;
            non_empty("sheet name", sheet_name)?;
            validate_a1_cell(cell)?;
        }
    }
    Ok(())
}

fn validate_bounds(bounds: &NormalizedBounds) -> anyhow::Result<()> {
    for (name, value) in [
        ("x", bounds.x),
        ("y", bounds.y),
        ("width", bounds.width),
        ("height", bounds.height),
    ] {
        anyhow::ensure!(value.is_finite(), "bounds {name} must be finite");
    }
    anyhow::ensure!(
        bounds.x >= 0.0 && bounds.y >= 0.0,
        "bounds origin must be non-negative"
    );
    anyhow::ensure!(
        bounds.width > 0.0 && bounds.height > 0.0,
        "bounds dimensions must be positive"
    );
    anyhow::ensure!(
        bounds.x + bounds.width <= 1.0 && bounds.y + bounds.height <= 1.0,
        "bounds must fit within normalized media coordinates"
    );
    Ok(())
}

fn validate_a1_range(range: &str) -> anyhow::Result<()> {
    let (start, end) = range
        .split_once(':')
        .ok_or_else(|| anyhow::anyhow!("sheet range '{range}' must use A1:A1 form"))?;
    validate_a1_cell(start)?;
    validate_a1_cell(end)
}

fn validate_a1_cell(cell: &str) -> anyhow::Result<()> {
    let split = cell
        .bytes()
        .position(|byte| byte.is_ascii_digit())
        .unwrap_or(cell.len());
    let (column, row) = cell.split_at(split);
    anyhow::ensure!(
        !column.is_empty() && column.bytes().all(|byte| byte.is_ascii_uppercase()),
        "cell '{cell}' must use uppercase A1 notation"
    );
    anyhow::ensure!(
        !row.is_empty()
            && row.bytes().all(|byte| byte.is_ascii_digit())
            && row != "0"
            && !row.starts_with('0'),
        "cell '{cell}' must use a positive row number"
    );
    Ok(())
}

fn preflight_capabilities(
    fixture: &MultimodalFixtureSet,
    arm: &ExperimentArm,
    capabilities: &MultimodalCapabilities,
) -> anyhow::Result<()> {
    let mut missing = BTreeSet::new();
    if arm.reader_mode == ReaderMode::TextProjection && !capabilities.text_projection_reader {
        missing.insert("text_projection_reader");
    }
    if arm.reader_mode == ReaderMode::SourceBlob && !capabilities.source_blob_reader {
        missing.insert("source_blob_reader");
    }
    if arm.cell == ExperimentCell::DOracleLane && !capabilities.oracle_region_filter {
        missing.insert("oracle_region_filter");
    }
    for case in &fixture.cases {
        match arm.requested_lane(case) {
            RecallLane::TextProjection if !capabilities.text_projection_recall => {
                missing.insert("text_projection_recall");
            }
            RecallLane::Native => {
                if case
                    .evidence
                    .iter()
                    .any(|item| matches!(item.region, EvidenceRegion::Screenshot { .. }))
                    && !capabilities.native_image_recall
                {
                    missing.insert("native_image_recall");
                }
                if case.evidence.iter().any(|item| {
                    matches!(
                        item.region,
                        EvidenceRegion::Page { .. }
                            | EvidenceRegion::Sheet { .. }
                            | EvidenceRegion::Cell { .. }
                    )
                }) && !capabilities.native_document_recall
                {
                    missing.insert("native_document_recall");
                }
            }
            RecallLane::HybridCollapsed => {
                if !capabilities.text_projection_recall {
                    missing.insert("text_projection_recall");
                }
                if !capabilities.hybrid_collapse {
                    missing.insert("hybrid_collapse");
                }
                if case
                    .evidence
                    .iter()
                    .any(|item| matches!(item.region, EvidenceRegion::Screenshot { .. }))
                    && !capabilities.native_image_recall
                {
                    missing.insert("native_image_recall");
                }
                if case.evidence.iter().any(|item| {
                    matches!(
                        item.region,
                        EvidenceRegion::Page { .. }
                            | EvidenceRegion::Sheet { .. }
                            | EvidenceRegion::Cell { .. }
                    )
                }) && !capabilities.native_document_recall
                {
                    missing.insert("native_document_recall");
                }
            }
            _ => {}
        }
    }
    anyhow::ensure!(
        missing.is_empty(),
        "adapter capability gap for {:?}: {}",
        arm.cell,
        missing.into_iter().collect::<Vec<_>>().join(", ")
    );
    Ok(())
}

fn validate_response(
    case: &MultimodalCase,
    request: &RecallRequest,
    response: &RecallResponse,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        !response.answer.trim().is_empty(),
        "case '{}' adapter returned an empty answer",
        case.case_id
    );
    anyhow::ensure!(
        !response.retrieved.is_empty(),
        "case '{}' adapter returned no retrieved evidence",
        case.case_id
    );
    anyhow::ensure!(
        response.execution.effective_lane == request.lane,
        "case '{}' adapter effective lane {:?} did not match requested {:?}",
        case.case_id,
        response.execution.effective_lane,
        request.lane
    );
    anyhow::ensure!(
        response.execution.effective_reader_mode == request.reader_mode,
        "case '{}' adapter reader mode did not match request",
        case.case_id
    );
    match request.lane {
        RecallLane::TextProjection => anyhow::ensure!(
            response.execution.text_branch_candidates > 0
                && response.execution.native_branch_candidates == 0,
            "case '{}' text control did not prove the text-only branch fired",
            case.case_id
        ),
        RecallLane::Native => anyhow::ensure!(
            response.execution.native_branch_candidates > 0,
            "case '{}' native branch did not fire",
            case.case_id
        ),
        RecallLane::HybridCollapsed => anyhow::ensure!(
            response.execution.text_branch_candidates > 0
                && response.execution.native_branch_candidates > 0,
            "case '{}' hybrid arm did not prove both branches fired",
            case.case_id
        ),
    }
    if request.lane == RecallLane::HybridCollapsed
        && case
            .evidence
            .iter()
            .any(|item| item.text_projection.is_some())
    {
        anyhow::ensure!(
            response.execution.collapsed_duplicates > 0,
            "case '{}' hybrid arm did not prove source-region collapse fired",
            case.case_id
        );
    }
    if request.oracle_regions.is_empty() {
        anyhow::ensure!(
            response.execution.oracle_regions_applied.is_empty(),
            "case '{}' adapter applied undeclared oracle regions",
            case.case_id
        );
    } else {
        anyhow::ensure!(
            response.execution.oracle_regions_applied == request.oracle_regions,
            "case '{}' oracle region proof did not match request",
            case.case_id
        );
    }
    match request.reader_mode {
        ReaderMode::TextProjection => anyhow::ensure!(
            response.execution.reader_media_parts == 0,
            "case '{}' text reader unexpectedly received media",
            case.case_id
        ),
        ReaderMode::SourceBlob => anyhow::ensure!(
            response.execution.reader_media_parts > 0,
            "case '{}' blob reader did not receive media",
            case.case_id
        ),
    }
    let evidence_by_id: BTreeMap<_, _> = case
        .evidence
        .iter()
        .map(|item| (item.evidence_id.as_str(), item))
        .collect();
    for retrieved in &response.retrieved {
        let source = evidence_by_id
            .get(retrieved.evidence_id.as_str())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "case '{}' adapter returned unknown evidence '{}'",
                    case.case_id,
                    retrieved.evidence_id
                )
            })?;
        anyhow::ensure!(
            retrieved.region == source.region,
            "case '{}' adapter changed evidence region '{}'",
            case.case_id,
            retrieved.evidence_id
        );
        if let Some(media) = &retrieved.source_media {
            anyhow::ensure!(
                media == &source.source_media,
                "case '{}' adapter changed source media '{}'",
                case.case_id,
                retrieved.evidence_id
            );
        }
        match request.lane {
            RecallLane::TextProjection => anyhow::ensure!(
                retrieved.lane == RecallLane::TextProjection,
                "case '{}' text arm returned non-text evidence",
                case.case_id
            ),
            RecallLane::Native => anyhow::ensure!(
                retrieved.lane == RecallLane::Native && retrieved.source_media.is_some(),
                "case '{}' native arm returned evidence without a native media reference",
                case.case_id
            ),
            RecallLane::HybridCollapsed => {}
        }
        if !request.oracle_regions.is_empty() {
            anyhow::ensure!(
                request.oracle_regions.contains(&retrieved.region),
                "case '{}' oracle arm returned evidence outside the oracle regions",
                case.case_id
            );
        }
    }
    match request.reader_mode {
        ReaderMode::TextProjection => anyhow::ensure!(
            response.retrieved.iter().any(|item| item
                .rendered_text
                .as_deref()
                .is_some_and(|text| !text.trim().is_empty())),
            "case '{}' text reader received no rendered text",
            case.case_id
        ),
        ReaderMode::SourceBlob => anyhow::ensure!(
            response
                .retrieved
                .iter()
                .any(|item| item.source_media.is_some()),
            "case '{}' blob reader received no source media reference",
            case.case_id
        ),
    }
    Ok(())
}

fn fired_proof(request: &RecallRequest, response: &RecallResponse) -> anyhow::Result<FiredProof> {
    let mut regions: Vec<_> = response
        .retrieved
        .iter()
        .map(|item| stable_json_hash(&item.region))
        .collect::<anyhow::Result<_>>()?;
    regions.sort();
    Ok(FiredProof {
        request_sha256: stable_json_hash(request)?,
        response_sha256: stable_json_hash(response)?,
        effective_lane: response.execution.effective_lane,
        effective_reader_mode: response.execution.effective_reader_mode,
        text_branch_candidates: response.execution.text_branch_candidates,
        native_branch_candidates: response.execution.native_branch_candidates,
        collapsed_duplicates: response.execution.collapsed_duplicates,
        oracle_region_count: response.execution.oracle_regions_applied.len(),
        reader_media_parts: response.execution.reader_media_parts,
        retrieved_region_fingerprint: stable_json_hash(&regions)?,
    })
}

fn enforce_budget(
    budget: &ExecutionBudget,
    provider_calls: u64,
    cost_micro_usd: u64,
) -> anyhow::Result<()> {
    if !budget.max_step.permits_provider_calls() {
        anyhow::ensure!(
            provider_calls == 0 && cost_micro_usd == 0,
            "offline cost-ladder step observed {provider_calls} provider calls and {cost_micro_usd} micro-USD"
        );
    }
    anyhow::ensure!(
        provider_calls <= budget.max_provider_calls,
        "provider-call budget exceeded: {provider_calls} > {}",
        budget.max_provider_calls
    );
    anyhow::ensure!(
        cost_micro_usd <= budget.max_cost_micro_usd,
        "cost budget exceeded: {cost_micro_usd} > {} micro-USD",
        budget.max_cost_micro_usd
    );
    Ok(())
}

fn validate_descriptor(descriptor: &AdapterDescriptor) -> anyhow::Result<()> {
    validate_id("adapter_id", &descriptor.adapter_id)?;
    non_empty("adapter version", &descriptor.adapter_version)
}

fn normalize_text(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn stable_json_hash(value: &impl Serialize) -> anyhow::Result<String> {
    let bytes = serde_json::to_vec(value)?;
    Ok(sha256(&bytes))
}

fn sha256(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}

fn validate_sha256(name: &str, value: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f')),
        "{name} must be a lowercase SHA-256 hex digest"
    );
    Ok(())
}

fn validate_id(name: &str, value: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        !value.is_empty()
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric()
                    || matches!(byte, b'-' | b'_' | b'.' | b':')),
        "{name} contains unsafe characters"
    );
    Ok(())
}

fn non_empty(name: &str, value: &str) -> anyhow::Result<()> {
    anyhow::ensure!(!value.trim().is_empty(), "{name} must be non-empty");
    Ok(())
}

/// No-network cell-A control. It performs deterministic lexical retrieval over the supplied text
/// projections and returns the top projection verbatim. It never inspects gold answers or oracle
/// evidence and deliberately advertises no native capability.
#[derive(Clone, Debug)]
pub struct TextProjectionBaseline {
    descriptor: AdapterDescriptor,
}

impl Default for TextProjectionBaseline {
    fn default() -> Self {
        Self {
            descriptor: AdapterDescriptor {
                adapter_id: "membench-text-projection-control".to_string(),
                adapter_version: "1".to_string(),
                capabilities: MultimodalCapabilities {
                    text_projection_recall: true,
                    text_projection_reader: true,
                    ..MultimodalCapabilities::default()
                },
            },
        }
    }
}

impl MultimodalRecallAdapter for TextProjectionBaseline {
    fn descriptor(&self) -> AdapterDescriptor {
        self.descriptor.clone()
    }

    fn recall(&mut self, request: &RecallRequest) -> Result<RecallResponse, AdapterFailure> {
        if request.lane != RecallLane::TextProjection {
            return Err(AdapterFailure::CapabilityUnavailable(format!(
                "lane {:?}",
                request.lane
            )));
        }
        if request.reader_mode != ReaderMode::TextProjection {
            return Err(AdapterFailure::CapabilityUnavailable(
                "source-blob reader".to_string(),
            ));
        }
        if !request.oracle_regions.is_empty() {
            return Err(AdapterFailure::CapabilityUnavailable(
                "oracle region filter".to_string(),
            ));
        }
        let query = lexical_terms(&request.question.text);
        let best = request
            .evidence
            .iter()
            .filter_map(|item| item.text_projection.as_ref().map(|text| (item, text)))
            .map(|(item, text)| {
                let terms = lexical_terms(text);
                let overlap = query.intersection(&terms).count();
                let union = query.union(&terms).count().max(1);
                (overlap, union, item.evidence_id.as_str(), item, text)
            })
            .max_by(|left, right| {
                ((left.0 as u128) * (right.1 as u128))
                    .cmp(&((right.0 as u128) * (left.1 as u128)))
                    .then_with(|| right.2.cmp(left.2))
            })
            .ok_or_else(|| AdapterFailure::Failed("no text projections available".to_string()))?;
        Ok(RecallResponse {
            answer: best.4.clone(),
            retrieved: vec![RetrievedEvidence {
                evidence_id: best.3.evidence_id.clone(),
                lane: RecallLane::TextProjection,
                region: best.3.region.clone(),
                rendered_text: Some(best.4.clone()),
                source_media: Some(best.3.source_media.clone()),
            }],
            execution: AdapterExecutionProof {
                effective_lane: RecallLane::TextProjection,
                effective_reader_mode: ReaderMode::TextProjection,
                text_branch_candidates: request
                    .evidence
                    .iter()
                    .filter(|item| item.text_projection.is_some())
                    .count(),
                native_branch_candidates: 0,
                collapsed_duplicates: 0,
                oracle_regions_applied: Vec::new(),
                reader_media_parts: 0,
            },
            provider_calls: 0,
            cost_micro_usd: 0,
        })
    }
}

fn lexical_terms(value: &str) -> BTreeSet<String> {
    value
        .split(|character: char| !character.is_alphanumeric())
        .filter(|term| !term.is_empty())
        .map(str::to_lowercase)
        .collect()
}

fn contains_word_bounded(haystack: &str, needle: &str) -> bool {
    haystack.match_indices(needle).any(|(index, _)| {
        let before = haystack[..index].chars().next_back();
        let after = haystack[index + needle.len()..].chars().next();
        !before.is_some_and(char::is_alphanumeric) && !after.is_some_and(char::is_alphanumeric)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn media(id: &str, media_type: &str) -> MediaRef {
        MediaRef {
            asset_id: id.to_string(),
            locator: format!("assets/{id}"),
            sha256: sha256(id.as_bytes()),
            media_type: media_type.to_string(),
        }
    }

    fn fixture() -> MultimodalFixtureSet {
        let screenshot = EvidenceRegion::Screenshot {
            trajectory_id: "trajectory-1".to_string(),
            state_index: 3,
            locator: "screenshots/trajectory-1/3.png".to_string(),
            bounds: Some(NormalizedBounds {
                x: 0.5,
                y: 0.1,
                width: 0.4,
                height: 0.2,
            }),
        };
        let page = EvidenceRegion::Page {
            document_id: "invoice-1".to_string(),
            page_number: 2,
            bounds: None,
        };
        let sheet = EvidenceRegion::Sheet {
            workbook_id: "forecast-1".to_string(),
            sheet_name: "Q2".to_string(),
            range: "B2:C4".to_string(),
        };
        let cell = EvidenceRegion::Cell {
            workbook_id: "forecast-1".to_string(),
            sheet_name: "Q2".to_string(),
            cell: "C4".to_string(),
        };
        MultimodalFixtureSet {
            schema: FIXTURE_SCHEMA.to_string(),
            fixture_set_id: "heldout-multimodal-v1".to_string(),
            split: FixtureSplit::HeldOut,
            source: DatasetProvenance {
                dataset_id: "synthetic-heldout".to_string(),
                version: "1".to_string(),
                source_digest: sha256(b"synthetic-heldout-v1"),
                source_uri: None,
                official_equivalent: false,
                leaderboard_eligible: false,
            },
            cases: vec![
                MultimodalCase {
                    case_id: "lmev2-image-1".to_string(),
                    origin: FixtureOrigin::LongMemEvalV2,
                    corpus_id: "web".to_string(),
                    question: ContentSpec {
                        text: "Which severity appears in the dashboard?".to_string(),
                        media: Vec::new(),
                    },
                    gold: GoldAnswer {
                        value: "critical".to_string(),
                        scoring: ScoringRule::ContainsAll {
                            case_sensitive: false,
                        },
                    },
                    media_dependent: true,
                    media_dependence_review: Some(MediaDependenceReview {
                        reviewed_by: "fixture-author".to_string(),
                        rationale: "The severity is rendered in the screenshot.".to_string(),
                    }),
                    oracle_lane: RecallLane::Native,
                    oracle_evidence: vec![screenshot.clone()],
                    evidence: vec![EvidenceItem {
                        evidence_id: "screenshot-evidence".to_string(),
                        source_media: media("incident.png", "image/png"),
                        region: screenshot,
                        text_projection: Some("dashboard incident severity critical".to_string()),
                    }],
                },
                MultimodalCase {
                    case_id: "pdf-page-1".to_string(),
                    origin: FixtureOrigin::HeldOutPdf,
                    corpus_id: "documents".to_string(),
                    question: ContentSpec {
                        text: "What is the invoice total?".to_string(),
                        media: Vec::new(),
                    },
                    gold: GoldAnswer {
                        value: "$1,284.50".to_string(),
                        scoring: ScoringRule::ContainsAll {
                            case_sensitive: false,
                        },
                    },
                    media_dependent: true,
                    media_dependence_review: Some(MediaDependenceReview {
                        reviewed_by: "fixture-author".to_string(),
                        rationale: "The answer is in a PDF page region.".to_string(),
                    }),
                    oracle_lane: RecallLane::HybridCollapsed,
                    oracle_evidence: vec![page.clone()],
                    evidence: vec![EvidenceItem {
                        evidence_id: "pdf-evidence".to_string(),
                        source_media: media("invoice.pdf", "application/pdf"),
                        region: page,
                        text_projection: Some("invoice total $1,284.50".to_string()),
                    }],
                },
                MultimodalCase {
                    case_id: "spreadsheet-cell-1".to_string(),
                    origin: FixtureOrigin::HeldOutSpreadsheet,
                    corpus_id: "workbooks".to_string(),
                    question: ContentSpec {
                        text: "What is Q2 net revenue?".to_string(),
                        media: Vec::new(),
                    },
                    gold: GoldAnswer {
                        value: "91300".to_string(),
                        scoring: ScoringRule::ContainsAll {
                            case_sensitive: false,
                        },
                    },
                    media_dependent: true,
                    media_dependence_review: Some(MediaDependenceReview {
                        reviewed_by: "fixture-author".to_string(),
                        rationale: "The answer is in a workbook cell.".to_string(),
                    }),
                    oracle_lane: RecallLane::HybridCollapsed,
                    oracle_evidence: vec![sheet.clone(), cell.clone()],
                    evidence: vec![
                        EvidenceItem {
                            evidence_id: "sheet-evidence".to_string(),
                            source_media: media(
                                "forecast.xlsx",
                                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                            ),
                            region: sheet,
                            text_projection: Some("Q2 net revenue 91300 USD".to_string()),
                        },
                        EvidenceItem {
                            evidence_id: "cell-evidence".to_string(),
                            source_media: media(
                                "forecast.xlsx",
                                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                            ),
                            region: cell,
                            text_projection: Some("91300".to_string()),
                        },
                    ],
                },
            ],
        }
    }

    fn plan(arm: ExperimentArm) -> MultimodalRunPlan {
        MultimodalRunPlan {
            arm,
            budget: ExecutionBudget::offline(),
            hypothesis: PreregisteredHypothesis {
                statement:
                    "The requested lane changes retrieved evidence on media-dependent cases."
                        .to_string(),
                mechanism: "Native media preserves structure omitted by text projection."
                    .to_string(),
                decision_gate:
                    "Only continue above the projection control when fired proofs differ."
                        .to_string(),
            },
            started_at: "2026-08-22T00:00:00Z".to_string(),
        }
    }

    #[derive(Default)]
    struct ProofAdapter;

    impl MultimodalRecallAdapter for ProofAdapter {
        fn descriptor(&self) -> AdapterDescriptor {
            AdapterDescriptor {
                adapter_id: "proof-adapter".to_string(),
                adapter_version: "1".to_string(),
                capabilities: MultimodalCapabilities {
                    text_projection_recall: true,
                    native_image_recall: true,
                    native_document_recall: true,
                    hybrid_collapse: true,
                    text_projection_reader: true,
                    source_blob_reader: true,
                    oracle_region_filter: true,
                },
            }
        }

        fn recall(&mut self, request: &RecallRequest) -> Result<RecallResponse, AdapterFailure> {
            let selected: Vec<_> = if request.oracle_regions.is_empty() {
                request.evidence.iter().collect()
            } else {
                request
                    .evidence
                    .iter()
                    .filter(|item| request.oracle_regions.contains(&item.region))
                    .collect()
            };
            let answer = selected
                .first()
                .and_then(|item| item.text_projection.clone())
                .ok_or_else(|| {
                    AdapterFailure::Failed("proof fixture lacks projection".to_string())
                })?;
            let text_candidates = request
                .evidence
                .iter()
                .filter(|item| item.text_projection.is_some())
                .count();
            let native_candidates = request.evidence.len();
            Ok(RecallResponse {
                answer,
                retrieved: selected
                    .into_iter()
                    .map(|item| RetrievedEvidence {
                        evidence_id: item.evidence_id.clone(),
                        lane: request.lane,
                        region: item.region.clone(),
                        rendered_text: item.text_projection.clone(),
                        source_media: Some(item.source_media.clone()),
                    })
                    .collect(),
                execution: AdapterExecutionProof {
                    effective_lane: request.lane,
                    effective_reader_mode: request.reader_mode,
                    text_branch_candidates: match request.lane {
                        RecallLane::Native => 0,
                        _ => text_candidates,
                    },
                    native_branch_candidates: match request.lane {
                        RecallLane::TextProjection => 0,
                        _ => native_candidates,
                    },
                    collapsed_duplicates: usize::from(request.lane == RecallLane::HybridCollapsed)
                        * text_candidates,
                    oracle_regions_applied: request.oracle_regions.clone(),
                    reader_media_parts: if request.reader_mode == ReaderMode::SourceBlob {
                        native_candidates
                    } else {
                        0
                    },
                },
                provider_calls: 0,
                cost_micro_usd: 0,
            })
        }
    }

    #[test]
    fn fixture_covers_screenshot_page_sheet_and_cell_regions() {
        let fixture = fixture();
        validate_fixture(&fixture).unwrap();
        let kinds: BTreeSet<_> = fixture
            .cases
            .iter()
            .flat_map(|case| &case.oracle_evidence)
            .map(|region| match region {
                EvidenceRegion::Screenshot { .. } => "screenshot",
                EvidenceRegion::Page { .. } => "page",
                EvidenceRegion::Sheet { .. } => "sheet",
                EvidenceRegion::Cell { .. } => "cell",
            })
            .collect();
        assert_eq!(
            kinds,
            BTreeSet::from(["cell", "page", "screenshot", "sheet"])
        );
    }

    #[test]
    fn checked_in_heldout_fixture_is_schema_valid_and_non_rankable() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/multimodal/v1/heldout-recall.json");
        let fixture = load_fixture_file(&path).unwrap();
        assert_eq!(fixture.split, FixtureSplit::HeldOut);
        assert_eq!(fixture.cases.len(), 3);
        assert!(!fixture.source.official_equivalent);
        assert!(!fixture.source.leaderboard_eligible);
    }

    #[test]
    fn text_projection_control_is_offline_deterministic_and_proves_it_fired() {
        let fixture = fixture();
        let mut first_adapter = TextProjectionBaseline::default();
        let first = run_experiment(
            &fixture,
            &plan(ExperimentArm::text_control()),
            &mut first_adapter,
            &DeterministicScorer,
        )
        .unwrap();
        let mut second_adapter = TextProjectionBaseline::default();
        let second = run_experiment(
            &fixture,
            &plan(ExperimentArm::text_control()),
            &mut second_adapter,
            &DeterministicScorer,
        )
        .unwrap();
        assert_eq!(first.run_id, second.run_id);
        assert_eq!(
            first.metrics,
            MultimodalMetrics {
                correct: 3,
                total: 3,
                media_dependent_correct: 3,
                media_dependent_total: 3
            }
        );
        assert_eq!(first.provenance.provider_calls, 0);
        assert_eq!(first.provenance.cost_micro_usd, 0);
        assert!(!first.provenance.oracle_gold);
        assert!(
            first
                .cases
                .iter()
                .all(|case| case.fired_proof.text_branch_candidates > 0
                    && case.fired_proof.native_branch_candidates == 0)
        );
    }

    #[test]
    fn native_and_hybrid_arms_fail_explicitly_before_adapter_calls() {
        for arm in [
            ExperimentArm::native(),
            ExperimentArm::hybrid(),
            ExperimentArm::reader_modality(ReaderMode::SourceBlob),
        ] {
            let mut adapter = TextProjectionBaseline::default();
            let error = run_experiment(&fixture(), &plan(arm), &mut adapter, &DeterministicScorer)
                .unwrap_err()
                .to_string();
            assert!(error.contains("adapter capability gap"), "{error}");
        }
    }

    #[test]
    fn all_a_through_e_cells_emit_distinct_fired_proofs_offline() {
        let fixture = fixture();
        let arms = [
            ExperimentArm::text_control(),
            ExperimentArm::native(),
            ExperimentArm::hybrid(),
            ExperimentArm::oracle(),
            ExperimentArm::reader_modality(ReaderMode::TextProjection),
            ExperimentArm::reader_modality(ReaderMode::SourceBlob),
        ];
        let mut proof_fingerprints = BTreeSet::new();
        for arm in arms {
            let mut adapter = ProofAdapter;
            let result = run_experiment(
                &fixture,
                &plan(arm.clone()),
                &mut adapter,
                &DeterministicScorer,
            )
            .unwrap();
            assert_eq!(result.provenance.provider_calls, 0);
            assert_eq!(result.provenance.cost_micro_usd, 0);
            if arm.cell == ExperimentCell::DOracleLane {
                assert!(result.provenance.oracle_gold);
                assert!(!result.provenance.leaderboard_eligible);
                assert!(
                    result
                        .cases
                        .iter()
                        .all(|case| case.fired_proof.oracle_region_count > 0)
                );
            }
            if arm.cell == ExperimentCell::EReaderModality
                && arm.reader_mode == ReaderMode::SourceBlob
            {
                assert!(
                    result
                        .cases
                        .iter()
                        .all(|case| case.fired_proof.reader_media_parts > 0)
                );
            }
            proof_fingerprints.insert(stable_json_hash(&result.cases).unwrap());
        }
        assert_eq!(
            proof_fingerprints.len(),
            5,
            "cell E's text-reader control must match cell C; every other arm must fire differently"
        );
    }

    #[test]
    fn oracle_is_always_marked_non_rankable() {
        let mut adapter = TextProjectionBaseline::default();
        adapter.descriptor.capabilities.oracle_region_filter = true;
        let error = run_experiment(
            &fixture(),
            &plan(ExperimentArm::oracle()),
            &mut adapter,
            &DeterministicScorer,
        )
        .unwrap_err();
        assert!(error.to_string().contains("native_"));
    }

    #[test]
    fn invalid_region_and_unsafe_locator_fail_closed() {
        let mut unsafe_fixture = fixture();
        unsafe_fixture.cases[0].evidence[0].source_media.locator = "../secret.png".to_string();
        assert!(
            validate_fixture(&unsafe_fixture)
                .unwrap_err()
                .to_string()
                .contains("unsafe path")
        );
        let mut bad_region_fixture = fixture();
        bad_region_fixture.cases[2].oracle_evidence[1] = EvidenceRegion::Cell {
            workbook_id: "forecast-1".to_string(),
            sheet_name: "Q2".to_string(),
            cell: "c04".to_string(),
        };
        assert!(validate_fixture(&bad_region_fixture).is_err());
    }

    #[test]
    fn external_scorers_never_silently_fall_back() {
        let error = DeterministicScorer
            .score(
                &ScoringRule::External {
                    evaluator: "official-v2".to_string(),
                },
                "gold",
                "answer",
            )
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("does not support external evaluator")
        );
    }

    #[test]
    fn longmemeval_v2_annotation_loader_hashes_media_and_checks_haystack_linkage() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("haystacks")).unwrap();
        fs::create_dir_all(root.path().join("screenshots/t1")).unwrap();
        fs::create_dir_all(root.path().join("question_screenshots")).unwrap();
        fs::write(
            root.path().join("screenshots/t1/0.png"),
            b"\x89PNG fixture screenshot",
        )
        .unwrap();
        fs::write(
            root.path().join("question_screenshots/q1.png"),
            b"\x89PNG fixture question",
        )
        .unwrap();
        fs::write(
            root.path().join("questions.jsonl"),
            serde_json::json!({
                "id": "q1", "domain": "web", "environment": "webarena",
                "question_type": "visual", "question": "Which badge?",
                "image": "question_screenshots/q1.png", "answer": "critical",
                "eval_function": "norm_phrase_set_match|lower=true"
            })
            .to_string()
                + "\n",
        )
        .unwrap();
        fs::write(
            root.path().join("trajectories.jsonl"),
            serde_json::json!({
                "id": "t1", "domain": "web", "environment": "webarena", "goal": "inspect",
                "outcome": "success", "start_url": "https://example.test",
                "states": [{"step": 1, "state_index": 0, "url": "https://example.test",
                    "action": null, "thought": null, "accessibility_tree": "incident card",
                    "screenshot": "screenshots/t1/0.png"}]
            })
            .to_string()
                + "\n",
        )
        .unwrap();
        fs::write(
            root.path().join("haystacks/lme_v2_small.json"),
            serde_json::to_vec(&serde_json::json!({"q1": ["t1"]})).unwrap(),
        )
        .unwrap();
        let annotation_path = root.path().join("annotations.json");
        fs::write(
            &annotation_path,
            serde_json::to_vec(&serde_json::json!({
                "schema": ANNOTATION_SCHEMA,
                "fixture_set_id": "lmev2-image-subset-test",
                "dataset_version": "test",
                "haystack_file": "lme_v2_small.json",
                "annotations": [{
                    "question_id": "q1", "image_dependent": true,
                    "reviewed_by": "fixture-reviewer",
                    "rationale": "The badge color and label exist only in pixels.",
                    "oracle_lane": "native",
                    "evidence": [{"trajectory_id": "t1", "state_index": 0}]
                }]
            }))
            .unwrap(),
        )
        .unwrap();

        let fixture = load_longmemeval_v2_image_subset(root.path(), &annotation_path).unwrap();
        assert_eq!(fixture.cases.len(), 1);
        assert_eq!(fixture.cases[0].question.media.len(), 1);
        assert_eq!(
            fixture.cases[0].evidence[0].source_media.sha256,
            sha256(b"\x89PNG fixture screenshot")
        );
        assert!(matches!(
            fixture.cases[0].gold.scoring,
            ScoringRule::External { .. }
        ));
        assert!(!fixture.source.official_equivalent);
        assert!(!fixture.source.leaderboard_eligible);

        fs::write(
            root.path().join("haystacks/lme_v2_small.json"),
            serde_json::to_vec(&serde_json::json!({"q1": ["other"]})).unwrap(),
        )
        .unwrap();
        let error = load_longmemeval_v2_image_subset(root.path(), &annotation_path).unwrap_err();
        assert!(error.to_string().contains("outside its haystack"));
    }
}
