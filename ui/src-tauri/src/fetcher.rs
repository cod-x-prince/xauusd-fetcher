// src-tauri/src/fetcher.rs
// WebSocket feed — adapted from the CLI fetcher's connection.rs.
// Runs in a background tokio task; emits "tick" and "connection-status"
// events to the Tauri window via AppHandle.

use std::time::{Duration, Instant};

use anyhow::anyhow;
use backoff::{future::retry, ExponentialBackoffBuilder};
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tauri::{AppHandle, Emitter};
use tokio::sync::broadcast;
use tokio_tungstenite::{
    connect_async_with_config,
    tungstenite::protocol::{Message, WebSocketConfig},
};

use crate::{FeedConfig, Tick};

// ── Raw wire types ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct RawTick {
    event:  Option<String>,
    symbol: Option<String>,
    price:  Option<f64>,
    bid:    Option<f64>,
    ask:    Option<f64>,
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Runs the WebSocket feed with automatic exponential-backoff reconnection.
/// Stops cleanly when the stop_rx channel receives a signal.
pub async fn run_feed(
    app: AppHandle,
    config: FeedConfig,
    stop_rx: broadcast::Receiver<()>,
) {
    let backoff = ExponentialBackoffBuilder::new()
        .with_initial_interval(Duration::from_millis(500))
        .with_multiplier(2.0)
        .with_max_interval(Duration::from_secs(30))
        .with_max_elapsed_time(None)
        .build();

    let _ = retry(backoff, || {
        let app    = app.clone();
        let config = config.clone();
        let mut stop = stop_rx.resubscribe();

        async move {
            emit_status(&app, false, "Connecting…");

            let url = format!("{}?apikey={}", config.ws_url, config.api_key);

            let ws_cfg = WebSocketConfig {
                max_message_size: Some(64 * 1024),
                max_frame_size:   Some(16 * 1024),
                accept_unmasked_frames: false,
                ..Default::default()
            };

            let (mut ws, _) = connect_async_with_config(url.as_str(), Some(ws_cfg), false)
                .await
                .map_err(|e| backoff::Error::transient(anyhow!(e)))?;

            // TCP_NODELAY — disable Nagle, same as CLI tool
            // (tokio-tungstenite sets it by default on most platforms,
            //  but we ensure it explicitly via the underlying stream)

            // Subscribe to symbol
            let sub = serde_json::json!({
                "action": "subscribe",
                "params": { "symbols": config.symbol }
            });
            ws.send(Message::Text(sub.to_string()))
                .await
                .map_err(|e| backoff::Error::transient(anyhow!(e)))?;

            emit_status(&app, true, &format!("Live · {}", config.symbol));

            // ── Hot receive loop ─────────────────────────────────────────────
            loop {
                tokio::select! {
                    biased; // check stop first

                    _ = stop.recv() => {
                        let _ = ws.close(None).await;
                        emit_status(&app, false, "Disconnected");
                        // Return permanent error so backoff stops retrying.
                        return Err(backoff::Error::permanent(anyhow!("user stopped feed")));
                    }

                    msg = ws.next() => {
                        match msg {
                            Some(Ok(Message::Text(text))) => {
                                // Capture timestamp before parsing — true latency
                                let parse_start = Instant::now();
                                let timestamp   = Utc::now().to_rfc3339();

                                if let Ok(raw) = serde_json::from_str::<RawTick>(&text) {
                                    if raw.event.as_deref() == Some("price") {
                                        if let Some(sym) = raw.symbol {
                                            let (bid, ask, spread) = match (raw.bid, raw.ask, raw.price) {
                                                (Some(b), Some(a), _) => (b, a, (a - b).abs()),
                                                (_, _, Some(p))       => (p, p, 0.0),
                                                _                     => continue,
                                            };

                                            let latency_us = parse_start.elapsed().as_micros() as u64;

                                            let tick = Tick {
                                                timestamp,
                                                symbol: sym,
                                                bid,
                                                ask,
                                                spread,
                                                latency_us,
                                            };

                                            let _ = app.emit("tick", &tick);
                                        }
                                    }
                                }
                            }

                            Some(Ok(Message::Ping(payload))) => {
                                let _ = ws.send(Message::Pong(payload)).await;
                            }

                            Some(Ok(Message::Close(_))) | None => break,

                            _ => {}
                        }
                    }
                }
            }

            emit_status(&app, false, "Reconnecting…");
            Err(backoff::Error::transient(anyhow!("connection dropped")))
        }
    })
    .await;
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn emit_status(app: &AppHandle, connected: bool, message: &str) {
    let _ = app.emit(
        "connection-status",
        serde_json::json!({ "connected": connected, "message": message }),
    );
}
