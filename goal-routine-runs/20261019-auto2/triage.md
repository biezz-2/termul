# Pass 1 triage — run 20261019-auto2 (2026-10-19)

Fetched once: `gh issue list -R gnoviawan/termul --state open --limit 200 --json …` → 39 open issues.
**Cap: 30 issues per run.** Covered the 30 newest; deferred 9: #1105, #962, #823, #822, #821, #795, #447, #362, #357 (next run's head of queue).

No `needs-info` (every bug in the set carries repro steps and/or logs), no `stale` (all updatedAt within 2026-10-09..19, none 60+ days), no `duplicate` (canvas wave #1420–#1429 checked against open PR #1418's 20 review findings and each other — all distinct symptoms; #1431's #730 link is a closed, different post-update issue; #1412/#1411/#1409/#1408 are four distinct surfaces from one reporter).

Area label convention: `ai` is the repo's area label for the ACP/agent/canvas surface (maintainer usage on #1430, #962, #823, #822, #821, #362, #357). Platform labels added only where the body names the platform.

| Issue | State | Evidence (body) | Action taken |
|---|---|---|---|
| #1431 | needs-labels → triaged | Title `[Bug]`; full repro (Settings→Updates, toast stuck 0%); env `OS: Windows 11`; no labels at all | Added `bug`, `priority-high` (stable-channel update path fully stuck, zero error surfaced), `win` |
| #1430 | triaged | enhancement, priority-high, ai, epic; detailed goal/problem/scope | none — feature gate: no `to-agent` label, not sanctioned for agent development |
| #1429 | needs-labels → triaged | bug, priority-medium; repro (background task wake-up calls canvas tool), log line, code refs `parent.rs` ~L466, `bridge.rs` ~L90 | Added `ai` (host-MCP/ACP surface) |
| #1428 | needs-labels → triaged | bug, priority-low; repro JSON, exported SVG excerpt, PNG/PDF control | Added `ai` |
| #1427 | needs-labels → triaged | bug, priority-high; measured repro (scale 64 + dropShadow times out, control without shadow passes), watchdog log | Added `ai`, `win` (body: "Windows 11") |
| #1426 | needs-labels → triaged | bug, priority-low; junction repro, error text, code ref `canvas_policy.rs` | Added `ai` |
| #1425 | needs-labels → triaged | bug, priority-low; repro (save_file → inspect_document reports name Untitled/path null), log line | Added `ai` (no platform: `windows[0].title` is a JSON field, not an OS) |
| #1424 | needs-labels → triaged | bug, priority-medium; deterministic 38-step batch repro, halves pass, ruled-out list | Added `ai` |
| #1423 | needs-labels → triaged | bug, priority-medium; repro (opacity 1.5 → 0.015, ok:true), related coercions | Added `ai` |
| #1422 | needs-labels → triaged | bug, priority-low; repro (unknown family/style accepted silently) | Added `ai` |
| #1421 | needs-labels → triaged | bug, priority-medium; repro string renders NOGLYPH boxes, SVG text control | Added `ai`, `win` (body: "Windows 11") |
| #1420 | needs-labels → triaged | bug, priority-high; repro (idle reaper → auto-resume drops MCP servers), termul.log evidence | Added `ai` |
| #1413 | needs-labels → triaged | bug, priority-medium; deterministic npx-cache repro script, host matrix incl. Linux aarch64 + macOS 14 M1, exit-254 log | Added `ai`, `linux`, `mac` (body names both platforms) |
| #1412 | needs-labels → triaged | bug, priority-low; screenshot; env `OS: Win 11, Linux`; thin but present repro (light mode) | Added `ai`, `win`, `linux` |
| #1411 | needs-labels → triaged | bug, priority-low; screenshot; env `OS: Win 11, Linux`; repro (light mode hover) | Added `ai`, `win`, `linux` |
| #1409 | needs-labels → triaged | bug, priority-low; screenshot; env `OS: Win 11, Linux`; repro (light mode resize) | Added `ai`, `win`, `linux` |
| #1408 | needs-labels → triaged | bug, priority-low; screenshots; env `OS: Win 11, Linux`; repro (Settings→Accounts) | Added `ai`, `win`, `linux` |
| #1407 | needs-labels → triaged | bug, priority-low; screenshots + error log; env `OS: Win 11, Linux`; repro (SSH → Enable Remote Access) | Added `win`, `linux` (SSH remote access is not the `ai` surface) |
| #1386 | triaged | bug, priority-medium; CI run IDs, panic message, cause hypotheses | none (no area label fits data_lock; priority present) |
| #1372 | needs-labels → triaged | bug, priority-medium, win; Indonesian body with steps + termul_manager_lib acp logs | Added `ai` (ACP agent activation surface) |
| #1370 | needs-labels → triaged | bug, priority-medium, win; code refs `links.rs:124`, `junction.rs`, doc line | Added `ai` (Claude ACP accounts surface) |
| #1365 | needs-labels → triaged | bug, priority-medium; code refs `command_loop.rs:809`, `reopen.rs:84`, `driver.rs`, persistence paths | Added `ai` |
| #1364 | needs-labels → triaged | bug, priority-medium; scenario (concurrent refresh + single-use tokens), code ref `mcp_oauth.rs:207` | Added `ai` |
| #1362 | triaged | bug, priority-low; code refs `use-terminal-restore.ts:923`, `lifecycle.rs:429`; trigger + impact | none (remote/terminal surface; priority present) |
| #1349 | triaged | enhancement, priority-low; ELF NEEDED evidence, fix sketch | none — feature gate: no `to-agent` |
| #1343 | triaged | bug, priority-medium; CI run IDs, 38+6 failure counts, spec groups | none (e2e suite; priority present) |
| #1330 | triaged | bug, priority-low; four concrete sub-items with code refs, split history | none (chat read-markers; priority present) |
| #1300 | needs-labels → triaged | bug, priority-medium; repro (close terminal → toast + 3–5 s); env `OS: Windows 11` | Added `win` (terminal surface, no `ai`) |
| #1299 | triaged | bug, priority-low, win; measured edge gaps, screenshot | none |
| #1156 | triaged | enhancement, priority-low; problem + proposed solution | none — feature gate: no `to-agent` |

## Pass 2 disposition

No Pass 2 development this run. Reasons:

- Unattended run: AGENTS.md requires human approval of the complete diff before any PR submission, so a Pass 2 cycle could not end in a PR tonight.
- Reproducibility on this host: the strongest dev-ready bug candidates are Windows-only surfaces (#1431, #1299, #1370, #1372) or desktop canvas UI (#1420–#1429), which cannot be exercised here (Linux host, nightly `termul-server` binary only). #1413 is Linux-reproducible and scoped (npx cache ENOENT → exit 254 surfaced as generic initialize timeout), and no open PR fixes it (checked open PR list — #1418 is canvas-stack only) — recorded as the leading Pass 2 candidate for a supervised run.
- No `to-agent` feature issue exists in the set (hard gate respected; label never added by this run).

GitHub mutations this pass (labels only, no comments — no issue met a comment state):

- #1431: +bug +priority-high +win
- #1429–#1420 (canvas/ACP wave): +ai each; #1427, #1421 additionally +win
- #1413: +ai +linux +mac
- #1412, #1411, #1409, #1408: +ai +win +linux
- #1407: +win +linux
- #1372, #1370, #1365, #1364: +ai
- #1300: +win
