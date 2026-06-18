# Raw Granularity Decision

Status: required experiment before the next paid full-quality benchmark.

The raw recall unit is not settled. The current implementation indexes each conversation message as
one raw source turn. That is a useful atomic evidence layer, but it may be too fine-grained for
questions that depend on surrounding context, pronouns, corrections, topic drift, or a short exchange
where the useful meaning spans multiple messages.

This is a memory substrate decision. It should be tested as a durable design choice, not repaired
with a one-off prompt or post-processing pass.

## Problem

For conversations, the capture system often cannot know whether a run of messages is one coherent
conversation, several interleaved conversations, or a loose stream of updates. Any fixed grouping can
be wrong:

- one message can be too small and lose local context;
- a whole session can be too large and pollute retrieval;
- fixed windows can split across topic boundaries;
- LLM segmentation can overfit, cost tokens, or hide evidence if treated as truth.

The memory system therefore needs layered raw evidence:

1. atomic source turns as immutable provenance;
2. derived local windows for retrieval context;
3. derived episode or topic cards when enough evidence supports a coherent group;
4. distilled facts and Reweave links/tags for long-term compact recall.

Distillery may return no durable facts when a message has no memory value. That is fine. The raw
source still needs enough indexed structure to reconnect useful context later without pretending the
source boundary was known at capture time.

## Candidate Shapes

| candidate | indexed unit | expected advantage | expected risk |
| --- | --- | --- | --- |
| Atomic turn | one captured message | exact provenance, cheap updates, no invented grouping | loses context; pronouns and adjacent answers rank poorly |
| Fixed local window | sliding or block windows of 3-8 nearby turns | keeps local context and topic terms together | can add noise or duplicate context |
| Speaker-aware window | nearby turns with speaker labels and reply adjacency | preserves short exchanges and corrections | still heuristic; may miss topic changes |
| Session card | compact summary for the provided source/session | token-efficient high-level recall | assumes source boundary is coherent |
| Episode card | derived topic/event group with source-turn ids | closer to real conversation units | requires reliable segmentation and audit |
| Distilled fact | atomic source-backed claim | compact and answerable | may compress away exact wording or context |
| Reweave/tag record | derived aliases, entities, topics, and links | bridges wording and reconnects memories | must not become benchmark-specific labels |

The likely product shape is not one winner. It is a layered index where atomic turns remain the
ground truth, while windows, episode cards, facts, and tags are rebuildable retrieval views.

## Production Data Shape

Raw-derived recall records should carry:

| field | purpose |
| --- | --- |
| `unit_kind` | `turn`, `local_window`, `session_card`, `episode_card`, `fact`, or `tag_link` |
| `source_turn_ids` | exact source turns covered by the unit |
| `source_span` | ordinal range or page/span locator |
| `speaker_set` | speakers represented in the unit |
| `topic_labels` | generic derived topics and aliases |
| `search_text` | searchable text with normalized aliases, entities, slot labels, and numbers |
| `answer_text` | answer-visible evidence text; may be shorter than `search_text` |
| `event_time_range` | earliest/latest event time represented |
| `derivation` | extractor, prompt/profile, algorithm, and source hash |

Derived units must be rebuildable from raw archive receipts plus Archive Markdown. They are not the
source of truth.

## Benchmark Matrix

Run the same completed vault copies through these arms:

| arm | raw recall index | facts | derived cards/tags | answerer |
| --- | --- | --- | --- | --- |
| A | atomic turns only | current facts | none | disabled, then enabled |
| B | atomic turns + fixed local windows | current facts | none | disabled, then enabled |
| C | atomic turns + speaker-aware windows | current facts | none | disabled, then enabled |
| D | atomic turns + session cards | current facts | generic tags only | disabled, then enabled |
| E | atomic turns + episode cards | current facts | generic tags/links | disabled, then enabled |
| F | atomic turns + windows + facts + Reweave tags | current facts | full derived recall metadata | disabled, then enabled |

Use copied vaults or fresh run roots. Do not mutate frozen task-152 artifacts.

## Metrics

Quality alone is not enough. Each arm must report:

- answer score on the same fixed question set;
- relevant evidence rank for facts and raw-derived units;
- percentage of answers where relevant evidence is missing from top-k;
- answer input tokens and context item counts;
- raw unit duplication rate;
- embedding cost and index size;
- latency for ingest, index, recall, and answer;
- failure breakdown by question type.

The evidence-rank audit may use benchmark gold only for offline diagnosis. The answer path must not
receive gold answers, answer sessions, verdicts, or question ids.

## Initial Gate

Start with a fixed stratified diagnostic slice, not a full 500:

- current-state failures;
- paraphrase failures such as education or degree questions;
- count/list questions;
- unavailable/negative-evidence questions;
- preference questions;
- one-hop direct questions where atomic turns should already work.

Promotion requires paired flips and context audits, not a single aggregate score. A raw granularity
arm may advance only if it improves evidence rank or token efficiency without hiding provenance or
increasing unavailable hallucinations.

## Decision Rule

Adopt a layered raw index if the experiment shows that:

1. atomic turns remain available as source-backed evidence;
2. local windows or episode cards recover relevant evidence that atomic turns miss;
3. derived units reduce answer tokens or improve rank without materially increasing false support;
4. facts plus Reweave tags improve paraphrase and multi-hop recall generically;
5. the same design works with a production vector backend and with SQLite debug parity.

If no derived raw unit improves rank, keep atomic turns but still implement better sparse retrieval,
aliases, tags, and reranking before a full quality run.
