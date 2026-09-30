---
name: oracle-test
description: Run an oracle-gold answerer/prompt A/B on LongMemEval-500 — feeds gold evidence straight to a chosen reader model + answer prompt, grades with the official judge, then prints accuracy-by-type and canonical cost. Use whenever the user wants to test a reader model or an answer prompt in isolation (the "reader ceiling" method), or compare answerers/prompts. Each run makes paid provider calls.
---

# Oracle Reader Comparison

Shared collaboration, Git, verification, and security rules: House Rules (`~/p/house-rules/AGENTS.md`).

Use the [native oracle command](../../../skills/membench/references/membench-commands.md#oracle-reader-comparison)
for paid answerer/prompt comparisons. It feeds gold-session evidence to the reader and uses the
normal answer/judge path; recall still runs for diagnostic traces. This is an oracle diagnostic,
not a clean benchmark claim.

Supply an existing compatible zvec vault root and prompt directory explicitly. The historical
helper's scratch prompt default and SQLite store are obsolete. Model/effort overrides use
`MEMBENCH_ANSWER_{OPERATOR,MODEL,THINKING,REASONING_EFFORT}`; verify reasoning fired in the
question debug bundle (see [membench diagnostics](../../../skills/membench/SKILL.md#diagnose-a-run)).

Read accuracy and stored cost with the [score-run skill](../score-run/SKILL.md), passing a
baseline as a second run for comparison. Per-run cost depends on emitted tokens, including
reasoning, so inspect the rollup rather than comparing token prices alone.
