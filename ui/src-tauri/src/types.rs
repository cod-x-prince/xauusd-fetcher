use serde::{Deserialize, Serialize};

// ── Raw tick ──────────────────────────────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Tick {
    pub timestamp_ms: i64,
    pub timestamp:    String,
    pub symbol:       String,
    pub bid:          f64,
    pub ask:          f64,
    pub spread:       f64,
    pub latency_us:   u64,
}

// ── Timeframe ─────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Timeframe {
    S5, S15, M1, M5, M15, M30, H1,
}

impl Timeframe {
    pub fn duration_ms(self) -> i64 {
        match self {
            Timeframe::S5  =>      5_000,
            Timeframe::S15 =>     15_000,
            Timeframe::M1  =>     60_000,
            Timeframe::M5  =>    300_000,
            Timeframe::M15 =>    900_000,
            Timeframe::M30 =>  1_800_000,
            Timeframe::H1  =>  3_600_000,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Timeframe::S5  => "S5",
            Timeframe::S15 => "S15",
            Timeframe::M1  => "M1",
            Timeframe::M5  => "M5",
            Timeframe::M15 => "M15",
            Timeframe::M30 => "M30",
            Timeframe::H1  => "H1",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "S5"  => Some(Timeframe::S5),
            "S15" => Some(Timeframe::S15),
            "M1"  => Some(Timeframe::M1),
            "M5"  => Some(Timeframe::M5),
            "M15" => Some(Timeframe::M15),
            "M30" => Some(Timeframe::M30),
            "H1"  => Some(Timeframe::H1),
            _     => None,
        }
    }
}

// ── Candle ────────────────────────────────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Candle {
    pub open_time:  i64,
    pub close_time: i64,
    pub open:       f64,
    pub high:       f64,
    pub low:        f64,
    pub close:      f64,
    pub tick_vol:   u64,
    pub timeframe:  Timeframe,
    pub closed:     bool,
}

impl Candle {
    pub fn new(price: f64, open_time: i64, tf: Timeframe) -> Self {
        Self {
            open_time,
            close_time: open_time + tf.duration_ms() - 1,
            open: price, high: price, low: price, close: price,
            tick_vol: 1, timeframe: tf, closed: false,
        }
    }

    #[inline(always)]
    pub fn update(&mut self, price: f64) {
        if price > self.high { self.high = price; }
        if price < self.low  { self.low  = price; }
        self.close    = price;
        self.tick_vol += 1;
    }

    pub fn is_bullish(&self) -> bool { self.close >= self.open }
    pub fn body_size(&self)   -> f64 { (self.close - self.open).abs() }
    pub fn range(&self)       -> f64 { self.high - self.low }
    pub fn upper_wick(&self)  -> f64 { self.high - self.close.max(self.open) }
    pub fn lower_wick(&self)  -> f64 { self.close.min(self.open) - self.low }
}

// ── Pattern ───────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Signal { Bullish, Bearish, Neutral }

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct PatternSignal {
    pub name:      String,
    pub signal:    Signal,
    pub strength:  f64,
    pub timeframe: Timeframe,
    pub timestamp: i64,
}

// ── Volume profile ────────────────────────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct VolumeBucket {
    pub price_mid: f64,
    pub tick_vol:  u64,
    pub is_poc:    bool,
    pub in_va:     bool,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct VolumeProfile {
    pub buckets:   Vec<VolumeBucket>,
    pub poc:       f64,
    pub vah:       f64,
    pub val:       f64,
    pub tick_size: f64,
}

// ── Feed config ───────────────────────────────────────────────────────────────

#[derive(Clone, Deserialize, Debug)]
pub struct FeedConfigIn {
    pub api_key:   String,
    pub ws_url:    String,
    pub symbol:    String,
    pub timeframe: Option<String>,
}

#[derive(Clone, Serialize, Debug)]
pub struct FeedConfigOut {
    pub ws_url:     String,
    pub symbol:     String,
    pub timeframe:  String,
    pub key_is_set: bool,
}
