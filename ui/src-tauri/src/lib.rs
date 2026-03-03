// src-tauri/src/lib.rs
// Tauri application: state management, commands, plugin registration.

mod fetcher;

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tokio::sync::{broadcast, Mutex};

use fetcher::run_feed;

// ── Public types (shared with frontend via IPC) ───────────────────────────────

/// Normalised price tick emitted to the frontend.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Tick {
    pub timestamp:  String,
    pub symbol:     String,
    pub bid:        f64,
    pub ask:        f64,
    pub spread:     f64,
    /// Parse latency in microseconds (time from frame arrival → structured tick)
    pub latency_us: u64,
}

/// Feed configuration — sent from frontend settings panel.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct FeedConfig {
    pub api_key: String,
    pub ws_url:  String,
    pub symbol:  String,
}

impl Default for FeedConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            ws_url:  "wss://ws.twelvedata.com/v1/quotes/price".to_string(),
            symbol:  "XAU/USD".to_string(),
        }
    }
}

// ── Application state ─────────────────────────────────────────────────────────

pub struct AppState {
    config:  Arc<Mutex<FeedConfig>>,
    stop_tx: Arc<Mutex<Option<broadcast::Sender<()>>>>,
}

// ── Tauri commands (callable from JS via invoke()) ────────────────────────────

/// Start (or restart) the WebSocket feed with the given config.
#[tauri::command]
async fn start_feed(
    app:    AppHandle,
    state:  tauri::State<'_, AppState>,
    config: FeedConfig,
) -> Result<(), String> {
    // Stop any running feed first
    stop_feed_inner(&state).await;

    // Persist config
    *state.config.lock().await = config.clone();

    // Create a fresh stop channel
    let (stop_tx, stop_rx) = broadcast::channel::<()>(1);
    *state.stop_tx.lock().await = Some(stop_tx);

    // Spawn the feed on the Tokio runtime — runs until stop signal or fatal error
    tokio::spawn(async move {
        run_feed(app, config, stop_rx).await;
    });

    Ok(())
}

/// Stop the running feed.
#[tauri::command]
async fn stop_feed(state: tauri::State<'_, AppState>) -> Result<(), String> {
    stop_feed_inner(&state).await;
    Ok(())
}

/// Return the current feed configuration.
#[tauri::command]
async fn get_config(state: tauri::State<'_, AppState>) -> Result<FeedConfig, String> {
    Ok(state.config.lock().await.clone())
}

async fn stop_feed_inner(state: &AppState) {
    if let Some(tx) = state.stop_tx.lock().await.take() {
        // Ignore the error — receiver may already be gone
        let _ = tx.send(());
    }
}

// ── Tauri entry point ─────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::default().build())
        .manage(AppState {
            config:  Arc::new(Mutex::new(FeedConfig::default())),
            stop_tx: Arc::new(Mutex::new(None)),
        })
        .invoke_handler(tauri::generate_handler![
            start_feed,
            stop_feed,
            get_config,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Tauri application");
}
