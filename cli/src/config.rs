// src/config.rs  —  v0.2.0  FINAL
// All runtime configuration: CLI flags → env vars → defaults.

use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(
    name    = "xauusd-fetcher",
    about   = "Ultra-low-latency XAU/USD price feed (v2 — SIMD + turbo)",
    version = env!("CARGO_PKG_VERSION"),
)]
pub struct Config {
    /// Twelve Data or Finnhub API key.
    #[arg(long, env = "API_KEY", hide_env_values = true)]
    pub api_key: String,

    /// WebSocket endpoint URL.
    #[arg(long, env = "WS_URL",
          default_value = "wss://ws.twelvedata.com/v1/quotes/price")]
    pub ws_url: String,

    /// Instrument symbol (provider-specific format).
    /// Twelve Data: XAU/USD  |  Finnhub: OANDA:XAU_USD
    #[arg(long, env = "SYMBOL", default_value = "XAU/USD")]
    pub symbol: String,

    /// Provider: "twelvedata" or "finnhub".
    #[arg(long, env = "PROVIDER", default_value = "twelvedata")]
    pub provider: String,

    /// Output format: "json" (NDJSON) or "csv".
    #[arg(long, env = "OUTPUT_FORMAT", default_value = "json")]
    pub output_format: String,

    /// Maximum reconnect backoff ceiling in seconds.
    #[arg(long, env = "BACKOFF_MAX_SECS", default_value_t = 60)]
    pub backoff_max_secs: u64,

    /// Show connection lifecycle logs on stderr.
    #[arg(long, short = 'v', default_value_t = false)]
    pub verbose: bool,

    /// [v2] Engage turbo mode: applies safe OS-level speed boosts
    /// (1ms timer resolution, HIGH process priority, High Performance
    /// power plan) and auto-reverts ALL changes on exit / Ctrl+C.
    #[arg(long, default_value_t = false)]
    pub turbo: bool,

    /// [v2] CPU core index to pin the async executor to (default: 2).
    /// Avoids core 0 (OS IRQs) and core 1 (OS scheduler tasks).
    #[arg(long, env = "CORE_ID", default_value_t = 2)]
    pub core_id: usize,
}