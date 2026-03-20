/// Axum HTTP server — serves the web dashboard on http://localhost:7622
///
/// Endpoints:
///   GET  /                        → embedded HTML dashboard
///   GET  /api/runs                → list_runs JSON
///   GET  /api/runs/:id            → single run summary JSON
///   GET  /api/runs/:id/steps      → step events JSON
///   GET  /api/runs/:id/stream     → SSE stream (placeholder, flushes history)
///   GET  /api/cache               → cache entry list JSON
///   GET  /api/labs                → available lab OS list JSON

use axum::{
    Router,
    routing::get,
};
use tower_http::cors::{Any, CorsLayer};

use super::api;

// Embedded HTML/JS dashboard
static DASHBOARD_HTML: &str = include_str!("dashboard.html");

pub async fn serve(port: u16) -> anyhow::Result<()> {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/", get(serve_dashboard))
        .route("/api/runs", get(api::list_runs))
        .route("/api/runs/:id", get(api::get_run))
        .route("/api/runs/:id/steps", get(api::get_steps))
        .route("/api/runs/:id/stream", get(api::stream_run))
        .route("/api/cache", get(api::list_cache))
        .route("/api/labs", get(api::list_labs))
        .layer(cors);

    let addr = format!("127.0.0.1:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("Zenith dashboard running at http://{}", addr);
    println!("Press Ctrl+C to stop.");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn serve_dashboard() -> axum::response::Html<&'static str> {
    axum::response::Html(DASHBOARD_HTML)
}
