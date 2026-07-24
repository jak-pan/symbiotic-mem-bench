---
name: mission-control
description: Manage canonical project tasks and report code-blind agent progress through a repository's Mission Control MCP or local bridge. Use when inspecting task priority or blockers, starting or pausing agent work, updating task lifecycle, handing work off, synchronizing provider-backed work, or working in a repository that contains .mission-control/project.json.
---
<!-- mission-control:managed version=2.0.0 -->

# Mission Control

Read `AGENTS.md`, `.mission-control/AGENT.md`, and `.mission-control/project.json` before changing project state. Treat the configured task provider as authoritative; never edit a legacy Markdown ledger when `taskAuthority` is `provider`.

## Operate the work item

1. Inspect `project_get`, `tasks_list`, `task_get`, and `task_graph` through the project `mission-control` MCP server.
2. Bind meaningful work explicitly with `work_start`. Include the canonical task ID, a short code-blind summary, and the next verification step.
3. Use `work_checkpoint` for meaningful verified progress, `work_wait` for a real dependency or human decision, and `work_fail` for failed execution. Use `work_finish` only after verification and handoff. It reports an execution boundary; it never marks the provider task done.
4. Use `task_transition` with a concrete reason when the canonical task lifecycle should change. Let Mission Control perform revision checks and provider write-back.
5. Report failures with the local bridge if MCP is unavailable. Reporting failure never blocks native project work.

Equivalent local commands:

```sh
node .mission-control/bridge.mjs work start --task TASK_ID --summary "Bounded work started" --next "Run verification"
node .mission-control/bridge.mjs work checkpoint --task TASK_ID --summary "Implementation checkpoint verified" --next "Run integration tests"
node .mission-control/bridge.mjs work wait --task TASK_ID --summary "Waiting on a decision" --next "Review the open question"
node .mission-control/bridge.mjs work finish --task TASK_ID --summary "Verified and handed off"
node .mission-control/bridge.mjs work fail --task TASK_ID --summary "Verification failed" --next "Inspect the failure"
```

Run `tasks_sync` or `tasks_reconcile` only for the declared migration path. Provider-authoritative projects return a safe no-op.

Keep every report code-blind. Never send source, diffs, prompts, transcripts, credentials, customer data, raw traces, or artifact bodies.
