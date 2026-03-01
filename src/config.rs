// src/config.rs
// All runtime configuration. Sources: CLI flags → environment variables → defaults.

use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(
    name    = "xauusd-fetcher",
    about   = "Ultra-low-latency XAU/USD price feed via Twelve Data WebSocket",
    version = "0.1.0"
)]
pub struct Config {
    /// Twelve Data API key.
    /// Also settable via API_KEY environment variable.
    #[arg(long, env = "API_KEY", hide_env_values = true)]
    pub api_key: String,

    /// WebSocket endpoint URL.
    /// Override to switch providers.
    #[arg(
        long,
        env = "WS_URL",
        default_value = "wss://ws.twelvedata.com/v1/quotes/price"
    )]
    pub ws_url: String,

    /// Instrument symbol as accepted by the provider.
    #[arg(long, env = "SYMBOL", default_value = "XAU/USD")]
    pub symbol: String,

    /// Enable verbose connection lifecycle logging.
    #[arg(long, short = 'v', default_value_t = false)]
    pub verbose: bool,

    /// Maximum reconnect backoff ceiling in seconds.
    #[arg(long, env = "BACKOFF_MAX_SECS", default_value_t = 60)]
    pub backoff_max_secs: u64,

    /// Output format: "json" (NDJSON, default) or "csv".
    #[arg(long, env = "OUTPUT_FORMAT", default_value = "json")]
    pub output_format: String,
}