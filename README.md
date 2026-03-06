# ⚡ XAU/USD Trading Terminal

> A production-grade, ultra-low-latency market data terminal for **Gold (XAU/USD)** and other
> instruments — built from scratch in **Rust + Tauri v2 + Vanilla JS**.
>
> **30–50µs processing latency. Live canvas chart. Pattern detection. OS keychain security.**

```
┌─────────────────────────────────────────────────────────────────────────┐
│ Au TERMINAL │ XAU/USD │ BTC/USD │  1M  5M  15M  45M  1H  4H  │ ● LIVE │
├─────────────┴─────────────────────────────────────────────────────────-─┤
│              │  ╭─╮                                         70420.57   │
│  70420.57    │ ╱   ╲      ╭──╮   ╭─╮                                   │
│  -13.75      │╱     ╲    ╱    ╲  │ │╲    ╭──╮              70396.29   │
│  (-0.019%)   │       ╲──╯      ╲─╯ │ ╲──╯   ╲                         │
│  BID   │ ASK │                     │         ╲──            70334.34   │
│ 70420  │70420│                                                          │
│ SPREAD: 0.00 │▁▂▃▁▂▄▃▂▁▃▄▅▆▄▃▂▁▂▃▁  ← VELOCITY HISTOGRAM             │
├──────────────┤▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓  ← VOLUME PROFILE                 │
│ HIGH  │ LOW  │                                                          │
│70446  │70408 │  Hammer · BULLISH  ←── LIVE PATTERN BADGE               │
│ TPM:41│ 45µs │                                                          │
├──────────────┴──────────────────────────────────────────────────────────┤
│ FEED TWELVEDATA  SYMBOL BTC/USD  TF M1  TICKS 41  UTC 06:08:09         │
└─────────────────────────────────────────────────────────────────────────┘
```

![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange?style=flat-square&logo=rust)
![Tauri](https://img.shields.io/badge/Tauri-v2-blue?style=flat-square)
![Version](https://img.shields.io/badge/Version-4.2.0-gold?style=flat-square)
![Latency](https://img.shields.io/badge/Processing%20Latency-30µs-brightgreen?style=flat-square)
![License](https://img.shields.io/badge/License-MIT-green?style=flat-square)

---

## Table of Contents

- [The Journey — v0.1.0 → v4.2.0](#the-journey)
- [Architecture](#architecture)
- [Network Latency vs Processing Latency](#network-latency-vs-processing-latency)
- [How We Achieved 30µs](#how-we-achieved-30µs-processing-latency)
- [Features](#features)
- [Security Model](#security-model)
- [Repository Structure](#repository-structure)
- [Prerequisites](#prerequisites)
- [Running the UI](#running-the-ui)
- [Running the CLI](#running-the-cli)
- [Providers & Symbols](#providers--symbols)
- [Market Hours](#market-hours)
- [Roadmap](#roadmap)

---

## The Journey

### v0.1.0 — First connection

Basic Rust CLI, WebSocket to Twelve Data, raw JSON to stdout. No latency measurement, no reconnection. Ticks were slow (~1–5ms), connection dropped silently.

```
WebSocket frame → serde_json::from_str → println!
```

Problems: Nagle algorithm buffering small TCP frames up to 200ms. No error recovery. Timestamp captured after parsing measured nothing useful.

---

### v0.2.0 — Performance Engineering

Every major speed optimisation introduced here.

- `TCP_NODELAY` — eliminated artificial 40–200ms buffering per tick
- `simd-json` with AVX2/SSE4.2 — hardware-accelerated JSON parsing
- `core_affinity` — pinned hot path thread to dedicated CPU core
- `parking_lot::Mutex` — lock cost dropped from ~100ns to ~5ns
- Thread-local buffers — zero heap allocations in tick loop
- **Turbo Mode** — RAII Windows OS-level performance tuning:
  - `timeBeginPeriod(1)` — timer resolution 15ms → 0.5ms
  - `SetPriorityClass(REALTIME_PRIORITY_CLASS)` — pre-empts all processes
  - Auto-reverts on exit via Rust `Drop`

```
Before v0.2.0:   1,000–5,000µs per tick
Standard mode:     300–500µs
Turbo mode:        100–250µs
```

---

### v2.0.0 — Production CLI

Exponential backoff reconnection, `WebSocketConfig` frame size caps, dual NDJSON+CSV output, `biased tokio::select!`, `current_thread` runtime.

---

### v3.0.0 — Secure Cross-Platform UI

Full native desktop terminal. Rust backend runs WebSocket feed; Vanilla JS frontend renders on Canvas with sub-5ms tick-to-pixel latency.

- Tauri v2 — ~8MB binary, no Electron, no Node.js
- OS keychain API key storage (Windows DPAPI / macOS Keychain / Linux SecretService)
- `Zeroizing<String>` — API key bytes wiped from RAM on drop
- 4-layer hardware-accelerated Canvas chart
- Bloomberg terminal aesthetic — gold accents, Space Mono font

**Achieved: 30µs processing latency**

---

### v4.0.0 — Pattern Detection & Smart Aggregation

- Real-time candlestick pattern engine (12 patterns)
- Per-timeframe OHLCV aggregator with live candle formation
- Volume profile calculation
- Symbol switching without reconnection
- Auto-start on launch with saved config
- Live pattern badge in header

---

### v4.1.0 — Historical Candle Cache

- REST API pre-fetch on connect — 99 historical 1min candles per symbol
- Parallel background fetch for XAU/USD + BTC/USD
- Once-per-session cache — zero rate limit hammering
- Candle resampling — 1min REST data resampled to any TF
- Cache restores instantly on symbol/TF switch

---

### v4.2.0 — Velocity Histogram & Chart Polish (Current)

- Tick velocity histogram — rolling ticks/sec, color-coded green/yellow/red
- New timeframes: 45M and 4H
- Dynamic candle width based on visible count
- Dual price axis (left + right)
- OHLCV crosshair tooltip with P&L and %
- Time axis labels
- Live candle deduplication by bucket-floor comparison
- Enlarged live price pill

---

## Architecture

```
WebSocket Feed (Twelve Data)
        │
        ▼
┌─────────────────────────────────────────┐
│  fetcher.rs — async WebSocket client    │
│  • TCP_NODELAY (Nagle disabled)         │
│  • Zeroizing<String> for API key/URL    │
│  • biased tokio::select! for control    │
│  • thread_local! zero-alloc hot path    │
│  • exponential backoff reconnect        │
└────────────────┬────────────────────────┘
                 │ mpsc tick channel
                 ▼
┌─────────────────────────────────────────┐
│  aggregator.rs — OHLCV candle engine    │
│  • Per-TF buffer map                    │
│  • 12-pattern candlestick detector      │
│  • Volume profile calculation           │
│  • broadcast stop signal                │
│  • Tauri event emitter → JS             │
└────────────────┬────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────┐
│  historical.rs — REST cache layer       │
│  • Parallel fetch XAU/USD + BTC/USD     │
│  • Once per session, no rate hammering  │
│  • hist-cache event → JS Map            │
└────────────────┬────────────────────────┘
                 │ Tauri events
                 ▼
┌─────────────────────────────────────────┐
│  index.html — 4-layer Canvas UI         │
│                                         │
│  cv-grid    price axes, grid lines      │
│  cv-candles OHLCV bars + volume         │
│  cv-live    forming candle + price line │
│  cv-cross   crosshair + OHLCV tooltip   │
│                                         │
│  + velocity histogram                   │
│  + resample() for TF switching          │
│  + histCache Map (symbol → candles)     │
└─────────────────────────────────────────┘
```

---

## Network Latency vs Processing Latency

The most critical concept in this project — they are completely different things requiring completely different solutions.

### The Full Stack

```
Price changes at exchange
        │  exchange matching engine (~µs)
        ▼
Twelve Data processes + pushes WebSocket
        │  provider internal latency (~1–10ms)
        ▼
Internet transit
        │  NETWORK LATENCY (~20–80ms) ← physics, cannot be reduced
        ▼
Your NIC receives frame
        │  Windows TCP stack (~100µs floor)
        ▼
tokio runtime delivers to app
        │
        ▼  ← Instant::now() captured HERE
PROCESSING LATENCY: 30–50µs  ← what our code controls
        │  simd-json parse: ~5–15µs
        │  channel dispatch: ~10–20µs
        │  Tauri IPC: ~1–3ms
        ▼
JS canvas redraw: ~2–5ms

Total end-to-end: ~25–90ms
  Network:    ~20–80ms  (physics — irreducible without co-location)
  Processing: ~30–50µs  (our code — highly optimised)
  Render:     ~2–5ms    (GPU compositing)
```

**The 30µs means our code adds virtually zero overhead on top of what the OS delivers.** The software floor without kernel bypass (DPDK/io_uring).

---

## How We Achieved 30µs Processing Latency

### 1. TCP_NODELAY — Biggest Single Win

Nagle's algorithm holds small TCP segments for up to 200ms waiting to batch them. A WebSocket tick is ~50–100 bytes. Without this fix every tick was delayed artificially.

```rust
// Unwrap 3 layers of TLS to reach raw TcpStream
tls.get_ref().get_ref().get_ref().set_nodelay(true)
```

**Impact: eliminated up to 200ms of artificial delay per tick.**

### 2. Timestamp Before Parse

```rust
let parse_start = Instant::now();       // BEFORE serde_json
let raw: RawTick = serde_json::from_str(&text)?;
// latency_us = parse_start.elapsed().as_micros()
```

### 3. Zero-Copy JSON + Thread-Local Buffer

```rust
thread_local! {
    static PARSE_BUF: RefCell<Vec<u8>> = RefCell::new(Vec::with_capacity(1024));
}
```

Zero heap allocations per tick. `simd-json` parses with AVX2/SSE4 vector instructions in-place — no `String::from()`, no intermediate allocation.

### 4. `biased` tokio `select!`

```rust
tokio::select! {
    biased;
    _ = stop.recv()  => { /* shutdown */ }
    msg = ws.next()  => { /* tick */     }
}
```

Control signals always evaluated before data. No starvation under high tick volume.

### 5. `parking_lot::Mutex`

Lock/unlock: `std::sync::Mutex` ~100ns → `parking_lot` ~5ns. Userspace spinlock for the uncontended case.

### 6. `current_thread` Tokio Runtime

No work-stealing thread migration. Single connection = no throughput benefit from multi-thread, only cache invalidation overhead.

### 7. Release Profile

```toml
[profile.release]
opt-level     = 3       # maximum vectorisation + inlining
lto           = true    # inline simd-json internals into our parse call
codegen-units = 1       # whole-program analysis
panic         = "abort" # no unwinding tables
strip         = true    # smaller binary, faster OS load
```

### 8. 4-Layer Canvas — Frontend Zero-Reflow

```javascript
requestAnimationFrame(drawLive); // ~60fps, GPU-composited
```

4 independent canvas layers — mousemove only redraws the crosshair layer. No DOM reflow, no layout recalculation.

---

## Features

### Chart

- Candlestick and line modes
- 6 timeframes: 1M · 5M · 15M · 45M · 1H · 4H
- Dynamic candle width
- Dual price axis (left + right)
- OHLCV crosshair tooltip with P&L and %change
- Live price pill on right axis
- Volume bars with opacity scaling
- Tick velocity histogram (green=normal / yellow=elevated / red=spike)

### Data

- Historical candle cache — 99 candles pre-fetched on connect
- Candle resampling — 1min REST data resampled to any TF
- Cache persists per session — instant symbol switching
- Live candle deduplication by bucket-floor

### Pattern Detection (12 patterns)

| Bullish           | Bearish           | Neutral      |
| ----------------- | ----------------- | ------------ |
| Hammer            | Shooting Star     | Doji         |
| Marubozu          | Bearish Marubozu  | Spinning Top |
| Morning Star      | Evening Star      |              |
| Bullish Engulfing | Bearish Engulfing |              |
| Tweezer Bottom    | Tweezer Top       |              |

- Live badge in header on pattern detection
- Timestamped pattern signal list

### Security

- API key stored in OS keychain — never touches disk as plaintext
- `Zeroizing<String>` — key bytes zeroed on drop
- `get_config()` returns `key_is_set: bool` only — key never returns to JS

---

## Security Model

```
User types API key
        │
        ▼  invoke("start_feed", { api_key })
Rust start_feed() — stores as Zeroizing<String>
        │
        ▼  Zeroizing::new(format!("{}?apikey={}", url, key))
WebSocket connects
        │
        ▼  drop(url) — bytes overwritten with 0x00 immediately
Feed runs — key never referenced again
        │
        ▼  JS calls get_config()
Returns { ws_url, symbol, key_is_set: true }
        └── api_key field does not exist in response
```

| Layer      | Mechanism                                  | Protects Against                |
| ---------- | ------------------------------------------ | ------------------------------- |
| In-memory  | `Zeroizing<String>`                        | Key lingering in RAM after drop |
| At rest    | OS Keychain (DPAPI/Keychain/SecretService) | Key readable from disk          |
| IPC        | `key_is_set: bool` only                    | Key leaking to JS renderer      |
| URL string | `Zeroizing::new()` + `drop()`              | WebSocket URL in heap           |
| Git        | `.gitignore` blocks `.env`, `.stronghold`  | Accidental commit               |
| Frontend   | Key never written to `localStorage`        | Key in browser DevTools         |

---

## Repository Structure

```
xauusd-fetcher/
│
├── cli/                        Rust CLI tick fetcher
│   └── src/
│       ├── main.rs             Entry point + Turbo Mode
│       ├── connection.rs       WebSocket hot loop (simd-json, TCP_NODELAY)
│       ├── types.rs            Tick, RawTick, SubscribeMsg
│       ├── output.rs           BufWriter stdout
│       └── turbo.rs            RAII Windows perf tuning
│
└── ui/                         Tauri v2 desktop terminal
    ├── src/
    │   └── index.html          Complete UI — Canvas chart, no framework
    └── src-tauri/
        └── src/
            ├── lib.rs          Tauri commands, AppState
            ├── fetcher.rs      WebSocket feed (zero-alloc)
            ├── aggregator.rs   OHLCV aggregation + pattern detection
            ├── historical.rs   REST historical fetch + session cache
            ├── recorder.rs     CSV data recorder
            ├── patterns.rs     Candlestick pattern engine
            └── types.rs        Tick, Candle, Timeframe
```

---

## Prerequisites

| Tool     | Min Version | Install                                         |
| -------- | ----------- | ----------------------------------------------- |
| Rust     | 1.75        | `rustup update stable`                          |
| Node.js  | 18          | [nodejs.org](https://nodejs.org)                |
| WebView2 | any         | Pre-installed Windows 10/11                     |
| API Key  | —           | [Twelve Data](https://twelvedata.com) free tier |

---

## Running the UI

```powershell
cd ui
npm install         # first time only (~1 min)
npm run dev         # dev mode with hot reload (~15s first compile)
npm run build       # production → ui\src-tauri\target\release\xauusd-ui.exe
```

1. Click **CONFIG** top right
2. Enter your Twelve Data API key
3. Select symbol in the header
4. Click **CONNECT**

> Use **BTC/USD** for 24/7 testing — gold only streams Mon–Fri market hours.

---

## Running the CLI

```powershell
cd cli
cargo build --release

.\target\release\xauusd-fetcher.exe --verbose

# Save to file
$ts = Get-Date -Format "yyyy-MM-dd_HHmmss"
.\target\release\xauusd-fetcher.exe | Tee-Object "ticks_$ts.ndjson"
```

Sample output:

```json
{
  "timestamp": "2026-03-06T06:08:09Z",
  "symbol": "BTC/USD",
  "bid": 70420.57,
  "ask": 70420.57,
  "spread": 0.0,
  "latency_us": 45
}
```

---

## Providers & Symbols

| Provider    | Symbol    | Description | Hours   |
| ----------- | --------- | ----------- | ------- |
| Twelve Data | `XAU/USD` | Gold Spot   | Mon–Fri |
| Twelve Data | `XAG/USD` | Silver Spot | Mon–Fri |
| Twelve Data | `BTC/USD` | Bitcoin     | 24/7    |
| Twelve Data | `ETH/USD` | Ethereum    | 24/7    |
| Twelve Data | `EUR/USD` | Euro/Dollar | Mon–Fri |

> Historical REST cache available for `XAU/USD` and `BTC/USD` only on free tier.
> Other symbols start from zero and accumulate candles live during the session.

---

## Market Hours

| Session       | UTC       | IST       |
| ------------- | --------- | --------- |
| Sydney open   | Sun 22:00 | Mon 03:30 |
| Tokyo open    | Mon 00:00 | Mon 05:30 |
| London open   | Mon 08:00 | Mon 13:30 |
| New York open | Mon 13:00 | Mon 18:30 |
| Market close  | Fri 22:00 | Sat 03:30 |

---

## Version History

| Version | Branch                | Latency     | What Changed                             |
| ------- | --------------------- | ----------- | ---------------------------------------- |
| v0.1.0  | `main`                | ~1–5ms      | Basic WebSocket CLI                      |
| v0.2.0  | `v2/ultra-fast`       | 100–500µs   | SIMD, core pinning, Turbo Mode           |
| v2.0.0  | `v2/ultra-fast`       | 100–300µs   | TCP_NODELAY, backoff, production README  |
| v3.0.0  | `v3/secure-ui`        | **30µs**    | Tauri UI, OS keychain, Canvas chart      |
| v4.0.0  | `v4/candles-patterns` | **30µs**    | Pattern detection, OHLCV aggregator      |
| v4.1.0  | `v4/candles-patterns` | **30µs**    | Historical cache, TF resampling          |
| v4.2.0  | `v4/candles-patterns` | **30–50µs** | Velocity histogram, M45/4H, chart polish |

---

## Roadmap

| Version | Status     | Features                                                 |
| ------- | ---------- | -------------------------------------------------------- |
| v4.3    | 🔨 Next    | UI polish (Bloomberg aesthetic), pattern outcome tracker |
| v5.0    | 📋 Planned | Multi-feed correlation (DXY + XAU/USD), alert engine     |
| v6.0    | 📋 Planned | Order execution — OANDA/Alpaca demo API                  |

---

## Disclaimer

For educational and research purposes only. Free-tier WebSocket feeds have data quality limitations compared to institutional vendors (Bloomberg, Refinitiv). Do not use as the sole basis for live trading decisions.

---

_Built with Rust · Tauri v2 · Vanilla JS · Twelve Data_
