# Fast lane triage

Cutoff: issues updated >2026-10-19T11:07:03Z. 3 issues in scope (#1435, #1432, #1420). Label mutations only; no comments per fast-lane policy.

| Issue | State | Evidence | Action |
|---|---|---|---|
| #1435 | triaged | Body: full repro (type `@` in agent chat composer on Linux AppImage v0.4.52 → "No files or folders match"; rg SIGSEGV 139 in logs), root-cause analysis (AppImage `LD_LIBRARY_PATH` inherited by spawned `rg`/`node`), env, logs, screenshot; had only `to-agent` | added `bug`, `priority-high`, `linux` (body: CachyOS AppImage), `ai` (agent-chat composer mention search, matches run 20261019-auto2 `+ai` policy for chat/agent-surface bugs) |
| #1432 | triaged | Was labeled by run 20261019-070510 (`bug,priority-high,win,to-agent`); already priority + area + platform labels, clear repro; no changes since | none |
| #1420 | triaged | Body: repro (idle-stop auto-resume → agent resumes with no MCP servers), expected/actual, log evidence, root cause with exact files (`use-idle-stop-auto-resume.ts`, `acp-store/slices/session.ts`, `acp/manager/mod.rs` `prepare_reopen_mcp`), suggested fix + workaround; labels `bug,priority-high,ai,to-agent` after this run's edit | added `bug` (was `priority-high,ai,to-agent` — missing type label) |

## Dev-ready scan (for issue→PR)

- No open PR claims any of the three (open PRs: #1433 #1418 #1410 #1371 #1148 #987 #927 #926 #910 #908 #894 #530 #448 #426 — none reference #1420/#1432/#1435).
- No `routine/*` branches on origin.
- Candidate pick: **#1420** — bug with repro, to-agent, has exact root-cause analysis with file/line pointers, renderer-side fix scoped by the issue itself. #1432 also viable (renderer, gated `window.open`), #1435 needs an AppImage/Linux runtime env to reproduce (packaging/sidecar env scrubbing — harder to verify here; prefer full routine pass).

#1420 picked for the issue→PR half. See `goal-routine-runs/20261019-114244/pass2-1420.md`.
