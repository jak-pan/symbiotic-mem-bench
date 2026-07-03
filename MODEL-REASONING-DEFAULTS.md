# Model Reasoning / Thinking Defaults

Reference for the **answerer (reader) models** we test and run, as of **June 2026**. Reasoning
behavior varies by model *and* by platform (a provider's native API and OpenRouter often differ),
and it changes what our `MEMBENCH_ANSWER_THINKING` / `MEMBENCH_ANSWER_REASONING_EFFORT` knobs
actually do.
Every claim below is from a cited primary source (OpenRouter model pages + provider docs); anything
unverified is marked.

## How membench controls reasoning

- **`MEMBENCH_ANSWER_THINKING=on|off`** — requests thinking for the answerer role (same pattern for
  `MEMBENCH_DISTILL_THINKING`, `MEMBENCH_JUDGE_THINKING`).
- **`MEMBENCH_ANSWER_REASONING_EFFORT=low|medium|high|max`** — sets effort, passed through as the
  provider's `reasoning_effort` / OpenRouter `reasoning` parameter. If unset, `role_reasoning_effort`
  falls back to the `THINKING` value when it is `high`/`max` (`src/bin/membench.rs:4758`).
- **Caveat:** these map cleanly onto *effort-controlled* reasoning models (DeepSeek, GPT-5.5, Gemini,
  Qwen). For **opt-in / token-toggle** models (e.g. Gemma-4's `<|think|>` token), a generic effort/on
  toggle may not trigger reasoning at all — so a low score can mean "reasoning never fired," not
  "weak model." Verify per model.
- Our oracle / reader-sweep runs used `SYMEM_ANSWER_THINKING=on` throughout (the pre-rename
  spelling of today's `MEMBENCH_ANSWER_THINKING`; GPT-5.5 additionally at
  `reasoning_effort=medium`). Accuracy/cost results: `blog/03-the-reader-ceiling.md`.

## Per-model table

| Model | Reasoning model? | Reasoning default | Control mechanism | Notes |
|---|---|---|---|---|
| `deepseek-v4-flash` | Yes (hybrid) | **On** (DeepSeek API) / opt-in (OpenRouter) | toggle + effort `high`/`max` (`low`/`medium`→`high`, `xhigh`→`max`); no token budget | 284B/13B MoE, 1M ctx |
| `deepseek-v4-pro` | Yes (hybrid) | **On** (API) / opt-in (OpenRouter) | same as flash | 1.6T/49B MoE, 1M ctx |
| `openai/gpt-5.5` | Yes | **On (medium)** | `reasoning_effort` `none`/`minimal`/`low`/`medium`(def)/`high`/`xhigh`; drive to `none` ≈ off; OpenRouter: effort-as-%-of-max **or** `max_tokens` (mutually exclusive) | reasoning tier "highest"; released Apr 2026 |
| `google/gemini-3.5-flash` | Yes | **On (medium)** | `thinking_level` `minimal`/`low`/`medium`(def)/`high`; **no hard off** (`minimal`≈off, not guaranteed); replaced `thinkingBudget` | build `gemini-3.5-flash-20260519` |
| `google/gemma-4-26b-a4b-it` | Yes (new in Gemma 4) | **Opt-in (OFF)** | `<|think|>` system token / `enable_thinking` bool — **on/off only, no effort levels** | **open-weight, Apache 2.0**, ~26B/4B-active MoE; **verified:** our default `THINKING=on` left reasoning OFF (0/50 reasoned, scored 0.824); `REASONING_EFFORT=high` fired it (47/50) but only lifted 50Q 0.82→0.86 — genuinely weak |
| `qwen/qwen3.7-plus` | Yes (hybrid) | **On** (default) | `enable_thinking` bool + `thinking_budget` (token cap); no effort levels | API-only (closed weights); 1M ctx; $0.32/$1.28 |
| `qwen/qwen3.7-max` | Yes (hybrid) | **On** (default) | same as plus | API-only (closed weights); 1M ctx; $1.25/$3.75; 3.7 flagship |
| `qwen/qwen3.6-35b-a3b` | Yes | **On** (default) | `enable_thinking` via `chat_template_kwargs` + `thinking_budget`; Qwen3 `/think` soft-switch NOT supported | **open-weight Apache-2.0**, ~3B-active/35B MoE (256 experts); Q4≈21–22GB → runs on 64–128GB Mac / 24GB GPU (even 6GB via MoE offload); 262K ctx (→1M YaRN) |
| `nvidia/nemotron-3-ultra-550b-a55b` | Yes | **Opt-in (OFF)** | `enable_thinking` flag + `medium_effort` + `reasoning_budget` (token cap) | **open-weight** (NVIDIA, on HF; BF16/NVFP4); 550B/55B MoE — datacenter-scale (~1×H200) |
| `nvidia/nemotron-3-super-120b-a12b` | Yes | **On** (default) | `enable_thinking` flag + `low_effort` + `reasoning_budget` | **open-weight** (HF); 120B/12B MoE — big-GPU (~1×H100 FP8) |
| `minimax/minimax-m3` | Yes | **On** (`adaptive` default) | `thinking` mode `enabled`/`adaptive`/`disabled`; no token budget | **open-weight** (HF `MiniMaxAI/MiniMax-M3`); ~428B/23B MoE — datacenter-scale |

## Key contrasts (verified)

- **Effort knob vs on/off toggle.** GPT-5.5, Gemini-3.5, and DeepSeek-v4 are single models with a
  graded effort control (and can be driven toward off). Gemma-4 is a binary `<|think|>` toggle with
  no graded effort.
- **Default on vs opt-in.** GPT-5.5 and Gemini-3.5-flash reason **by default (medium)**. Gemma-4 is
  **off by default**. DeepSeek-v4 is on-by-default on its own API but opt-in via OpenRouter.
- **No hard "off" on Gemini-3.x.** `thinking_level: minimal` is the floor but does not guarantee zero
  reasoning.

## Open weights & local-runnability

- **Open-weight:** `qwen3.6-35b-a3b` (Apache-2.0), `gemma-4-26b-a4b` (Apache-2.0), `nemotron-3-ultra`/`super` (NVIDIA, on HF), `minimax-m3` (HF).
- **Closed / API-only:** `deepseek-v4-flash`/`pro`, `gpt-5.5`, `gemini-3.5-flash`, `qwen3.7-plus`/`max`.
- **Consumer/standard-hardware runnable:** only `qwen3.6-35b-a3b` (~3B active, ~21GB Q4 → 64–128GB Mac or 24GB GPU) and `gemma-4-26b-a4b` (~4B active). The other open models are datacenter-scale (active params: nemotron-super 12B, ultra 55B, minimax-m3 23B).
- **Upshot:** `qwen3.6-35b-a3b` is the standout — ceiling-tier accuracy (90.6% in our oracle sweep) *and* self-hostable on a workstation. `gemma-4` is also consumer-runnable but far weaker — with thinking forced on (verified firing, 47/50 calls) it still only reaches ~0.86 on 50Q.

## Sources

- **DeepSeek:** openrouter.ai/deepseek/deepseek-v4-{flash,pro} · api-docs.deepseek.com/guides/thinking_mode · huggingface.co/deepseek-ai/DeepSeek-V4-Pro
- **GPT-5.5:** developers.openai.com/api/docs/models/gpt-5.5 · developers.openai.com/api/docs/guides/reasoning · openrouter.ai/openai/gpt-5.5
- **Gemini-3.5-flash:** ai.google.dev/gemini-api/docs/thinking · ai.google.dev/gemini-api/docs/whats-new-gemini-3.5 · openrouter.ai/google/gemini-3.5-flash
- **Gemma-4:** ai.google.dev/gemma/docs/core/model_card_4 · huggingface.co/google/gemma-4-26B-A4B-it
- **Qwen:** openrouter.ai/qwen/qwen3.7-{plus,max} · alibabacloud.com/help/en/model-studio/deep-thinking · huggingface.co/Qwen/Qwen3.6-35B-A3B · huggingface.co/bartowski/Qwen_Qwen3.6-35B-A3B-GGUF
- **Nemotron:** openrouter.ai/nvidia/nemotron-3-ultra-550b-a55b · docs.api.nvidia.com/nim/reference/nvidia-nemotron-3-{ultra,super} · huggingface.co/collections/nvidia/nvidia-nemotron-v3
- **MiniMax-M3:** openrouter.ai/minimax/minimax-m3 · huggingface.co/MiniMaxAI/MiniMax-M3 · platform.minimax.io/docs

*Last updated: 2026-06-29. All 10 reader configs researched against primary sources (cited per group).*
