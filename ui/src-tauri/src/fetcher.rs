use std::time::{Duration, Instant};
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tauri::{AppHandle, Emitter};
use tokio::sync::{broadcast, mpsc};
use tokio_tungstenite::{
    connect_async_with_config,
    tungstenite::protocol::{Message, WebSocketConfig},
    MaybeTlsStream,
};
use zeroize::Zeroizing;
use crate::types::Tick;

#[derive(Clone, Debug)]
pub struct FeedConfig {
    pub api_key: String,
    pub ws_url:  String,
    pub symbol:  String,
}

#[derive(Deserialize)]
struct RawTick {
    event:  Option<String>,
    symbol: Option<String>,
    price:  Option<f64>,
    bid:    Option<f64>,
    ask:    Option<f64>,
}

pub async fn run_feed(
    app:         AppHandle,
    config:      FeedConfig,
    mut stop_rx: broadcast::Receiver<()>,
    mut sym_rx:  mpsc::Receiver<String>,
    tick_tx:     mpsc::Sender<Tick>,
) {
    let mut backoff_ms = 250u64;

    loop {
        emit_status(&app, false, "Connecting...");

        let url = Zeroizing::new(
            format!("{}?apikey={}", config.ws_url, config.api_key)
        );

        let ws_cfg = WebSocketConfig {
            max_message_size:       Some(64 * 1024),
            max_frame_size:         Some(16 * 1024),
            accept_unmasked_frames: false,
            ..Default::default()
        };

        let connect_result =
            connect_async_with_config(url.as_str(), Some(ws_cfg), false).await;
        drop(url);

        let (mut ws, _) = match connect_result {
            Ok(r) => { backoff_ms = 250; r }
            Err(e) => {
                emit_status(&app, false, &format!("Reconnecting in {}ms...", backoff_ms));
                tokio::select! {
                    biased;
                    _ = stop_rx.recv() => { emit_status(&app, false, "Disconnected"); return; }
                    _ = tokio::time::sleep(Duration::from_millis(backoff_ms)) => {}
                }
                backoff_ms = (backoff_ms * 2).min(30_000);
                let _ = e;
                continue;
            }
        };

        set_nodelay(&ws);

        let mut current_sym = config.symbol.clone();
        if let Err(e) = subscribe(&mut ws, &current_sym).await {
            let _ = e;
            continue;
        }

        emit_status(&app, true, &format!("Live · {}", current_sym));

        let dropped = loop {
            tokio::select! {
                biased;

                _ = stop_rx.recv() => {
                    let _ = ws.close(None).await;
                    emit_status(&app, false, "Disconnected");
                    return;
                }

                Some(new_sym) = sym_rx.recv() => {
                    if new_sym != current_sym {
                        let unsub = serde_json::json!({
                            "action": "unsubscribe",
                            "params": { "symbols": current_sym }
                        });
                        let _ = ws.send(Message::Text(unsub.to_string())).await;
                        if subscribe(&mut ws, &new_sym).await.is_ok() {
                            current_sym = new_sym.clone();
                            emit_status(&app, true, &format!("Live · {}", new_sym));
                            let _ = app.emit("symbol-changed", &new_sym);
                        }
                    }
                }

                msg = ws.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            if let Some(tick) = parse_tick(&text) {
                                let _ = app.emit("tick", &tick);
                                let _ = tick_tx.try_send(tick);
                            }
                        }
                        Some(Ok(Message::Ping(p))) => {
                            let _ = ws.send(Message::Pong(p)).await;
                        }
                        Some(Ok(Message::Close(_))) | None => break true,
                        _ => {}
                    }
                }
            }
        };

        if dropped {
            emit_status(&app, false, "Reconnecting...");
            tokio::select! {
                biased;
                _ = stop_rx.recv() => { emit_status(&app, false, "Disconnected"); return; }
                _ = tokio::time::sleep(Duration::from_millis(backoff_ms)) => {}
            }
            backoff_ms = (backoff_ms * 2).min(30_000);
        }
    }
}

#[inline(always)]
fn parse_tick(text: &str) -> Option<Tick> {
    let start = Instant::now();
    let ts_ms = Utc::now().timestamp_millis();
    let raw: RawTick = serde_json::from_str(text).ok()?;
    if raw.event.as_deref() != Some("price") { return None; }
    let sym = raw.symbol?;
    let (bid, ask, spread) = match (raw.bid, raw.ask, raw.price) {
        (Some(b), Some(a), _) => (b, a, (a - b).abs()),
        (_, _, Some(p))       => (p, p, 0.0),
        _                     => return None,
    };
    Some(Tick {
        timestamp_ms: ts_ms,
        timestamp:    Utc::now().to_rfc3339(),
        symbol: sym, bid, ask, spread,
        latency_us: start.elapsed().as_micros() as u64,
    })
}

async fn subscribe(
    ws:  &mut tokio_tungstenite::WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>,
    sym: &str,
) -> anyhow::Result<()> {
    let msg = serde_json::json!({
        "action": "subscribe",
        "params": { "symbols": sym }
    });
    ws.send(Message::Text(msg.to_string())).await?;
    Ok(())
}

fn set_nodelay(ws: &tokio_tungstenite::WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>) {
    match ws.get_ref() {
        MaybeTlsStream::Plain(tcp) => { let _ = tcp.set_nodelay(true); }
        MaybeTlsStream::NativeTls(tls) => {
            let _ = tls.get_ref().get_ref().get_ref().set_nodelay(true);
        }
        _ => {}
    }
}

fn emit_status(app: &AppHandle, connected: bool, msg: &str) {
    let _ = app.emit("connection-status",
        serde_json::json!({ "connected": connected, "message": msg }));
}

