use crate::types::{Candle, PatternSignal, Signal, Timeframe};

const DOJI_RATIO:     f64 = 0.10;
const HAMMER_RATIO:   f64 = 2.0;
const MARUBOZU_RATIO: f64 = 0.05;

pub fn detect(candles: &[Candle], tf: Timeframe) -> Vec<PatternSignal> {
    if candles.is_empty() { return vec![]; }
    let mut out = Vec::new();
    let last = candles.len() - 1;
    let ts   = candles[last].close_time;

    if let Some(s) = doji(&candles[last], tf, ts)          { out.push(s); }
    if let Some(s) = hammer(&candles[last], tf, ts)        { out.push(s); }
    if let Some(s) = shooting_star(&candles[last], tf, ts) { out.push(s); }
    if let Some(s) = marubozu(&candles[last], tf, ts)      { out.push(s); }
    if let Some(s) = spinning_top(&candles[last], tf, ts)  { out.push(s); }

    if candles.len() >= 2 {
        let p = &candles[last - 1];
        let c = &candles[last];
        if let Some(s) = bullish_engulfing(p, c, tf, ts) { out.push(s); }
        if let Some(s) = bearish_engulfing(p, c, tf, ts) { out.push(s); }
        if let Some(s) = tweezer_top(p, c, tf, ts)       { out.push(s); }
        if let Some(s) = tweezer_bottom(p, c, tf, ts)    { out.push(s); }
    }

    if candles.len() >= 3 {
        let c0 = &candles[last - 2];
        let c1 = &candles[last - 1];
        let c2 = &candles[last];
        if let Some(s) = morning_star(c0, c1, c2, tf, ts)         { out.push(s); }
        if let Some(s) = evening_star(c0, c1, c2, tf, ts)         { out.push(s); }
        if let Some(s) = three_white_soldiers(c0, c1, c2, tf, ts) { out.push(s); }
        if let Some(s) = three_black_crows(c0, c1, c2, tf, ts)    { out.push(s); }
    }

    out
}

fn doji(c: &Candle, tf: Timeframe, ts: i64) -> Option<PatternSignal> {
    let r = c.range();
    if r == 0.0 { return None; }
    if c.body_size() / r < DOJI_RATIO {
        Some(PatternSignal { name: "Doji".into(), signal: Signal::Neutral,
            strength: 1.0 - (c.body_size() / r) / DOJI_RATIO, timeframe: tf, timestamp: ts })
    } else { None }
}

fn hammer(c: &Candle, tf: Timeframe, ts: i64) -> Option<PatternSignal> {
    let body = c.body_size(); let lower = c.lower_wick(); let upper = c.upper_wick();
    if body == 0.0 || lower < body * HAMMER_RATIO || upper > body * 0.5 { return None; }
    Some(PatternSignal { name: "Hammer".into(), signal: Signal::Bullish,
        strength: (lower / body / HAMMER_RATIO).min(1.0), timeframe: tf, timestamp: ts })
}

fn shooting_star(c: &Candle, tf: Timeframe, ts: i64) -> Option<PatternSignal> {
    let body = c.body_size(); let upper = c.upper_wick(); let lower = c.lower_wick();
    if body == 0.0 || upper < body * HAMMER_RATIO || lower > body * 0.5 { return None; }
    Some(PatternSignal { name: "Shooting Star".into(), signal: Signal::Bearish,
        strength: (upper / body / HAMMER_RATIO).min(1.0), timeframe: tf, timestamp: ts })
}

fn marubozu(c: &Candle, tf: Timeframe, ts: i64) -> Option<PatternSignal> {
    let r = c.range(); if r == 0.0 { return None; }
    if c.upper_wick() / r < MARUBOZU_RATIO && c.lower_wick() / r < MARUBOZU_RATIO {
        Some(PatternSignal { name: "Marubozu".into(),
            signal: if c.is_bullish() { Signal::Bullish } else { Signal::Bearish },
            strength: 0.9, timeframe: tf, timestamp: ts })
    } else { None }
}

fn spinning_top(c: &Candle, tf: Timeframe, ts: i64) -> Option<PatternSignal> {
    let r = c.range(); if r == 0.0 { return None; }
    if c.body_size() / r < 0.3 && (c.upper_wick() / r - c.lower_wick() / r).abs() < 0.15 {
        Some(PatternSignal { name: "Spinning Top".into(), signal: Signal::Neutral,
            strength: 0.5, timeframe: tf, timestamp: ts })
    } else { None }
}

fn bullish_engulfing(prev: &Candle, curr: &Candle, tf: Timeframe, ts: i64) -> Option<PatternSignal> {
    if prev.is_bullish() || !curr.is_bullish() { return None; }
    if curr.open <= prev.close && curr.close >= prev.open {
        Some(PatternSignal { name: "Bullish Engulfing".into(), signal: Signal::Bullish,
            strength: 0.85, timeframe: tf, timestamp: ts })
    } else { None }
}

fn bearish_engulfing(prev: &Candle, curr: &Candle, tf: Timeframe, ts: i64) -> Option<PatternSignal> {
    if !prev.is_bullish() || curr.is_bullish() { return None; }
    if curr.open >= prev.close && curr.close <= prev.open {
        Some(PatternSignal { name: "Bearish Engulfing".into(), signal: Signal::Bearish,
            strength: 0.85, timeframe: tf, timestamp: ts })
    } else { None }
}

fn tweezer_top(prev: &Candle, curr: &Candle, tf: Timeframe, ts: i64) -> Option<PatternSignal> {
    let tol = (prev.range() + curr.range()) / 2.0 * 0.02;
    if (prev.high - curr.high).abs() <= tol && prev.is_bullish() && !curr.is_bullish() {
        Some(PatternSignal { name: "Tweezer Top".into(), signal: Signal::Bearish,
            strength: 0.65, timeframe: tf, timestamp: ts })
    } else { None }
}

fn tweezer_bottom(prev: &Candle, curr: &Candle, tf: Timeframe, ts: i64) -> Option<PatternSignal> {
    let tol = (prev.range() + curr.range()) / 2.0 * 0.02;
    if (prev.low - curr.low).abs() <= tol && !prev.is_bullish() && curr.is_bullish() {
        Some(PatternSignal { name: "Tweezer Bottom".into(), signal: Signal::Bullish,
            strength: 0.65, timeframe: tf, timestamp: ts })
    } else { None }
}

fn morning_star(c0: &Candle, c1: &Candle, c2: &Candle, tf: Timeframe, ts: i64) -> Option<PatternSignal> {
    if c0.is_bullish() || !c2.is_bullish() { return None; }
    if c1.body_size() < c0.body_size() * 0.3 && c2.close > (c0.open + c0.close) / 2.0 {
        Some(PatternSignal { name: "Morning Star".into(), signal: Signal::Bullish,
            strength: 0.90, timeframe: tf, timestamp: ts })
    } else { None }
}

fn evening_star(c0: &Candle, c1: &Candle, c2: &Candle, tf: Timeframe, ts: i64) -> Option<PatternSignal> {
    if !c0.is_bullish() || c2.is_bullish() { return None; }
    if c1.body_size() < c0.body_size() * 0.3 && c2.close < (c0.open + c0.close) / 2.0 {
        Some(PatternSignal { name: "Evening Star".into(), signal: Signal::Bearish,
            strength: 0.90, timeframe: tf, timestamp: ts })
    } else { None }
}

fn three_white_soldiers(c0: &Candle, c1: &Candle, c2: &Candle, tf: Timeframe, ts: i64) -> Option<PatternSignal> {
    if c0.is_bullish() && c1.is_bullish() && c2.is_bullish()
        && c1.close > c0.close && c2.close > c1.close
        && c1.open > c0.open && c1.open < c0.close
        && c2.open > c1.open && c2.open < c1.close {
        Some(PatternSignal { name: "Three White Soldiers".into(), signal: Signal::Bullish,
            strength: 0.95, timeframe: tf, timestamp: ts })
    } else { None }
}

fn three_black_crows(c0: &Candle, c1: &Candle, c2: &Candle, tf: Timeframe, ts: i64) -> Option<PatternSignal> {
    if !c0.is_bullish() && !c1.is_bullish() && !c2.is_bullish()
        && c1.close < c0.close && c2.close < c1.close
        && c1.open < c0.open && c1.open > c0.close
        && c2.open < c1.open && c2.open > c1.close {
        Some(PatternSignal { name: "Three Black Crows".into(), signal: Signal::Bearish,
            strength: 0.95, timeframe: tf, timestamp: ts })
    } else { None }
}
