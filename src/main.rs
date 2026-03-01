// src/main.rs
// Entry point: load .env, parse CLI args, init tracing, start feed loop.

mod config;
mod connection;
mod output;
mod types;

use anyhow::Result;
use clap::Parser;
use tracing::{error, info};
use tracing_subscriber::{fmt, EnvFilter};

use config::Config;
use connection::run_feed;

// ── Single-threaded runtime ────────────────────────────────────────────────
// `current_thread` avoids cross-thread work-stealing overhead.
// For a single persistent WebSocket connection this gives the lowest jitter.
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    // ── Load .env file if present (silently ignored if missing) ───────────
    dotenvy::dotenv().ok();

    // ── Parse CLI flags / environment variables ────────────────────────────
    let cfg = Config::parse();

    // ── Initialise structured logging ──────────────────────────────────────
    // Default: WARN level.  Pass --verbose or RUST_LOG=info for more detail.
    let filter = if cfg.verbose {
        EnvFilter::new("info")
    } else {
        // Respect RUST_LOG env var; fall back to warn.
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"))
    };

    fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_ansi(true)
        .init();

    info!(
        "Starting XAU/USD fetcher | symbol={} | backoff_cap={}s | fmt={}",
        cfg.symbol, cfg.backoff_max_secs, cfg.output_format
    );

    // ── Run feed — retries forever with exponential backoff ────────────────
    if let Err(e) = run_feed(cfg).await {
        error!("Fatal: {:#}", e);
        std::process::exit(1);
    }

    Ok(())
}