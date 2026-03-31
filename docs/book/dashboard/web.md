# Web Dashboard

The Zenith web dashboard provides a browser-based view of all past and live runs, step-level logs, cache statistics, and active lab environments.

---

## Starting the dashboard

```bash
zenith ui                # starts on http://localhost:7622
zenith ui --port 9000    # custom port
```

Open `http://localhost:7622` in any browser. The server runs until you stop it with `Ctrl+C`.

---

## Features

### Run list

The left panel shows all past and active runs, ordered by most recent. Each row shows:

- Run ID
- Job name
- Overall status (running / success / failed)
- Start time and duration

Click any run to see its step detail in the right panel.

### Step detail

For the selected run, the right panel shows each step with:

- Step name
- Status badge: `running` / `cached` / `success` / `failed`
- Duration
- Collapsible log output — click a step to expand its stdout/stderr

### Auto-refresh

The dashboard polls for new data every 10 seconds automatically. Live runs update in near-real-time.

### Cache statistics

A summary card at the top shows:

- Total cache entries
- Total size on disk
- Number of hits in the last 24 hours

### Active labs

A panel lists all active `zenith lab` environments with their OS and workspace path.

---

## REST API

The dashboard is backed by a local REST API on the same port:

| Endpoint | Description |
|---|---|
| `GET /api/runs` | List all run summaries |
| `GET /api/runs/:id` | Full run detail with steps |
| `GET /api/runs/:id/steps` | Steps for a run |
| `GET /api/runs/:id/stream` | SSE stream that replays history and delivers live updates |
| `GET /api/cache` | Cache statistics |
| `GET /api/labs` | Active lab environments |

You can query these directly for scripting or integration:

```bash
curl http://localhost:7622/api/runs | jq '.[0]'
```

---

## Notes

- The dashboard reads run history from `~/.zenith/logs/` — it does not require the daemon to be running
- All data is local; nothing is sent to an external service
- The embedded HTML/JS is served directly from the `zenith ui` process — no separate web server is needed
