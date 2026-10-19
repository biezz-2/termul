# Termul routine run — 20261019-auto2 (2026-10-19)

Unattended run per `.claude/skills/termul-routines/SKILL.md`. Passes executed: 1 (triage & housekeeping), 3 (nightly QA), 4 (chat-agent QA). Pass 2 (development) not run — unattended; AGENTS.md requires human diff approval before any PR. No issues closed, no PRs opened/merged/force-pushed, no `to-agent` label added, no new issues filed (nothing new found beyond tracked items).

## Pass 1 — Triage & housekeeping (report: `triage.md`)

39 open issues fetched; 30 covered (cap), 9 deferred (#1105, #962, #823, #822, #821, #795, #447, #362, #357). 23 label mutations applied; no comments (no issue met a comment state); nothing closed/locked/pinned/retitled.

| Issue | Mutation |
|---|---|
| #1431 | +bug +priority-high +win |
| #1429 | +ai |
| #1428 | +ai |
| #1427 | +ai +win |
| #1426 | +ai |
| #1425 | +ai |
| #1424 | +ai |
| #1423 | +ai |
| #1422 | +ai |
| #1421 | +ai +win |
| #1420 | +ai |
| #1413 | +ai +linux +mac |
| #1412 | +ai +win +linux |
| #1411 | +ai +win +linux |
| #1409 | +ai +win +linux |
| #1408 | +ai +win +linux |
| #1407 | +win +linux |
| #1372 | +ai |
| #1370 | +ai |
| #1365 | +ai |
| #1364 | +ai |
| #1300 | +win |

Feature gate held: no `to-agent`-labeled feature issue exists; all enhancement issues (#1430, #1349, #1156, and the deferred set) stay undeveloped.

Pass 2 disposition recorded for a supervised run: #1413 is the leading candidate (Linux-reproducible, clearly scoped, no open fix PR — #1418 covers canvas-stack findings only, not #1413).

## Pass 3 — Nightly QA (report: `nightly-qa.md`)

- Lever `bun scripts/routine/nightly-audit.mjs --json` → **verdict `pass`**, 8/8 checks, nightly `0.5.0-nightly.20261019.a130218` (sha `15eb380b…`), pinned commit 2026-10-18T14:00Z.
- Solved-today (Jakarta 2026-10-19 window, 4 closures): #1391 verified live on the booted server — idle opencode agent pins exactly one core (100.4%, 0% clean baseline, 0% after kill); expected pre-fix state since #1398 merged 8.5 h after the pin; **no new issue**. #1363, #1392, #1393 → `not-verifiable-on-server` (renderer-only fixes: AbortController timeout, chat launcher UI).
- Regression hunt: no check that was passing now fails; known inventory unchanged (#1391 re-check next nightly; greeting-turn cost re-observed, known agent-side).
- Deeper UI QA: not run — nightly server binary ships no `dist-web` (boot log: "dist-web/index.html not found"); no server-side UI surface beyond wire probes.
- New issues filed: **none** (no new symptoms; duplicate searches empty).

## Pass 4 — Chat-agent QA (report: `chat-agent-qa.md`, frame log `chat-agent-qa.jsonl`)

Real opencode agent over the nightly server's `/ws` relay, model `sepintu/moonshot/kimi-k3`. **All 11 assertions pass**: spawn_agent → create_session (3.1 s) → subscribe → send_prompt (command_id = turnId, "Reply with exactly OK and nothing else.") → 13 message_chunk frames → agent-role text exactly `"OK"` → prompt_complete → end_turn reply → close_session → kill_agent. Full teardown verified: no orphan opencode processes, server CPU 0, scratch state removed.

Findings: none new. Both known classes observed and not filed (greeting-turn cost — agent-side; idle CPU — #1391, fix in dev).

## Artifacts

- `triage.md` — Pass 1 classification, one row per issue with body evidence
- `open-issues.json` — fetched issue set
- `nightly-audit.json` / `nightly-audit-keep.json` — lever evidence (verdict field: pass)
- `probe-1391-cpu.mjs` + `probe-1391-cpu.out` — #1391 live verification
- `probe-chat-agent.mjs` + `chat-agent-qa.jsonl` (48 frames) + `chat-agent-qa.md` — Pass 4 evidence
- `nightly-qa.md` — Pass 3 report

All re-runnable: the audit JSON by anyone via `bun scripts/routine/nightly-audit.mjs --json`; the probes against a freshly booted nightly server (ports/flags recorded in the probe headers).
