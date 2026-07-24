<!-- mission-control:managed version=2.2.0 -->
# Mission Control reporting

This repository keeps authority over its code, reviews, and execution. The
durable task authority is declared in `.mission-control/project.json`:

- `legacy` means the declared repository ledger is a migration source and may
  still be synchronized;
- `provider` means the linked GitHub/local task provider is canonical and the
  repository Markdown ledger is read-only history or a generated index.

Mission Control receives only code-blind operating metadata and adds agent,
run, question, evidence, analysis, nudge, and automation overlays.

## Required agent behavior

Report meaningful boundaries when `.mission-control/bridge.mjs` is present:

```sh
node .mission-control/bridge.mjs work start --task task:149 --summary "Implementing the bounded work item" --next "Run the relevant verification"
node .mission-control/bridge.mjs work wait --task task:149 --summary "Waiting on a human decision" --next "Review the open question"
node .mission-control/bridge.mjs work finish --task task:149 --summary "Work item verified and handed off"
node .mission-control/bridge.mjs pulse failed --summary "Verification failed" --next "Inspect the recorded failure"
```

Before provider cutover, reconcile any operator-approved legacy changes only
when the manifest grants `tasks:write`:

```sh
node .mission-control/bridge.mjs tasks reconcile
```

This command compares the declared task-source revision before writing. A
conflict writes nothing and must be resolved in the native project. It never
commits or pushes repository changes. Read-only projects return a skipped
receipt.

Use `--progress 0..100` only when the native workflow has an evidence-backed
progress measure. Do not invent a percentage.

Before provider cutover, synchronize the declared migration ledger after it
changes:

```sh
node .mission-control/bridge.mjs tasks sync
```

## Payload boundary

Allowed: current state, short code-blind summary, next action, native task
identifiers, priorities, dependencies, and source links.

Forbidden: source code, diffs, prompts, transcripts, credentials, tokens,
customer data, raw traces, or unfiltered artifact bodies.

Reporting is non-blocking. If Mission Control is unavailable, continue the
native workflow and mention the reporting failure in the normal handoff. The
configured task provider remains authoritative.

The bridge preflights every supplied task identifier. It may safely adopt one
canonical provider task when the configured provider can prove its identity.
If it cannot, the bridge reports `not_published` and emits no unlinked task
pulse; continue native work and repair the provider/project route separately.

The optional Git hook emits a checkpoint after a successful commit and only
synchronizes legacy migration metadata before provider cutover. A commit is
never reported as task completion.
