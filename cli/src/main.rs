// src/main.rs  —  v0.2.0
// Entry point: load .env, parse CLI, init tracing, apply v2 speed boosts,
// then hand off to the feed loop.

mod config;
mod connection;
mod output;
mod turbo;
mod types;

use anyhow::Result;
use clap::Parser;
use tracing::{info, warn};
use tracing_subscriber::{fmt, EnvFilter};

use config::Config;
use connection::run_feed;
use turbo::TurboGuard;

// ── Single-threaded runtime ────────────────────────────────────────────────
// Eliminates cross-thread work-stealing overhead & cache-line bouncing.
// For a single persistent WebSocket connection this gives lowest jitter.
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    // ── Load .env (silently ignored if absent) ─────────────────────────────
    dotenvy::dotenv().ok();

    // ── Parse CLI / env ────────────────────────────────────────────────────
    let cfg = Config::parse();

    // ── Tracing ────────────────────────────────────────────────────────────
    let filter = if cfg.verbose {
        EnvFilter::new("info")
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"))
    };
    fmt().with_env_filter(filter).with_target(false).with_ansi(true).init();

    info!(
        "XAU/USD Fetcher v{} | symbol={} | provider={} | fmt={}",
        env!("CARGO_PKG_VERSION"),
        cfg.symbol,
        cfg.provider,
        cfg.output_format,
    );

    // ── v2: Turbo subsystem (RAII — auto-reverts on exit / Ctrl+C) ────────
    // With --turbo:  applies timer resolution + process priority + core pin.
    // Without:       system is completely untouched.
    let _turbo = if cfg.turbo {
        Some(TurboGuard::engage())
    } else {
        info!("Running in normal mode. Use --turbo to enable OS-level speed boosts.");
        None
    };

    // ── v2: Pin main thread to a dedicated core (always, not just turbo) ──
    // Prevents OS from migrating the async executor between cores mid-tick.
    // Core 2 avoids core 0 (OS IRQs) and core 1 (OS scheduler).
    if let Some(cores) = core_affinity::get_core_ids() {
        let target = cores.get(2).or_else(|| cores.last()).copied();
        if let Some(core) = target {
            if core_affinity::set_for_current(core) {
                info!("Executor pinned to CPU core {}", core.id);
            } else {
                warn!("Could not pin to core {} — continuing unpinned", core.id);
            }
        }
    }

    // ── Feed loop — retries forever with exponential backoff ───────────────
    if let Err(e) = run_feed(cfg).await {
        tracing::error!("Fatal: {:#}", e);
        std::process::exit(1);
    }

    Ok(())
}