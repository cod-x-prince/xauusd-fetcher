// src/types.rs
// Wire types (Twelve Data format) and our normalised output Tick.

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

// ── Finnhub incoming tick ────────────────────────────────────────────────────
#[allow(dead_code)]

/// Raw price event from Finnhub WebSocket.
/// Shape: {"type":"trade","data":[{"p":1925.5,"s":"OANDA:XAU_USD","t":1234567890,"v":1}]}
#[derive(Deserialize, Debug)]
pub struct FinnhubMsg {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub data: Option<Vec<FinnhubTrade>>,
}

#[derive(Deserialize, Debug)]
pub struct FinnhubTrade {
    #[allow(dead_code)] #[serde(rename = "p")] pub price: f64,
    #[allow(dead_code)] #[serde(rename = "s")] pub symbol: String,
    #[allow(dead_code)] #[serde(rename = "t")] pub timestamp_ms: Option<i64>,
    #[allow(dead_code)] #[serde(rename = "v")] pub volume: Option<f64>,
}

/// Matches the JSON shape Twelve Data sends on a "price" event.
/// Twelve Data sends a single `price` field (not bid/ask) for most instruments.
/// bid/ask are kept as Option for forward-compatibility with quote endpoints.
#[derive(Deserialize, Debug)]
pub struct RawTick {
    #[allow(dead_code)] pub event:  String,
    pub symbol:   String,
    /// Single trade price (crypto, stocks, most forex on Basic plan)
    pub price:    Option<f64>,
    /// Bid price — only on premium quote streams
    pub bid:      Option<f64>,
    /// Ask price — only on premium quote streams
    pub ask:      Option<f64>,
    #[allow(dead_code)] pub timestamp: Option<i64>,
}

/// Generic event envelope — used for heartbeat / subscribe-status frames.
#[derive(Deserialize, Debug)]
pub struct RawEvent {
    #[allow(dead_code)] pub event:   String,
    pub message: Option<String>,
    pub status:  Option<String>,
}

// ── Outgoing: normalised tick written to stdout ──────────────────────────────

#[derive(Serialize, Debug)]
pub struct Tick {
    /// Wall-clock UTC time this machine received the frame — not provider time.
    /// Captured before parsing so it reflects true network arrival latency.
    pub timestamp: DateTime<Utc>,
    pub symbol:    String,
    pub bid:       f64,
    pub ask:       f64,
    /// Ask − bid (spread in price units, e.g. USD per troy oz).
    pub spread:    f64,
}

impl Tick {
    /// Convert a raw provider tick into our normalised form.
    /// Handles both:
    ///   - Single price format: {"event":"price","price":66866.0}  ← Twelve Data default
    ///   - Bid/ask format:      {"event":"price","bid":x,"ask":y}  ← premium quote streams
    pub fn from_raw(raw: &RawTick, received_at: DateTime<Utc>) -> Option<Self> {
        // Prefer explicit bid/ask; fall back to single price field
        let (bid, ask, spread) = match (raw.bid, raw.ask, raw.price) {
            (Some(b), Some(a), _) => (b, a, (a - b).abs()),   // true bid/ask
            (_, _, Some(p))       => (p, p, 0.0),              // single price
            _                     => return None,              // no price data
        };
        Some(Tick {
            timestamp: received_at,
            symbol:    raw.symbol.clone(),
            bid,
            ask,
            spread,
        })
    }
}