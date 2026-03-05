use std::{
    collections::HashMap,
    fs::{self, File, OpenOptions},
    io::{BufWriter, Write},
    path::PathBuf,
};
use chrono::Utc;
use tokio::sync::{broadcast, mpsc};
use crate::types::{Candle, Tick};

const FLUSH_MS:        u64   = 1_000;
const TICK_BUF_BYTES:  usize = 65_536;
const CANDLE_BUF_BYTES: usize = 16_384;

#[allow(dead_code)]
pub enum RecorderMsg {
    Tick(Tick),
    Candle(Candle),
    SymbolChange(String),
}

pub struct RecorderHandle {
    pub tx: mpsc::Sender<RecorderMsg>,
}

pub fn spawn(data_dir: PathBuf, mut stop_rx: broadcast::Receiver<()>) -> RecorderHandle {
    let (tx, mut rx) = mpsc::channel::<RecorderMsg>(2_048);

    tokio::spawn(async move {
        let mut state  = State::new(data_dir);
        let mut flush  = tokio::time::interval(
            tokio::time::Duration::from_millis(FLUSH_MS)
        );

        loop {
            tokio::select! {
                biased;
                _ = stop_rx.recv() => { state.flush_all(); break; }
                _ = flush.tick()   => { state.flush_all(); }
                Some(msg) = rx.recv() => match msg {
                    RecorderMsg::Tick(t)         => state.write_tick(&t),
                    RecorderMsg::Candle(c)       => state.write_candle(&c),
                    RecorderMsg::SymbolChange(s) => { state.flush_all(); state.set_symbol(s); }
                }
            }
        }
    });

    RecorderHandle { tx }
}

struct State {
    data_dir:       PathBuf,
    symbol:         String,
    date:           String,
    tick_writer:    Option<BufWriter<File>>,
    candle_writers: HashMap<String, BufWriter<File>>,
}

impl State {
    fn new(data_dir: PathBuf) -> Self {
        let _ = fs::create_dir_all(&data_dir);
        Self {
            data_dir,
            symbol:         "UNKNOWN".into(),
            date:           today(),
            tick_writer:    None,
            candle_writers: HashMap::new(),
        }
    }

    fn set_symbol(&mut self, sym: String) {
        self.symbol = sym.chars().filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-').collect();
        self.tick_writer    = None;
        self.candle_writers.clear();
    }

    fn write_tick(&mut self, t: &Tick) {
        self.ensure_tick();
        if let Some(w) = &mut self.tick_writer {
            let _ = writeln!(w, "{},{},{},{:.5},{:.5},{:.5},{}",
                t.timestamp_ms, t.timestamp, t.symbol,
                t.bid, t.ask, t.spread, t.latency_us);
        }
    }

    fn write_candle(&mut self, c: &Candle) {
        let tf = c.timeframe.label().to_string();
        self.ensure_candle(&tf);
        if let Some(w) = self.candle_writers.get_mut(&tf) {
            let _ = writeln!(w, "{},{},{:.5},{:.5},{:.5},{:.5},{},{}",
                c.open_time, c.close_time,
                c.open, c.high, c.low, c.close,
                c.tick_vol, tf);
        }
    }

    fn ensure_tick(&mut self) {
        let d = today();
        if d != self.date { self.date = d.clone(); self.tick_writer = None; self.candle_writers.clear(); }
        if self.tick_writer.is_some() { return; }
        let path = self.data_dir.join("ticks")
            .tap_mkdir()
            .join(format!("{}_{}.csv", self.symbol, self.date));
        if let Ok(f) = open_append(&path) {
            let mut w = BufWriter::with_capacity(TICK_BUF_BYTES, f);
            if path.metadata().map(|m| m.len()).unwrap_or(1) == 0 {
                let _ = writeln!(w, "timestamp_ms,timestamp,symbol,bid,ask,spread,latency_us");
            }
            self.tick_writer = Some(w);
        }
    }

    fn ensure_candle(&mut self, tf: &str) {
        if self.candle_writers.contains_key(tf) { return; }
        let path = self.data_dir.join("candles")
            .tap_mkdir()
            .join(format!("{}_{}_{}.csv", self.symbol, tf, self.date));
        if let Ok(f) = open_append(&path) {
            let mut w = BufWriter::with_capacity(CANDLE_BUF_BYTES, f);
            if path.metadata().map(|m| m.len()).unwrap_or(1) == 0 {
                let _ = writeln!(w, "open_time,close_time,open,high,low,close,tick_vol,timeframe");
            }
            self.candle_writers.insert(tf.to_string(), w);
        }
    }

    fn flush_all(&mut self) {
        if let Some(w) = &mut self.tick_writer { let _ = w.flush(); }
        for w in self.candle_writers.values_mut() { let _ = w.flush(); }
    }
}

impl Drop for State {
    fn drop(&mut self) { self.flush_all(); }
}

fn today() -> String { Utc::now().format("%Y-%m-%d").to_string() }

fn open_append(p: &PathBuf) -> std::io::Result<File> {
    OpenOptions::new().create(true).append(true).open(p)
}

trait TapMkdir { fn tap_mkdir(self) -> Self; }
impl TapMkdir for PathBuf {
    fn tap_mkdir(self) -> Self { let _ = fs::create_dir_all(&self); self }
}








