# Phase 11: GUI & IDE Integration

## Objective

Give Zenith a visual face. Developers should be able to see workflow status, browse logs, inspect sandbox state, and trigger runs without touching the terminal — through a web dashboard, a polished TUI, and a VSCode extension.

**Status: COMPLETE**

---

## Components Built

### 1. Run History Persistence

Every `zenith run` now writes structured history to `~/.zenith/logs/<run-id>/`:
- `summary.json` — job name, status (`running` / `success` / `failed`), start/finish timestamps, step counts
- `steps.jsonl` — one JSON object per step event (started / done / failed / cached), including captured log lines

**Key types** in `src/ui/history.rs`:
```
RunLogger       — writer side (used by runner.rs)
list_runs()     — reader: list summaries, sorted newest first
get_steps()     — reader: all step events for a run
get_run()       — reader: single run summary
```

---

### 2. Web Dashboard (`zenith ui`)

```bash
zenith ui              # starts on http://localhost:7622
zenith ui --port 9000  # custom port
```

**`src/ui/server.rs`** — Axum 0.7 HTTP server with CORS.

**`src/ui/api.rs`** — REST endpoints:
| Endpoint | Description |
|---|---|
| `GET /api/runs` | List last 100 runs, newest first |
| `GET /api/runs/:id` | Single run summary JSON |
| `GET /api/runs/:id/steps` | All step events for a run |
| `GET /api/runs/:id/stream` | SSE stream: replay history then send `done` event |
| `GET /api/cache` | Cache entry list (hash, OS, arch, age, artifacts) |
| `GET /api/labs` | Available lab OS options |

**`src/ui/dashboard.html`** — embedded dark-theme SPA (single-page app):
- Two-panel layout: run list (left) + step detail (right)
- Color-coded status dots (green / red / yellow pulse for running)
- Collapsible step cards with log output
- Auto-refresh every 10 seconds; manual refresh button

---

### 3. TUI Dashboard (`zenith tui`)

```bash
zenith tui
```

**`src/tui/mod.rs`** — ratatui + crossterm two-pane terminal dashboard.

**Layout:**
```
┌──────────────────────────────────────────────────────┐
│  ⚡ Zenith Dashboard  | q:quit r:refresh Tab:switch   │
├────────────────────────┬─────────────────────────────┤
│  Runs                  │  Steps (Enter to expand)    │
│  ● abc123… build      │  [DONE]   Install deps      │
│  ● def456… test       │  [DONE]   Build             │
│  ◉ ghi789… lint       │  [FAILED] Run tests         │
│                        ├─────────────────────────────┤
│                        │  Logs: Run tests            │
│                        │  npm ERR! test failed       │
└────────────────────────┴─────────────────────────────┘
```

**Key bindings:**
| Key | Action |
|---|---|
| `↑` / `k` | Move selection up |
| `↓` / `j` | Move selection down |
| `Tab` | Switch focus between panes |
| `Enter` / `→` | Expand step log pane |
| `←` | Return focus to run list |
| `r` | Refresh runs from disk |
| `q` / `Esc` | Quit |

---

### 4. VSCode Extension (`vscode-zenith/`)

A TypeScript extension installable from the `vscode-zenith/` directory at the repo root.

**Commands:**
| Command | Description |
|---|---|
| `Zenith: Run Workflow` | Run `zenith run` in the workspace |
| `Zenith: Run Specific Job` | Pick a job from `.zenith.yml` via QuickPick |
| `Zenith: Open Web Dashboard` | Open embedded WebView panel on port 7622 |
| `Zenith: Open TUI Dashboard` | Spawn `zenith tui` in an integrated terminal |
| `Zenith: Clean Cache` | Run `zenith cache clean` |
| `Zenith: Show Output` | Reveal the Zenith output channel |

**Features:**
- Output channel streams `zenith run` stdout/stderr in real time
- Status bar item: `$(zap) Zenith` → shows `running` / `success` / `failed` with color
- Embedded WebView dashboard proxy (no browser needed)
- YAML diagnostics: warns when `.zenith.yml` has no `jobs:` block
- `.zenith.yml` file watcher — re-validates on save
- JSON Schema at `vscode-zenith/schemas/zenith-schema.json` — enables autocomplete and validation for all `.zenith.yml` fields

**Settings:**
```json
"zenith.binaryPath": "zenith"
"zenith.dashboardPort": 7622
"zenith.autoOpenDashboard": false
```

---

## Key Files

| File | Role |
|---|---|
| `src/ui/history.rs` | Run history types + persistence (logger + reader) |
| `src/ui/server.rs` | Axum HTTP server, route registration |
| `src/ui/api.rs` | REST + SSE handler functions |
| `src/ui/dashboard.html` | Embedded dark-theme web dashboard |
| `src/ui/mod.rs` | `ui` module root |
| `src/tui/mod.rs` | ratatui two-pane TUI |
| `src/runner.rs` | Wired to `RunLogger` — writes history on every run |
| `vscode-zenith/package.json` | Extension manifest, commands, menus, settings |
| `vscode-zenith/src/extension.ts` | Extension implementation (TypeScript) |
| `vscode-zenith/schemas/zenith-schema.json` | JSON Schema for `.zenith.yml` |
| `vscode-zenith/language-configuration.json` | YAML language config (brackets, comments) |

---

## Verification Checklist

- [x] `zenith run` writes `~/.zenith/logs/<run-id>/summary.json` and `steps.jsonl`
- [x] `zenith ui` starts an HTTP server; dashboard loads at `http://localhost:7622`
- [x] Dashboard lists all past runs; clicking a run shows its steps
- [x] Step cards expand to show captured log output
- [x] Dashboard auto-refreshes every 10 seconds
- [x] `zenith tui` opens a two-pane terminal dashboard
- [x] TUI keyboard navigation: ↑↓ to select, Enter to expand logs, Tab to switch panes, r to refresh, q to quit
- [x] VSCode extension command `Zenith: Run Workflow` streams output to an output channel
- [x] VSCode status bar item reflects run state (running / success / failed)
- [x] VSCode `Zenith: Open Web Dashboard` opens embedded WebView
- [x] `.zenith.yml` has autocomplete and validation in VSCode via JSON Schema

---

## Next Steps

Phase 12 focuses on **Low-Level System Optimization** — custom kernel, minimal init process, and rootfs deduplication to cut VM boot times to under 50ms. See [phase_12.md](phase_12.md).
