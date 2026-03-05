mod aggregator;
mod fetcher;
mod patterns;
mod recorder;
mod types;

use std::{path::PathBuf, sync::Arc};
use parking_lot::Mutex as PLMutex;
use tauri::{AppHandle, Emitter};
use tokio::sync::{broadcast, mpsc};
use zeroize::Zeroizing;
use aggregator::AggregatorHandle;
use fetcher::FeedConfig;
use recorder::{RecorderHandle, RecorderMsg};
use types::{FeedConfigIn, FeedConfigOut, Timeframe};

pub struct AppState {
    api_key:    Arc<PLMutex<Zeroizing<String>>>,
    ws_url:     Arc<PLMutex<String>>,
    symbol:     Arc<PLMutex<String>>,
    timeframe:  Arc<PLMutex<Timeframe>>,
    stop_tx:    Arc<PLMutex<Option<broadcast::Sender<()>>>>,
    symbol_tx:  Arc<PLMutex<Option<mpsc::Sender<String>>>>,
    agg_handle: Arc<PLMutex<Option<AggregatorHandle>>>,
    rec_handle: Arc<PLMutex<Option<RecorderHandle>>>,
    data_dir:   PathBuf,
}

#[tauri::command]
fn start_feed(
    app:    AppHandle,
    state:  tauri::State<'_, AppState>,
    config: FeedConfigIn,
) -> Result<(), String> {
    stop_inner(&state);

    let tf = config.timeframe.as_deref()
        .and_then(Timeframe::from_str)
        .unwrap_or(Timeframe::M1);

    *state.api_key.lock()   = Zeroizing::new(config.api_key.clone());
    *state.ws_url.lock()    = config.ws_url.clone();
    *state.symbol.lock()    = config.symbol.clone();
    *state.timeframe.lock() = tf;

    // Clone everything needed before spawning
    let app2      = app.clone();
    let ws_url    = config.ws_url.clone();
    let symbol    = config.symbol.clone();
    let api_key   = config.api_key.clone();
    let data_dir  = state.data_dir.clone();
    let stop_tx_arc  = state.stop_tx.clone();
    let sym_tx_arc   = state.symbol_tx.clone();
    let agg_arc      = state.agg_handle.clone();
    let rec_arc      = state.rec_handle.clone();

    tauri::async_runtime::spawn(async move {
        // save config
        if let Ok(store) = tauri_plugin_store::StoreBuilder::new(&app2, "settings.json").build() {
            store.set("ws_url",      serde_json::Value::String(ws_url.clone()));
            store.set("symbol",      serde_json::Value::String(symbol.clone()));
            store.set("timeframe",   serde_json::Value::String(tf.label().to_string()));
            store.set("has_api_key", serde_json::Value::Bool(true));
            let _ = store.save();
        }

        let (stop_tx, stop_rx1) = broadcast::channel::<()>(1);
        let stop_rx2 = stop_tx.subscribe();
        let stop_rx3 = stop_tx.subscribe();
        let stop_rx4 = stop_tx.subscribe();
        *stop_tx_arc.lock() = Some(stop_tx);

        let (tick_tx, mut tick_rx) = mpsc::channel::<types::Tick>(512);
        let (sym_tx,  sym_rx)      = mpsc::channel::<String>(8);
        *sym_tx_arc.lock() = Some(sym_tx);

        let agg = aggregator::spawn(app2.clone(), stop_rx2);
        let rec = recorder::spawn(data_dir, stop_rx3);

        let agg_tick_tx = agg.tick_tx.clone();
        let rec_tx      = rec.tx.clone();

        tokio::spawn(async move {
            let mut stop_fan = stop_rx4;
            loop {
                tokio::select! {
                    biased;
                    _ = stop_fan.recv() => break,
                    Some(tick) = tick_rx.recv() => {
                        let _ = agg_tick_tx.try_send(tick.clone());
                        let _ = rec_tx.try_send(RecorderMsg::Tick(tick));
                    }
                }
            }
        });

        let _ = agg.tf_tx.try_send(tf);
        *agg_arc.lock() = Some(agg);
        *rec_arc.lock() = Some(rec);

        let feed_cfg = FeedConfig { api_key, ws_url, symbol: symbol.clone() };
        fetcher::run_feed(app2.clone(), feed_cfg, stop_rx1, sym_rx, tick_tx).await;
    });

    let _ = app.emit("feed-started", serde_json::json!({
        "symbol": config.symbol, "timeframe": tf.label()
    }));


    Ok(())
}

#[tauri::command]
fn stop_feed(state: tauri::State<'_, AppState>) -> Result<(), String> {
    stop_inner(&state);
    Ok(())
}

#[tauri::command]
fn switch_symbol(
    app:    AppHandle,
    state:  tauri::State<'_, AppState>,
    symbol: String,
) -> Result<(), String> {
    let sym: String = symbol.chars()
        .filter(|c| c.is_alphanumeric() || *c == '/' || *c == ':' || *c == '_' || *c == '-')
        .collect();

    *state.symbol.lock() = sym.clone();

    if let Some(a) = state.agg_handle.lock().as_ref() {
        let _ = a.symbol_tx.try_send(sym.clone());
    }
    if let Some(r) = state.rec_handle.lock().as_ref() {
        let _ = r.tx.try_send(RecorderMsg::SymbolChange(sym.clone()));
    }

    let ws_url = state.ws_url.lock().clone();
    let tf      = *state.timeframe.lock();

    // Send symbol switch — non-blocking, no await needed
    if let Some(s) = state.symbol_tx.lock().as_ref() {
        let _ = s.try_send(sym.clone());
    }

    // Save config synchronously — no async needed
    if let Ok(store) = tauri_plugin_store::StoreBuilder::new(&app, "settings.json").build() {
        store.set("symbol",    serde_json::Value::String(sym));
        store.set("ws_url",    serde_json::Value::String(ws_url));
        store.set("timeframe", serde_json::Value::String(tf.label().to_string()));
        let _ = store.save();
    }

    Ok(())
}

#[tauri::command]
fn switch_timeframe(
    app:       AppHandle,
    state:     tauri::State<'_, AppState>,
    timeframe: String,
) -> Result<(), String> {
    let tf = Timeframe::from_str(&timeframe)
        .ok_or_else(|| format!("unknown timeframe: {}", timeframe))?;
    *state.timeframe.lock() = tf;
    if let Some(a) = state.agg_handle.lock().as_ref() {
        let _ = a.tf_tx.try_send(tf);
    }
    {
        let app2     = app.clone();
        let hist_key = state.api_key.lock().to_string();
        let hist_sym = state.symbol.lock().clone();
        tauri::async_runtime::spawn(async move {
        });
    }
    Ok(())
}

#[tauri::command]
fn get_config(state: tauri::State<'_, AppState>) -> Result<FeedConfigOut, String> {
    Ok(FeedConfigOut {
        ws_url:     state.ws_url.lock().clone(),
        symbol:     state.symbol.lock().clone(),
        timeframe:  state.timeframe.lock().label().to_string(),
        key_is_set: !state.api_key.lock().is_empty(),
    })
}

#[tauri::command]
fn get_data_dir(state: tauri::State<'_, AppState>) -> String {
    state.data_dir.display().to_string()
}

fn stop_inner(state: &AppState) {
    if let Some(tx) = state.stop_tx.lock().take() { let _ = tx.send(()); }
    *state.agg_handle.lock() = None;
    *state.rec_handle.lock() = None;
    *state.symbol_tx.lock()  = None;
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let data_dir = dirs::document_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("XAUUSDLive")
        .join("data");

    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::default().build())
        .manage(AppState {
            api_key:    Arc::new(PLMutex::new(Zeroizing::new(String::new()))),
            ws_url:     Arc::new(PLMutex::new("wss://ws.twelvedata.com/v1/quotes/price".into())),
            symbol:     Arc::new(PLMutex::new("XAU/USD".into())),
            timeframe:  Arc::new(PLMutex::new(Timeframe::M1)),
            stop_tx:    Arc::new(PLMutex::new(None)),
            symbol_tx:  Arc::new(PLMutex::new(None)),
            agg_handle: Arc::new(PLMutex::new(None)),
            rec_handle: Arc::new(PLMutex::new(None)),
            data_dir,
        })
        .invoke_handler(tauri::generate_handler![
            start_feed, stop_feed, switch_symbol, switch_timeframe,
            get_config, get_data_dir,
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(tokio::time::Duration::from_millis(600)).await;
                if let Ok(store) = tauri_plugin_store::StoreBuilder::new(
                    &handle, "settings.json"
                ).build() {
                    let has_key = store.get("has_api_key")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    if has_key {
                        let symbol = store.get("symbol")
                            .and_then(|v| v.as_str().map(String::from))
                            .unwrap_or_else(|| "XAU/USD".into());
                        let tf = store.get("timeframe")
                            .and_then(|v| v.as_str().map(String::from))
                            .unwrap_or_else(|| "M1".into());
                        let _ = handle.emit("auto-start-pending",
                            serde_json::json!({ "symbol": symbol, "timeframe": tf }));
                    } else {
                        let _ = handle.emit("show-settings", ());
                    }
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error running app")
}






