# Terminal Dashboard (TUI)

The Zenith TUI is a fullscreen terminal dashboard built with ratatui. It provides the same run history and step log view as the web dashboard, without leaving the terminal.

---

## Starting the TUI

```bash
zenith tui
```

The TUI takes over the full terminal. It reads run history from `~/.zenith/logs/` — the daemon does not need to be running.

---

## Layout

```
┌─ Runs ──────────────────────┬─ Steps ────────────────────────────────────┐
│ ● ci           success  12s │ ✓ Install deps           0.8s              │
│ ● test         failed   5s  │ ✓ Build                  8.2s  [CACHED]    │
│ ● ci           success  11s │ ✗ Test                   3.1s              │
│                             │   > cargo test --lib                       │
│                             │   running 42 tests                         │
│                             │   test foo::bar ... FAILED                 │
│                             │   ...                                      │
└─────────────────────────────┴────────────────────────────────────────────┘
```

- **Left pane:** run list, sorted by most recent
- **Right pane:** steps for the selected run; expand a step to see its log output

---

## Key bindings

| Key | Action |
|---|---|
| `Tab` | Switch focus between the run list and step list |
| `↑` / `↓` | Navigate the focused list |
| `Enter` | Expand / collapse step log output |
| `r` | Refresh — reload run history from disk |
| `q` | Quit |

---

## Step status indicators

| Symbol | Meaning |
|---|---|
| `●` (yellow) | Running |
| `✓` (green) | Success |
| `✗` (red) | Failed |
| `○` (grey) | Cached (skipped) |
| `…` (blue) | Pending (waiting on `depends_on`) |

---

## Notes

- The TUI is read-only — it displays history but does not trigger new runs
- Run history is read from `~/.zenith/logs/`; start `zenith run` in another terminal to see live updates appear after pressing `r`
- Works in any terminal emulator that supports ANSI colour codes (256-colour or true colour recommended)
