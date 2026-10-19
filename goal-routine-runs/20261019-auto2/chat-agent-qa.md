# Pass 4 — Chat-agent QA (real opencode agent) — run 20261019-auto2 (2026-10-19)

## Setup

- Nightly `termul-server` `0.5.0-nightly.20261019.a130218` (same verified binary sha `15eb380b…`, booted on scratch port 19617 with a scratch state dir; the audit script's `--keep-server` run lost its server to `timeout`'s parent kill, so the binary was booted directly with identical flags).
- Agent: real opencode, `/root/.local/state/termul/acp-registry-binaries/opencode/opencode` + `acp` arg, spawned through `spawn_agent` over the `/ws` relay — model `sepintu/moonshot/kimi-k3` via the host-configured provider (free tier).
- Probe: `goal-routine-runs/20261019-auto2/probe-chat-agent.mjs` — minimal RFC6455 client (same pattern as the audit script), full frame log at `goal-routine-runs/20261019-auto2/chat-agent-qa.jsonl` (48 frames: every request, reply, event batch, heartbeat).

## Wire protocol exercised (all per the skill's wire facts)

- `command_id` (snake_case) on `/ws` for ledgered requests — required for `spawn_agent`, `create_session`, `send_prompt`, `close_session`.
- `send_prompt`: `command_id` == payload `turnId`; reply resolves only after the full turn, with `payload: "end_turn"`.
- `subscribe { sessionId }` required before session events are forwarded to the connection (live-only, no replay).
- Events arrive both as single `{sid, seq, type, payload}` frames and as `{type: "events", events: […]}` batches; `message_chunk` text at `payload.content.text`, `role: "thought" | "agent"`.
- First post-connect frame: `auth_required` (logged; the client routes handshake-chunk leftovers so nothing is lost).
- Relay `ping` requests: seen as `heartbeat`-period frames; probe answers ping requests (pongTimeoutMs 75 s never tripped; connection survived the full 64.8 s longest turn).

## Run result — all assertions pass

| Step | Result |
|---|---|
| `authenticate` | ok |
| `spawn_agent` (opencode + acp) | ok — agentId `…e829`, capabilities incl. loadSession, image, http/sse MCP |
| `create_session` (throwaway cwd) | ok, 3.07 s — model list confirms `sepintu/moonshot/kimi-k3` as current model |
| `send_prompt` "Reply with exactly OK and nothing else." | ok, 5.94 s, reply `payload: "end_turn"` |
| `message_chunk` events | 13 chunks (thought + agent) |
| Agent text (role=agent only) | `"OK"` — exact match |
| `prompt_complete` | arrived (seq 10, stopReason end_turn) |
| `close_session` | ok |
| `kill_agent` | ok — zero `opencode acp` processes remain, server CPU returns to 0 ticks/5 s |

Assertion log lines are in `chat-agent-qa.jsonl` (`dir: "meta"`, `assert: …, pass: true` × 11; `pass4-summary` ends with `verdict: "pass"`).

## Findings

- None new. Both known classes were observed and NOT filed, per the skill:
  - Greeting-turn cost at `create_session` (opencode agent-side behavior, QA report 2026-09-15 P2) — visible in the earlier probe runs (create_session ~2.5–3 s including greeting frames; one long turn took 64.8 s under concurrent load, resolved correctly).
  - Idle-agent CPU pinning (#1391) — already verified live in Pass 3 on this pinned nightly; fix #1398 is in dev.
- Protocol details worth keeping (documented in the skill already, re-confirmed tonight): `send_prompt`/`close_session` payloads want `agentId` + `sessionId`; `create_session` wants `agentId` + `cwd`; `spawn_agent` wants a nested `config` object. No subscribe → no events on the wire.

Teardown: server killed, port free, scratch dirs and state removed. The run directory retains all evidence.
