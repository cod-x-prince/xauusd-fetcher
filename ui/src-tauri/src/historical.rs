use chrono::NaiveDateTime;
use serde::Deserialize;
use tauri::{AppHandle, Emitter};
use crate::types::{Candle, Timeframe};

const OUTPUT_SIZE: u32 = 100;

#[derive(Deserialize)]
struct TdResponse {
    status: Option<String>,
    values: Option<Vec<TdCandle>>,
}

#[derive(Deserialize)]
struct TdCandle {
    datetime: String,
    open:     String,
    high:     String,
    low:      String,
    close:    String,
    volume:   Option<String>,
}

fn tf_to_interval(tf: Timeframe) -> &'static str {
    match tf {
        // Twelve Data REST does not support sub-minute intervals
        // Fall back to 1min so the chart always gets historical context
        Timeframe::S5  => "1min",
        Timeframe::S15 => "1min",
        Timeframe::M1  => "1min",
        Timeframe::M5  => "5min",
        Timeframe::M15 => "15min",
        Timeframe::M30 => "30min",
        Timeframe::H1  => "1h",
    }
}

fn parse_ts(dt: &str) -> i64 {
    NaiveDateTime::parse_from_str(dt, "%Y-%m-%d %H:%M:%S")
        .map(|n| n.and_utc().timestamp_millis())
        .unwrap_or(0)
}

pub async fn fetch_and_emit(
    app:     &AppHandle,
    api_key: &str,
    symbol:  &str,
    tf:      Timeframe,
) {
    let interval = tf_to_interval(tf);
    let sym_encoded = symbol.replace('/', "%2F");
    let url = format!(
        "https://api.twelvedata.com/time_series?symbol={}&interval={}&outputsize={}&apikey={}",
        sym_encoded, interval, OUTPUT_SIZE, api_key
    );

    let resp = match reqwest::get(&url).await {
        Ok(r)  => r,
        Err(e) => { eprintln!("[historical] request failed: {e}"); return; }
    };

    let text = match resp.text().await {
        Ok(t)  => t,
        Err(e) => { eprintln!("[historical] read failed: {e}"); return; }
    };
    eprintln!("[historical] raw response (first 300 chars): {}", &text[..text.len().min(300)]);
    let body: TdResponse = match serde_json::from_str(&text) {
        Ok(b)  => b,
        Err(e) => { eprintln!("[historical] parse failed: {e}"); return; }
    }; let body = body;

    if body.status.as_deref() != Some("ok") {
        eprintln!("[historical] API status not ok — full response logged above");
        return;
    }

    let values = match body.values {
        Some(v) => v,
        None    => return,
    };

    // Values come newest-first from Twelve Data — reverse to chronological
    let mut candles: Vec<Candle> = values.iter().rev().filter_map(|v| {
        let open  = v.open.parse::<f64>().ok()?;
        let high  = v.high.parse::<f64>().ok()?;
        let low   = v.low.parse::<f64>().ok()?;
        let close = v.close.parse::<f64>().ok()?;
        let tick_vol = v.volume.as_deref()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(1);
        let open_time  = parse_ts(&v.datetime);
        let close_time = open_time + tf.duration_ms() - 1;
        Some(Candle {
            open_time, close_time,
            open, high, low, close,
            tick_vol, timeframe: tf,
            closed: true,
        })
    }).collect();

    // Drop the last entry — it is the still-forming candle
    if candles.len() > 1 { candles.pop(); }

    let count = candles.len();
    let _ = app.emit("candle-history", candles);
    eprintln!("[historical] emitted {} candles for {} {}", count, symbol, interval);
}





