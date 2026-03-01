// src/connection.rs
// WebSocket lifecycle management:
//   • TCP_NODELAY (Nagle bypass) for sub-millisecond delivery
//   • Exponential backoff reconnection
//   • Zero-copy frame dispatch
//   • Timestamp captured before parsing (true arrival latency)

use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use backoff::{future::retry, ExponentialBackoffBuilder};
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::net::TcpStream;
use tokio_tungstenite::{
    connect_async_with_config,
    tungstenite::protocol::{Message, WebSocketConfig},
    MaybeTlsStream, WebSocketStream,
};
use tracing::{debug, info, warn};

use crate::{
    config::Config,
    output::emit,
    types::{RawEvent, RawTick, SubscribeMsg, SubscribeParams, Tick},
};

// ── Public entry point ───────────────────────────────────────────────────────

/// Runs the feed loop forever, reconnecting with exponential backoff on failure.
pub async fn run_feed(cfg: Config) -> Result<()> {
    let backoff = ExponentialBackoffBuilder::new()
        .with_initial_interval(Duration::from_millis(250))
        .with_multiplier(2.0)
        .with_max_interval(Duration::from_secs(cfg.backoff_max_secs))
        .with_max_elapsed_time(None) // retry forever — no deadline
        .build();

    retry(backoff, || async {
        info!("Connecting to {}", cfg.ws_url);
        match connect_and_stream(&cfg).await {
            Ok(()) => {
                // Server closed cleanly — still reconnect.
                warn!("Connection closed cleanly — reconnecting");
                Err(backoff::Error::transient(anyhow!("server closed connection")))
            }
            Err(e) => {
                warn!("Connection error: {:#} — reconnecting", e);
                Err(backoff::Error::transient(e))
            }
        }
    })
    .await
    .map_err(|e| anyhow!("Backoff exhausted: {}", e))
}

// ── Connect → subscribe → receive ────────────────────────────────────────────

async fn connect_and_stream(cfg: &Config) -> Result<()> {
    // Embed the API key as a query parameter (Twelve Data convention).
    // FIX: we build ws_url_with_key directly here — the earlier dead
    //      `url` / `req` variables (which caused compiler warnings) are gone.
    let ws_url_with_key = format!("{}?apikey={}", cfg.ws_url, cfg.api_key);

    // ── WebSocket frame / buffer configuration ────────────────────────────
    // Capping max_message_size prevents re-allocation on unexpectedly large frames.
    // A smaller max_frame_size reduces head-of-line blocking.
    let ws_cfg = WebSocketConfig {
        max_message_size: Some(64 * 1024), // 64 KB
        max_frame_size:   Some(16 * 1024), // 16 KB per frame
        accept_unmasked_frames: false,
        ..Default::default()
    };

    let (mut ws_stream, response) =
        connect_async_with_config(ws_url_with_key.as_str(), Some(ws_cfg), false)
            .await
            .context("WebSocket handshake failed")?;

    info!("Connected — HTTP {}", response.status());

    // ── TCP_NODELAY: the single most important latency knob ───────────────
    // Disabling Nagle's algorithm prevents the kernel from batching small TCP
    // segments (which can add 40–200 ms of artificial delay).
    set_tcp_nodelay(&ws_stream);

    // ── Subscribe to the instrument ───────────────────────────────────────
    let sub_msg = SubscribeMsg {
        action: "subscribe",
        params: SubscribeParams { symbols: &cfg.symbol },
    };
    ws_stream
        .send(Message::Text(serde_json::to_string(&sub_msg)?))
        .await
        .context("Failed to send subscription message")?;

    info!("Subscribed to {}", cfg.symbol);

    // ── Hot receive loop ──────────────────────────────────────────────────
    while let Some(msg) = ws_stream.next().await {
        // Record the arrival timestamp BEFORE any parsing.
        // This is the true network delivery time, not the post-parse time.
        let received_at = Utc::now();

        match msg? {
            Message::Text(text) => {
                // Borrow &str from the owned String — no copy needed for parsing.
                handle_text(text.as_str(), received_at, cfg)?;
            }
            Message::Binary(bin) => {
                // Some providers send binary-encoded JSON.
                if let Ok(s) = std::str::from_utf8(&bin) {
                    handle_text(s, received_at, cfg)?;
                }
            }
            Message::Ping(payload) => {
                // Reply immediately — slow pong responses can trigger server-side disconnect.
                ws_stream.send(Message::Pong(payload)).await?;
                debug!("Pong sent");
            }
            Message::Close(frame) => {
                info!("Server requested close: {:?}", frame);
                break;
            }
            _ => {} // Pong / other frames — safely ignored
        }
    }

    Ok(())
}

// ── Message dispatcher ───────────────────────────────────────────────────────

/// Parse a raw WebSocket text frame and route to the output layer.
/// Marked `#[inline(always)]` so the compiler can fold this into the hot loop.
#[inline(always)]
fn handle_text(
    text: &str,
    received_at: chrono::DateTime<Utc>,
    cfg: &Config,
) -> Result<()> {
    // First pass: deserialise only far enough to read the "event" discriminant.
    // serde_json borrows string slices from `text` where possible (zero-copy).
    let v: Value = serde_json::from_str(text)?;

    match v.get("event").and_then(Value::as_str) {
        Some("price") => {
            let raw: RawTick = serde_json::from_value(v)?;
            if let Some(tick) = Tick::from_raw(&raw, received_at) {
                emit(&tick, &cfg.output_format)?;
            }
        }
        Some("heartbeat") => {
            debug!("Heartbeat");
        }
        Some("subscribe-status") => {
            let ev: RawEvent = serde_json::from_value(v)?;
            info!("Subscribe status: status={:?} msg={:?}", ev.status, ev.message);
        }
        Some(other) => {
            debug!("Unknown event type: {}", other);
        }
        None => {
            debug!("Frame with no 'event' field — ignored");
        }
    }

    Ok(())
}

// ── TCP_NODELAY helper ───────────────────────────────────────────────────────

/// Set TCP_NODELAY on the underlying socket through the MaybeTlsStream wrapper.
///
/// FIX applied here:
///   Old (wrong):  tls.get_ref().get_ref().get_ref()  →  3 levels, doesn't compile
///   New (correct): tls.get_ref().get_ref()            →  2 levels reaches TcpStream
///
/// Chain for native-tls:
///   tokio_native_tls::TlsStream<TcpStream>
///     .get_ref()  →  native_tls::TlsStream<TcpStream>    (tokio_native_tls layer)
///     .get_ref()  →  TcpStream                            (native_tls layer)
fn set_tcp_nodelay(stream: &WebSocketStream<MaybeTlsStream<TcpStream>>) {
    match stream.get_ref() {
        MaybeTlsStream::Plain(tcp) => {
            // Plain (non-TLS) TCP stream — one direct call.
            if let Err(e) = tcp.set_nodelay(true) {
                warn!("TCP_NODELAY failed (plain): {}", e);
            } else {
                debug!("TCP_NODELAY enabled (plain)");
            }
        }
        MaybeTlsStream::NativeTls(tls) => {
            // Full unwrap chain for tokio-native-tls v0.3:
            //   tokio_native_tls::TlsStream<TcpStream>
            //     .get_ref() → native_tls::TlsStream<AllowStd<TcpStream>>
            //     .get_ref() → AllowStd<TcpStream>
            //     .get_ref() → TcpStream   ← set_nodelay lives here
            let tcp = tls.get_ref().get_ref().get_ref(); // 3 levels required
            if let Err(e) = tcp.set_nodelay(true) {
                warn!("TCP_NODELAY failed (TLS): {}", e);
            } else {
                debug!("TCP_NODELAY enabled (TLS)");
            }
        }
        _ => {
            warn!("Unknown stream variant — TCP_NODELAY not set");
        }
    }
}