/// REST API handlers for the web dashboard.
///
/// All responses are JSON unless noted.

use axum::{
    extract::Path,
    response::{
        IntoResponse,
        sse::{Event, Sse},
    },
    Json,
};
use futures_util::stream;
use std::convert::Infallible;
use std::time::Duration;

use crate::ui::history;

// ─── /api/runs ───────────────────────────────────────────────────────────────

pub async fn list_runs() -> impl IntoResponse {
    let runs = history::list_runs(100);
    Json(runs)
}

// ─── /api/runs/:id ───────────────────────────────────────────────────────────

pub async fn get_run(Path(id): Path<String>) -> impl IntoResponse {
    match history::get_run(&id) {
        Some(run) => Json(serde_json::to_value(run).unwrap_or_default()).into_response(),
        None => (
            axum::http::StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "run not found"})),
        )
            .into_response(),
    }
}

// ─── /api/runs/:id/steps ─────────────────────────────────────────────────────

pub async fn get_steps(Path(id): Path<String>) -> impl IntoResponse {
    let steps = history::get_steps(&id);
    Json(steps)
}

// ─── /api/runs/:id/stream (SSE) ──────────────────────────────────────────────

pub async fn stream_run(
    Path(id): Path<String>,
) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
    // Replay persisted step events as SSE, then send a "done" sentinel.
    let steps = history::get_steps(&id);
    let summary = history::get_run(&id);

    let mut events: Vec<Result<Event, Infallible>> = steps
        .into_iter()
        .filter_map(|s| {
            serde_json::to_string(&s).ok().map(|data| {
                Ok(Event::default().event("step").data(data))
            })
        })
        .collect();

    // Append a final "done" event carrying the run summary
    if let Some(sum) = summary {
        if let Ok(data) = serde_json::to_string(&sum) {
            events.push(Ok(Event::default().event("done").data(data)));
        }
    }

    Sse::new(stream::iter(events))
        .keep_alive(axum::response::sse::KeepAlive::new().interval(Duration::from_secs(15)))
}

// ─── /api/cache ──────────────────────────────────────────────────────────────

pub async fn list_cache() -> impl IntoResponse {
    match crate::sandbox::cache::CacheManager::new() {
        Ok(cm) => {
            let entries: Vec<_> = cm
                .list_entries()
                .into_iter()
                .map(|(hash, e)| {
                    serde_json::json!({
                        "hash":         hash,
                        "os":           e.os,
                        "arch":         e.arch,
                        "run":          e.run,
                        "created_at":   e.created_at_secs,
                        "has_artifacts": e.has_artifacts,
                    })
                })
                .collect();
            Json(serde_json::json!({"entries": entries})).into_response()
        }
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// ─── /api/labs ───────────────────────────────────────────────────────────────

pub async fn list_labs() -> impl IntoResponse {
    // Return the set of lab OS options supported by the container backend.
    let labs = serde_json::json!({
        "labs": [
            {"os": "ubuntu",  "description": "Ubuntu 22.04 LTS"},
            {"os": "alpine",  "description": "Alpine Linux 3.19"},
            {"os": "debian",  "description": "Debian 12 (Bookworm)"},
            {"os": "fedora",  "description": "Fedora 40"},
            {"os": "arch",    "description": "Arch Linux (rolling)"},
        ]
    });
    Json(labs)
}
