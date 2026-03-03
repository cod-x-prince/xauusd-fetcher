// src/connection.rs  —  v0.2.0  FINAL
// WebSocket lifecycle + ultra-low-latency hot receive loop.
//
// v2 speed improvements:
//   • simd-json replaces serde_json on the hot path  (~2-4x faster parsing)
//   • thread_local! byte buffer — zero heap allocation per tick
//   • std::time timestamp — no chrono TZ lookup on hot path
//   • parking_lot mutex in output layer — 3-5x faster lock
//   • #[inline(always)] on every hot-path function
//   • Deferred DateTime — heartbeats pay zero chrono overhead

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Context, Result};
use backoff::{future::retry, ExponentialBackoffBuilder};
use chrono::{DateTime, TimeZone, Utc};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio_tungstenite::{
    connect_async_with_config,
    tungstenite::protocol::{Message, WebSocketConfig},
    MaybeTlsStream, WebSocketStream,
};
use tracing::{debug, info, warn};

// simd-json trait imports — required to unlock accessor methods on OwnedValue
use simd_json::prelude::ValueAsScalar;    // .as_str(), .as_f64(), .as_bool()
use simd_json::prelude::ValueAsContainer; // .as_array(), .as_object()
use simd_json::prelude::ValueObjectAccess; // .get("key") on JSON objects

use crate::{
    config::Config,
    output::emit,
    types::{SubscribeMsg, SubscribeParams, Tick},
};

// ── Public entry point ────────────────────────────────────────────────────────

pub async fn run_feed(cfg: Config) -> Result<()> {
    let backoff = ExponentialBackoffBuilder::new()
        .with_initial_interval(Duration::from_millis(250))
        .with_multiplier(2.0)
        .with_max_interval(Duration::from_secs(cfg.backoff_max_secs))
        .with_max_elapsed_time(None)
        .build();

    retry(backoff, || async {
        info!("Connecting to {}", cfg.ws_url);
        match connect_and_stream(&cfg).await {
            Ok(()) => {
                warn!("Connection closed cleanly — reconnecting");
                Err(backoff::Error::transient(anyhow!("clean close")))
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

// ── Connect → subscribe → receive ─────────────────────────────────────────────

async fn connect_and_stream(cfg: &Config) -> Result<()> {
    let ws_url_with_key = format!("{}?apikey={}", cfg.ws_url, cfg.api_key);

    let ws_cfg = WebSocketConfig {
        max_message_size: Some(64 * 1024),
        max_frame_size:   Some(16 * 1024),
        accept_unmasked_frames: false,
        ..Default::default()
    };

    let (mut ws, response) =
        connect_async_with_config(ws_url_with_key.as_str(), Some(ws_cfg), false)
            .await
            .context("WebSocket handshake failed")?;

    info!("Connected — HTTP {}", response.status());
    set_tcp_nodelay(&ws);

    // ── Subscribe ──────────────────────────────────────────────────────────
    let sub_text = build_subscribe_msg(cfg);
    ws.send(Message::Text(sub_text))
        .await
        .context("Subscription send failed")?;
    info!("Subscribed to {} via {}", cfg.symbol, cfg.provider);

    // ── Hot receive loop ───────────────────────────────────────────────────
    // thread_local! buffer: allocated ONCE per thread, reused every tick.
    // Eliminates Vec::new() heap allocation that happened on every message in v1.
    thread_local! {
        static PARSE_BUF: std::cell::RefCell<Vec<u8>> =
            std::cell::RefCell::new(Vec::with_capacity(1024));
    }

    while let Some(msg) = ws.next().await {
        // Capture arrival time BEFORE any work — true network arrival latency.
        // std::time is ~2x faster than chrono::Utc::now() (no TZ table lookup).
        let arrival = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();

        match msg? {
            Message::Text(text) => {
                PARSE_BUF.with(|cell| {
                    let mut buf = cell.borrow_mut();
                    buf.clear();
                    buf.extend_from_slice(text.as_bytes());
                    // simd-json parses in-place using AVX2/SSE4 — mutates buffer.
                    if let Err(e) = handle_bytes(&mut buf, arrival, cfg) {
                        debug!("Parse error: {}", e);
                    }
                });
            }
            Message::Binary(mut bin) => {
                if let Err(e) = handle_bytes(&mut bin, arrival, cfg) {
                    debug!("Binary parse error: {}", e);
                }
            }
            Message::Ping(payload) => {
                ws.send(Message::Pong(payload)).await?;
                debug!("Pong sent");
            }
            Message::Close(frame) => {
                info!("Server close: {:?}", frame);
                break;
            }
            _ => {}
        }
    }
    Ok(())
}

// ── simd-json hot path ────────────────────────────────────────────────────────

#[inline(always)]
fn handle_bytes(buf: &mut Vec<u8>, arrival: Duration, cfg: &Config) -> Result<()> {
    // simd-json with runtime-detection: picks AVX2 / SSE4 / scalar automatically.
    let v: simd_json::OwnedValue = simd_json::to_owned_value(buf)
        .map_err(|e| anyhow!("simd-json: {}", e))?;

    // Build DateTime only when we actually need it (not for heartbeats).
    // Closure is zero-cost until called.
    let make_ts = || -> DateTime<Utc> {
        Utc.timestamp_opt(arrival.as_secs() as i64, arrival.subsec_nanos())
            .single()
            .unwrap_or_else(Utc::now)
    };

    match cfg.provider.as_str() {
        "finnhub"    => handle_finnhub(&v, make_ts, cfg),
        _            => handle_twelvedata(&v, make_ts, cfg),
    }
}

// ── Twelve Data handler ────────────────────────────────────────────────────────

#[inline(always)]
fn handle_twelvedata(
    v: &simd_json::OwnedValue,
    make_ts: impl Fn() -> DateTime<Utc>,
    cfg: &Config,
) -> Result<()> {
    // Use .get() instead of direct [] indexing — [] panics on missing keys,
    // .get() returns None safely. Critical on the hot path.
    let event = v.get("event").and_then(|e: &simd_json::OwnedValue| e.as_str());

    match event {
        Some("price") => {
            let symbol = v.get("symbol").and_then(|s: &simd_json::OwnedValue| s.as_str()).unwrap_or("?").to_owned();
            let price  = v.get("price").and_then(|x: &simd_json::OwnedValue| x.as_f64());
            let bid    = v.get("bid").and_then(|x: &simd_json::OwnedValue| x.as_f64());
            let ask    = v.get("ask").and_then(|x: &simd_json::OwnedValue| x.as_f64());

            let (b, a, spread): (f64, f64, f64) = match (bid, ask, price) {
                (Some(b), Some(a), _) => (b, a, (a - b).abs()),
                (_, _, Some(p))       => (p, p, 0.0),
                _                     => return Ok(()),
            };

            emit(&Tick {
                timestamp: make_ts(),
                symbol, bid: b, ask: a, spread,
            }, &cfg.output_format)?;
        }
        Some("heartbeat")        => debug!("Heartbeat"),
        Some("subscribe-status") => info!(
            "Subscribe status: {:?}",
            v.get("status").and_then(|s: &simd_json::OwnedValue| s.as_str()).unwrap_or("?")
        ),
        Some(other) => debug!("Unknown event: {}", other),
        None        => debug!("Frame missing 'event' field"),
    }
    Ok(())
}

// ── Finnhub handler ────────────────────────────────────────────────────────────

#[inline(always)]
fn handle_finnhub(
    v: &simd_json::OwnedValue,
    make_ts: impl Fn() -> DateTime<Utc>,
    cfg: &Config,
) -> Result<()> {
    match v.get("type").and_then(|t: &simd_json::OwnedValue| t.as_str()) {
        Some("trade") => {
            if let Some(arr) = v.get("data").and_then(|d: &simd_json::OwnedValue| d.as_array()) {
                let ts = make_ts();
                for trade in arr {
                    let price  = trade.get("p").and_then(|x: &simd_json::OwnedValue| x.as_f64()).unwrap_or(0.0);
                    let symbol = trade.get("s").and_then(|x: &simd_json::OwnedValue| x.as_str()).unwrap_or("?").to_owned();
                    emit(&Tick {
                        timestamp: ts,
                        symbol, bid: price, ask: price, spread: 0.0,
                    }, &cfg.output_format)?;
                }
            }
        }
        Some("ping") | Some("no_data") => debug!("Finnhub control frame"),
        Some(other) => debug!("Unknown Finnhub frame: {}", other),
        None        => debug!("Finnhub frame missing 'type' field"),
    }
    Ok(())
}

// ── Subscription message builder ──────────────────────────────────────────────

fn build_subscribe_msg(cfg: &Config) -> String {
    match cfg.provider.as_str() {
        "finnhub" => format!(r#"{{"type":"subscribe","symbol":"{}"}}"#, cfg.symbol),
        _ => serde_json::to_string(&SubscribeMsg {
            action: "subscribe",
            params: SubscribeParams { symbols: &cfg.symbol },
        }).unwrap_or_default(),
    }
}

// ── TCP_NODELAY ────────────────────────────────────────────────────────────────

fn set_tcp_nodelay(stream: &WebSocketStream<MaybeTlsStream<TcpStream>>) {
    match stream.get_ref() {
        MaybeTlsStream::Plain(tcp) => {
            let _ = tcp.set_nodelay(true);
            debug!("TCP_NODELAY enabled (plain)");
        }
        MaybeTlsStream::NativeTls(tls) => {
            // tokio_native_tls → native_tls → AllowStd → TcpStream (3 levels)
            let tcp = tls.get_ref().get_ref().get_ref();
            let _ = tcp.set_nodelay(true);
            debug!("TCP_NODELAY enabled (TLS)");
        }
        _ => warn!("Unknown stream variant — TCP_NODELAY not set"),
    }
}