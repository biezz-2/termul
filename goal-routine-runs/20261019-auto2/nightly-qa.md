# Pass 3 — Nightly QA — run 20261019-auto2 (2026-10-19)

## Lever

`bun scripts/routine/nightly-audit.mjs --json` → `goal-routine-runs/20261019-auto2/nightly-audit.json`

**Verdict: `pass`** — 8/8 checks green (manifest, binary-download, server-boot, ws-authenticate, ws-ping, ws-store-roundtrip, ws-list-persisted-sessions, terminal-roundtrip).

- Nightly: `0.5.0-nightly.20261019.a130218` (linux-x86_64-server), pub 2026-10-18T17:31:10Z, sha256 `15eb380b…2afdf2`, 78,215,528 bytes.
- Pinned commit `a130218` = 2026-10-18T14:00:01Z (`feat(theme): match Termul syntax colors… (#963)`).

## Solved-today check

Jakarta date of the nightly's `pub_date` = **2026-10-19** (window starts 2026-10-18T17:00Z). Closed in window: #1393, #1392, #1391, #1363. All four closed after the pinned commit (verified via `compare/a130218...dev`: PRs #1398/#1395/#1394/#1366 all merged 22:45–23:16Z, i.e. after the pin), so each was checked by hand on the booted server (same verified binary, scratch state dir, port 19617).

| Issue | Fix PR (merged) | On-server result |
|---|---|---|
| #1391 CPU pinning per idle agent | #1398 | **Reproduced live on the pinned nightly** — this is the expected pre-fix state, not a new regression (see below). Not re-filed; #1391 stays closed with the fix in dev, first nightly carrying it will be tomorrow's. |
| #1363 web file writes no timeout | #1366 | `not-verifiable-on-server`: fix is renderer-only (`webServerFilesystem` 30 s `AbortController` in `web-server-api.ts`/`editor-store.ts`; PR touches 6 renderer files, zero Rust). The blackholed-save symptom lives in the client, not the server binary. |
| #1392 account switch to signed-out blocked | #1395 | `not-verifiable-on-server`: chat launcher UI behavior; no server-surface path. |
| #1393 account switcher in new-chat empty state | #1394 | `not-verifiable-on-server`: chat launcher UI; no server-surface path. |

### #1391 live verification detail (evidence: `probe-1391-cpu.out`)

Probe: `goal-routine-runs/20261019-auto2/probe-1391-cpu.mjs` — connects `/ws` (audit-script protocol), `spawn_agent` (opencode, `/root/.local/state/termul/acp-registry-binaries/opencode/opencode acp`), `create_session`, then samples `/proc/<pid>/stat` utime+stime over 5 s windows.

- Clean baseline (no agents): **0 ticks / 0%**
- One idle agent (post `create_session`, greeting settled, no prompt): **502 ticks/5 s = 100.4%** — exactly one core of 8, steady across two windows.
- Earlier run with two agents: 200.2% — one core each.
- `kill_agent` both → CPU returns to **0 ticks/5 s** within seconds; zero `opencode acp` processes remain.

Conclusion: the pinned nightly still busy-loops one core per live idle ACP agent (thread 599475/600531 style: server-side reader thread). PR #1398 (tokio stdio read, "idle acp-agent thread stays parked") merged at 2026-10-18T22:45:06Z, 8.5 h after the pin — the fix is in dev but not in this nightly. **No new issue: known, tracked, closed with fix in flight.** Next run should re-check on the post-#1398 nightly and prune the inventory entry if it passes.

## Regression hunt (known-failure inventory)

| Inventory entry | Result on nightly 20261019.a130218 |
|---|---|
| `#1391` idle CPU pinning | Still present on this pinned nightly (expected — fix merged after pin). Not pruned; re-check next run. |
| Greeting-turn cost at `create_session` (opencode, known agent-side) | Observed again in Pass 4 probe runs (`create_session` took ~20 s+ with greeting frames). Known, not a Termul bug, not filed. |

No check that was passing now fails: the audit lever is 8/8 green and the CPU behavior is the already-tracked pre-fix state. **No regression filed.**

## Deeper UI QA

Not run this pass: the nightly's desktop web client (`dist-web`) is not shipped in the server binary (`dist-web/index.html not found` warning at boot; would need `bun run build:web` from a source tree, which is not the shipped artifact). No in-server UI surface to drive beyond the wire probes above. Chat-agent surface covered by Pass 4.

## New issues filed

None. All four closed-issue verifications either passed the expected pre-fix state (tracked in #1391), or were `not-verifiable-on-server` with no new symptom observed. Duplicate search (open+closed, `gh search issues`) for idle-CPU, fs-write-timeout and account-launcher symptoms found no new matches to comment on.
