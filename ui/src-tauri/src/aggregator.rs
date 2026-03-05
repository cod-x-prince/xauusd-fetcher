use std::collections::{HashMap, VecDeque};
use tauri::{AppHandle, Emitter};
use tokio::sync::{broadcast, mpsc};
use crate::patterns;
use crate::types::{Candle, Tick, Timeframe, VolumeBucket, VolumeProfile};

const MAX_CANDLES:     usize = 1_000;
const MAX_VOL_BUCKETS: usize = 200;
const VALUE_AREA_PCT:  f64   = 0.70;

struct CandleBuffer {
    tf:      Timeframe,
    candles: VecDeque<Candle>,
    live:    Option<Candle>,
}

impl CandleBuffer {
    fn new(tf: Timeframe) -> Self {
        Self { tf, candles: VecDeque::with_capacity(MAX_CANDLES), live: None }
    }

    #[inline(always)]
    fn process_tick(&mut self, price: f64, ts_ms: i64) -> Option<Candle> {
        let bucket = (ts_ms / self.tf.duration_ms()) * self.tf.duration_ms();
        match &mut self.live {
            None => {
                self.live = Some(Candle::new(price, bucket, self.tf));
                None
            }
            Some(live) if live.open_time == bucket => {
                live.update(price);
                None
            }
            Some(_) => {
                let mut closed = self.live.take().unwrap();
                closed.closed = true;
                if self.candles.len() >= MAX_CANDLES { self.candles.pop_front(); }
                self.candles.push_back(closed.clone());
                self.live = Some(Candle::new(price, bucket, self.tf));
                Some(closed)
            }
        }
    }

    fn live_candle(&self) -> Option<&Candle> { self.live.as_ref() }

    fn last_closed(&self, n: usize) -> Vec<Candle> {
        let len = self.candles.len();
        if len == 0 { return vec![]; }
        let start = if len > n { len - n } else { 0 };
        self.candles.range(start..).cloned().collect()
    }

    fn all_candles(&self, max: usize) -> Vec<Candle> {
        let mut out: Vec<Candle> = self.candles.iter().rev().take(max).cloned().collect();
        out.reverse();
        if let Some(live) = &self.live { out.push(live.clone()); }
        out
    }
}

fn build_volume_profile(candles: &[Candle]) -> Option<VolumeProfile> {
    if candles.len() < 2 { return None; }
    let min_p = candles.iter().map(|c| c.low).fold(f64::INFINITY, f64::min);
    let max_p = candles.iter().map(|c| c.high).fold(f64::NEG_INFINITY, f64::max);
    let range = max_p - min_p;
    if range == 0.0 { return None; }

    let raw_size  = range / 80.0;
    let magnitude = 10f64.powf(raw_size.log10().floor());
    let tick_size = (raw_size / magnitude).ceil() * magnitude;
    let n = ((range / tick_size).ceil() as usize + 1).min(MAX_VOL_BUCKETS);

    let mut buckets = vec![0u64; n];
    for c in candles {
        let idx = ((c.close - min_p) / tick_size) as usize;
        buckets[idx.min(n - 1)] += c.tick_vol;
    }

    let total: u64 = buckets.iter().sum();
    if total == 0 { return None; }

    let poc_idx = buckets.iter().enumerate().max_by_key(|(_, &v)| v).map(|(i, _)| i).unwrap_or(0);
    let poc     = min_p + poc_idx as f64 * tick_size + tick_size / 2.0;

    let target = (total as f64 * VALUE_AREA_PCT) as u64;
    let mut va_vol = buckets[poc_idx];
    let mut lo = poc_idx;
    let mut hi = poc_idx;
    while va_vol < target && (lo > 0 || hi < n - 1) {
        let add_lo = if lo > 0 { buckets[lo - 1] } else { 0 };
        let add_hi = if hi < n - 1 { buckets[hi + 1] } else { 0 };
        if add_lo >= add_hi && lo > 0 { lo -= 1; va_vol += buckets[lo]; }
        else if hi < n - 1 { hi += 1; va_vol += buckets[hi]; }
        else if lo > 0 { lo -= 1; va_vol += buckets[lo]; }
        else { break; }
    }

    let vah = min_p + (hi + 1) as f64 * tick_size;
    let val = min_p + lo as f64 * tick_size;

    let result: Vec<VolumeBucket> = buckets.iter().enumerate()
        .filter(|(_, &v)| v > 0)
        .map(|(i, &v)| VolumeBucket {
            price_mid: min_p + i as f64 * tick_size + tick_size / 2.0,
            tick_vol: v,
            is_poc: i == poc_idx,
            in_va: i >= lo && i <= hi,
        })
        .collect();

    Some(VolumeProfile { buckets: result, poc, vah, val, tick_size })
}

pub struct AggregatorHandle {
    pub tick_tx:   mpsc::Sender<Tick>,
    pub symbol_tx: mpsc::Sender<String>,
    pub tf_tx:     mpsc::Sender<Timeframe>,
}

pub fn spawn(app: AppHandle, mut stop_rx: broadcast::Receiver<()>) -> AggregatorHandle {
    let (tick_tx,   mut tick_rx)   = mpsc::channel::<Tick>(512);
    let (symbol_tx, mut symbol_rx) = mpsc::channel::<String>(8);
    let (tf_tx,     mut tf_rx)     = mpsc::channel::<Timeframe>(8);

    tokio::spawn(async move {
        let tfs = [Timeframe::S5, Timeframe::S15, Timeframe::M1,
                   Timeframe::M5, Timeframe::M15, Timeframe::M30, Timeframe::H1];
        let mut buffers: HashMap<Timeframe, CandleBuffer> =
            tfs.iter().map(|&tf| (tf, CandleBuffer::new(tf))).collect();
        let mut active_tf = Timeframe::M1;

        loop {
            tokio::select! {
                biased;
                _ = stop_rx.recv() => break,

                Some(new_tf) = tf_rx.recv() => {
                    active_tf = new_tf;
                    // Only emit candle-history if we have live-aggregated candles
                    // Otherwise historical::fetch_and_emit handles it
                    if let Some(buf) = buffers.get(&active_tf) {
                        if !buf.candles.is_empty() {
                            let _ = app.emit("candle-history", buf.all_candles(500));
                        }
                    }
                }

                Some(_) = symbol_rx.recv() => {
                    for buf in buffers.values_mut() {
                        buf.candles.clear();
                        buf.live = None;
                    }
                    // Do not emit empty candle-history here —
                    // historical::fetch_and_emit will populate the chart
                }

                Some(tick) = tick_rx.recv() => {
                    let price = tick.bid;
                    let ts_ms = tick.timestamp_ms;

                    for (&tf, buf) in buffers.iter_mut() {
                        if let Some(closed) = buf.process_tick(price, ts_ms) {
                            if tf == active_tf {
                                let _ = app.emit("candle-close", &closed);
                                let recent = buf.last_closed(5);
                                for pat in patterns::detect(&recent, tf) {
                                    let _ = app.emit("pattern", &pat);
                                }
                                if let Some(vp) = build_volume_profile(&buf.last_closed(200)) {
                                    let _ = app.emit("volume-profile", &vp);
                                }
                            }
                        }
                        if tf == active_tf {
                            if let Some(live) = buf.live_candle() {
                                let _ = app.emit("candle-update", live);
                            }
                        }
                    }
                }
            }
        }
    });

    AggregatorHandle { tick_tx, symbol_tx, tf_tx }
}




