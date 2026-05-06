//! `bgplg` — the looking-glass HTTP API binary.
//!
//! M0 scaffolding only: stands up an axum server with `/api/v1/health` so the
//! build pipeline has something real to compile and exercise.

use anyhow::Result;
use axum::{routing::get, Json, Router};
use serde::Serialize;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Serialize)]
struct Health {
    status: &'static str,
    version: &'static str,
}

async fn health() -> Json<Health> {
    Json(Health {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let app = Router::new().route("/api/v1/health", get(health));

    let addr = std::env::var("BGPLG_LISTEN").unwrap_or_else(|_| "127.0.0.1:8080".into());
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!(%addr, "bgplg listening");
    axum::serve(listener, app).await?;
    Ok(())
}
