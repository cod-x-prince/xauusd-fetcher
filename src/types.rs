// src/types.rs  —  v0.2.0  FINAL
//
// v2 changes vs v1:
//   - RawTick, RawEvent, Tick::from_raw() all REMOVED
//     v2 reads fields directly from simd_json::OwnedValue in connection.rs
//     so serde struct deserialisation is no longer needed on the hot path.
//   - Only three things remain:
//       1. SubscribeMsg / SubscribeParams  (outgoing subscription)
//       2. FinnhubMsg / FinnhubTrade       (kept for future use)
//       3. Tick                            (normalised output)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ── Outgoing: subscription request ──────────────────────────────────────────

#[derive(Serialize)]
pub struct SubscribeMsg<'a> {
    pub action: &'a str,
    pub params: SubscribeParams<'a>,
}

#[derive(Serialize)]
pub struct SubscribeParams<'a> {
    pub symbols: &'a str,
}

// ── Finnhub wire types (kept for future extension) ───────────────────────────

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
pub struct FinnhubMsg {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub data:     Option<Vec<FinnhubTrade>>,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
pub struct FinnhubTrade {
    #[serde(rename = "p")] pub price:        f64,
    #[serde(rename = "s")] pub symbol:       String,
    #[serde(rename = "t")] pub timestamp_ms: Option<i64>,
    #[serde(rename = "v")] pub volume:       Option<f64>,
}

// ── Normalised output tick ───────────────────────────────────────────────────

/// One price update written to stdout (JSON Lines or CSV).
#[derive(Serialize, Debug)]
pub struct Tick {
    /// Wall-clock UTC time this machine received the WebSocket frame.
    /// Captured BEFORE parsing so it reflects true network arrival latency.
    pub timestamp: DateTime<Utc>,
    pub symbol:    String,
    pub bid:       f64,
    pub ask:       f64,
    /// ask − bid  (0.0 when only a single trade price is available)
    pub spread:    f64,
}