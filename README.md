# ⚡ XAU/USD Ultra-Low-Latency Market Data Fetcher

> Sub-millisecond **Gold (XAU/USD)** price streaming built in **Rust** — WebSocket, TCP_NODELAY, zero-copy JSON parsing, and exponential-backoff reconnection.

```
{"timestamp":"2026-03-02T00:01:23.441Z","symbol":"XAU/USD","bid":2938.42,"ask":2938.42,"spread":0.0}
{"timestamp":"2026-03-02T00:01:23.887Z","symbol":"XAU/USD","bid":2938.45,"ask":2938.45,"spread":0.0}
{"timestamp":"2026-03-02T00:01:24.102Z","symbol":"XAU/USD","bid":2938.39,"ask":2938.39,"spread":0.0}
```

![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange?style=flat-square&logo=rust)
![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20Linux%20%7C%20macOS-blue?style=flat-square)
![Version](https://img.shields.io/badge/Version-2.0.0-purple?style=flat-square)
![License](https://img.shields.io/badge/License-MIT-green?style=flat-square)

---

## 📋 Table of Contents

- [Features](#-features)
- [Architecture](#-architecture)
- [Project Structure](#-project-structure)
- [Prerequisites](#-prerequisites)
- [Installation](#-installation)
- [Configuration](#-configuration)
- [Running](#-running)
- [Output Formats](#-output-formats)
- [Saving Data to File](#-saving-data-to-file)
- [Supported Providers](#-supported-providers)
- [Supported Symbols](#-supported-symbols)
- [Latency Optimisations](#-latency-optimisations)
- [Market Hours](#-market-hours)
- [Troubleshooting](#-troubleshooting)
- [Roadmap](#-roadmap)

---

## ✨ Features

| Feature                            | Details                                                        |
| ---------------------------------- | -------------------------------------------------------------- |
| 🚀 **Sub-millisecond processing**  | Timestamp captured before JSON parsing — true arrival latency  |
| 🔌 **Persistent WebSocket stream** | No polling, no REST overhead                                   |
| 🔄 **Auto-reconnect**              | Exponential backoff (250ms → configurable ceiling)             |
| 📡 **Multi-provider**              | Twelve Data and Finnhub supported out of the box               |
| 🛡️ **TCP_NODELAY**                 | Nagle algorithm disabled at the socket level                   |
| 💾 **Dual output**                 | JSON Lines (NDJSON) or CSV, flushed once per tick              |
| 🏗️ **Release-optimised binary**    | LTO + single codegen unit + `panic = abort` + symbol stripping |
| ⚙️ **Flexible config**             | CLI flags → environment variables → `.env` file                |
| 📝 **Structured logging**          | `tracing` subscriber with `--verbose` flag or `RUST_LOG`       |

---

## 🏗️ Architecture

```
┌──────────────────────────────────────────────────────────┐
│                    xauusd-fetcher v2                     │
│                                                          │
│  main.rs         Bootstrap, CLI parsing, tracing init    │
│  config.rs       Config struct (clap Parser + env vars)  │
│  connection.rs   WebSocket lifecycle + hot receive loop  │
│  types.rs        Wire types (RawTick, Tick, Finnhub…)    │
│  output.rs       Global BufWriter<Stdout> behind Mutex   │
└───────────────────────────┬──────────────────────────────┘
                            │  wss://
             ┌──────────────┴───────────────┐
             │                              │
    ┌────────▼────────┐            ┌────────▼────────┐
    │   Twelve Data   │            │    Finnhub       │
    │  (XAU/USD etc.) │            │ (OANDA:XAU_USD)  │
    └─────────────────┘            └─────────────────┘
```

**Per-tick data flow:**

```
Network frame arrives
        │
        ▼
Timestamp captured (Utc::now())     ← before parsing = true latency
        │
        ▼
serde_json::from_str(&str)          ← borrows frame buffer, no allocation
        │
        ▼
Event discriminant check            ← "price" | "heartbeat" | "subscribe-status"
        │
        ▼
Tick::from_raw()                    ← normalise bid / ask / spread
        │
        ▼
BufWriter stdout + flush            ← 1 write(2) syscall per tick
```

**Reconnect loop:**

```
connect_and_stream() returns Err  ─┐
                                   │
backoff::retry (exponential)       │ 250ms → 500ms → 1s → … → max
                                   │
connect_and_stream() again  ◄──────┘  fresh TLS handshake + subscribe
```

---

## 📁 Project Structure

```
xauusd-fetcher/
├── Cargo.toml              # Dependencies & release profile
├── Cargo.lock              # Locked dependency tree
├── .env                    # Your secrets (git-ignored)
├── .env.example            # Template — copy and fill in
├── debug_ws.py             # Python helper to inspect raw WebSocket frames
├── README.md               # This file
├── data/                   # Tick output directory (create before piping)
└── src/
    ├── main.rs             # current_thread tokio runtime, entry point
    ├── config.rs           # Config struct (clap + env)
    ├── connection.rs       # WebSocket + TCP_NODELAY + backoff hot loop
    ├── types.rs            # RawTick, Tick, FinnhubMsg, SubscribeMsg structs
    └── output.rs           # Lazy<Mutex<BufWriter<Stdout>>>
```

---

## 📦 Prerequisites

| Requirement               | Version                    | Notes                  |
| ------------------------- | -------------------------- | ---------------------- |
| **Rust**                  | stable ≥ 1.75              | `rustup update stable` |
| **Cargo**                 | bundled with Rust          | —                      |
| **API Key**               | Twelve Data **or** Finnhub | free tiers supported   |
| **Python 3** _(optional)_ | any                        | for `debug_ws.py` only |

### Get a Free API Key

**Option A — Twelve Data** (recommended for XAU/USD)

1. Sign up at [twelvedata.com](https://twelvedata.com) — free Basic plan
2. Dashboard → API Keys → copy your key
3. ⚠️ XAU/USD only streams during **forex market hours** (Mon–Fri)

**Option B — Finnhub** (best for 24/7 development)

1. Sign up at [finnhub.io](https://finnhub.io) — free forever
2. Dashboard → copy API key
3. Use symbol `OANDA:XAU_USD`

---

## 🔧 Installation

```bash
# 1. Clone
git clone https://github.com/yourname/xauusd-fetcher
cd xauusd-fetcher

# 2. Configure
cp .env.example .env
# Open .env and paste your real API key

# 3. Build — release is mandatory for all optimisations
cargo build --release
```

> ⚠️ Always build with `--release`. Debug builds skip LTO, don't strip symbols, and run significantly slower.

---

## ⚙️ Configuration

Settings resolve in this priority order: **CLI flag → env var → `.env` file → default**.

### `.env` file

```bash
# ── Twelve Data (default) ────────────────────────────────────
API_KEY=your_twelve_data_api_key_here
WS_URL=wss://ws.twelvedata.com/v1/quotes/price
SYMBOL=XAU/USD
OUTPUT_FORMAT=json            # "json" (NDJSON) or "csv"
BACKOFF_MAX_SECS=60

# ── Finnhub (uncomment to switch) ───────────────────────────
# API_KEY=your_finnhub_api_key_here
# WS_URL=wss://ws.finnhub.io
# SYMBOL=OANDA:XAU_USD

# ── Logging ─────────────────────────────────────────────────
# RUST_LOG=info               # or use --verbose flag at runtime
```

### Full Option Reference

| CLI Flag             | Env Var            | Default         | Description                  |
| -------------------- | ------------------ | --------------- | ---------------------------- |
| `--api-key`          | `API_KEY`          | _(required)_    | Provider API key             |
| `--ws-url`           | `WS_URL`           | Twelve Data WSS | WebSocket endpoint URL       |
| `--symbol`           | `SYMBOL`           | `XAU/USD`       | Instrument to subscribe to   |
| `--output-format`    | `OUTPUT_FORMAT`    | `json`          | `json` (NDJSON) or `csv`     |
| `--backoff-max-secs` | `BACKOFF_MAX_SECS` | `60`            | Max reconnect wait ceiling   |
| `--verbose` / `-v`   | `RUST_LOG=info`    | off             | Connection lifecycle logging |

---

## 🚀 Running

### Standard

```powershell
.\target\release\xauusd-fetcher.exe
```

### With verbose connection logs

```powershell
.\target\release\xauusd-fetcher.exe --verbose
```

### Override symbol at runtime (no config file edit needed)

```powershell
$env:SYMBOL = "BTC/USD"
.\target\release\xauusd-fetcher.exe --verbose
```

### Full CLI override (no `.env` needed)

```powershell
.\target\release\xauusd-fetcher.exe `
  --api-key "your_key" `
  --symbol "XAU/USD" `
  --output-format json `
  --backoff-max-secs 30 `
  --verbose
```

### Linux / macOS

```bash
./target/release/xauusd-fetcher --verbose
SYMBOL=BTC/USD ./target/release/xauusd-fetcher
```

### Inspect raw WebSocket frames (Python debugger)

```powershell
$env:API_KEY = "your_key"   # set the key
python debug_ws.py           # edit SYMBOL inside the script to change instrument
```

---

## 📄 Output Formats

### JSON Lines / NDJSON (default)

One compact JSON object per line — easy to pipe, grep, stream, and parse downstream.

```json
{"timestamp":"2026-03-02T00:01:23.441Z","symbol":"XAU/USD","bid":2938.42,"ask":2938.42,"spread":0.0}
{"timestamp":"2026-03-02T00:01:24.102Z","symbol":"XAU/USD","bid":2938.39,"ask":2938.39,"spread":0.0}
```

### CSV

```
2026-03-02T00:01:23.441+00:00,XAU/USD,2938.42000,2938.42000,0.00000
2026-03-02T00:01:24.102+00:00,XAU/USD,2938.39000,2938.39000,0.00000
```

Switch with `--output-format csv`.

### Field Reference

| Field       | Type         | Description                                                  |
| ----------- | ------------ | ------------------------------------------------------------ |
| `timestamp` | ISO 8601 UTC | When **this machine received** the WebSocket frame           |
| `symbol`    | string       | e.g. `XAU/USD`, `BTC/USD`                                    |
| `bid`       | float        | Bid price (or single trade price if bid unavailable)         |
| `ask`       | float        | Ask price (or single trade price if ask unavailable)         |
| `spread`    | float        | `ask − bid`; `0.0` when only a single price field is present |

> **Note on Twelve Data Basic plan:** the provider sends a single `price` field per tick, not a bid/ask pair. `Tick::from_raw()` normalises this transparently as `bid = ask = price, spread = 0.0`. Premium quote endpoints that send true bid/ask are also handled automatically.

---

## 💾 Saving Data to File

### Recommended — timestamped session files (PowerShell)

```powershell
mkdir -Force data, logs | Out-Null
$ts = Get-Date -Format "yyyy-MM-dd_HHmmss"
.\target\release\xauusd-fetcher.exe --verbose `
  2>"logs\conn_$ts.log" | `
  Tee-Object -Append -FilePath "data\ticks_$ts.ndjson"
```

This command simultaneously:

- ✅ Streams ticks live to your terminal
- ✅ Appends every tick to a timestamped `.ndjson` file
- ✅ Saves connection events to a separate `.log` file
- ✅ Never overwrites a previous session

### Simple append (bash)

```bash
mkdir -p data logs
./target/release/xauusd-fetcher 2>logs/conn.log >> data/ticks.ndjson
```

### CSV recording

```bash
./target/release/xauusd-fetcher --output-format csv >> data/ticks.csv
```

### Querying saved data

```powershell
# Count ticks captured
(Get-Content data\ticks_*.ndjson).Count

# Show last 10 ticks
Get-Content data\ticks_*.ndjson | Select-Object -Last 10

# Filter by price with jq (winget install jqlang.jq)
Get-Content data\ticks_*.ndjson | jq 'select(.bid > 2940)'
```

```bash
# Linux / macOS
wc -l data/ticks.ndjson
tail -10 data/ticks.ndjson
jq 'select(.bid > 2940)' data/ticks.ndjson
```

---

## 📡 Supported Providers

### Twelve Data

| Property      | Value                                     |
| ------------- | ----------------------------------------- |
| WebSocket URL | `wss://ws.twelvedata.com/v1/quotes/price` |
| Symbol format | `XAU/USD`, `BTC/USD`, `EUR/USD`           |
| Free plan     | Basic — 1 concurrent WebSocket connection |
| XAU/USD       | ✅ Forex market hours only (Mon–Fri)      |
| Price format  | Single `price` field per tick             |

### Finnhub

| Property      | Value                              |
| ------------- | ---------------------------------- |
| WebSocket URL | `wss://ws.finnhub.io`              |
| Symbol format | `OANDA:XAU_USD`, `BINANCE:BTCUSDT` |
| Free plan     | ✅ Completely free                 |
| XAU/USD       | ✅ 24/7 via OANDA feed             |
| Price format  | Trade price (no bid/ask spread)    |

---

## 🔤 Supported Symbols

### Twelve Data

```
XAU/USD    Gold / US Dollar          ← primary target
XAG/USD    Silver / US Dollar
BTC/USD    Bitcoin / US Dollar       ← 24/7, great for dev/testing
ETH/USD    Ethereum / US Dollar
EUR/USD    Euro / US Dollar
GBP/USD    British Pound / US Dollar
```

### Finnhub

```
OANDA:XAU_USD      Gold (24/7)
OANDA:XAG_USD      Silver
BINANCE:BTCUSDT    Bitcoin (24/7)
BINANCE:ETHUSDT    Ethereum (24/7)
```

---

## ⚡ Latency Optimisations

### Network Layer

| Technique                            | Location                              | Effect                                                         |
| ------------------------------------ | ------------------------------------- | -------------------------------------------------------------- |
| **TCP_NODELAY**                      | `connection.rs` → `set_tcp_nodelay()` | Disables Nagle — eliminates 40–200ms ACK batching delay        |
| **Frame + message size caps**        | `connection.rs` → `WebSocketConfig`   | 16 KB frames / 64 KB messages — prevents head-of-line blocking |
| **OS-native TLS (SChannel/OpenSSL)** | `Cargo.toml` → `native-tls` feature   | No third-party crypto overhead                                 |

### Runtime Layer

| Technique                                | Location                          | Effect                                                            |
| ---------------------------------------- | --------------------------------- | ----------------------------------------------------------------- |
| **`current_thread` tokio runtime**       | `main.rs`                         | No cross-thread work-stealing or task migration jitter            |
| **Timestamp before parsing**             | `connection.rs` hot loop          | Captures true network arrival time, not post-processing time      |
| **Zero-copy JSON**                       | `connection.rs` → `handle_text()` | `&str` borrowed from frame buffer — no extra allocation           |
| **`#[inline(always)]` on `handle_text`** | `connection.rs`                   | Hot path folded directly into receive loop by the compiler        |
| **Global `Lazy<Mutex<BufWriter>>`**      | `output.rs`                       | Single writer instance — no allocation per tick                   |
| **Per-tick flush**                       | `output.rs` → `emit()`            | One `write(2)` syscall per tick; downstream sees data immediately |

### Compiler Layer

| Setting             | `Cargo.toml`        | Effect                                                  |
| ------------------- | ------------------- | ------------------------------------------------------- |
| `opt-level = 3`     | `[profile.release]` | Full speed optimisation                                 |
| `lto = true`        | `[profile.release]` | Cross-crate function inlining                           |
| `codegen-units = 1` | `[profile.release]` | Single compilation unit — better whole-program inlining |
| `panic = "abort"`   | `[profile.release]` | No stack unwinding tables or overhead                   |
| `strip = true`      | `[profile.release]` | Smaller binary, faster load                             |

---

## 🕐 Market Hours

### Gold (XAU/USD) — Forex Schedule

```
┌──────────────┬──────────────┬──────────────────┐
│   Session    │   UTC        │   IST            │
├──────────────┼──────────────┼──────────────────┤
│ Sydney open  │ Sun 22:00    │ Mon 03:30 AM     │
│ Tokyo open   │ Mon 00:00    │ Mon 05:30 AM     │
│ London open  │ Mon 08:00    │ Mon 13:30 PM     │
│ New York open│ Mon 13:00    │ Mon 18:30 PM     │
│ Market close │ Fri 22:00    │ Sat 03:30 AM     │
└──────────────┴──────────────┴──────────────────┘
Weekend: CLOSED — no ticks arrive on Twelve Data
```

> **Dev tip:** Use `BTC/USD` (Twelve Data) or `BINANCE:BTCUSDT` (Finnhub) for 24/7 tick flow during development and testing.

---

## 🔧 Troubleshooting

### Connected but no ticks appearing

```
Cause 1: Market is closed (weekend / holiday)
Fix:     Switch to BTC/USD for 24/7 flow
         $env:SYMBOL = "BTC/USD"; .\target\release\xauusd-fetcher.exe --verbose

Cause 2: Wrong provider/symbol combination
Fix:     Twelve Data → "XAU/USD"   |   Finnhub → "OANDA:XAU_USD"

Cause 3: Silent error frames from provider
Fix:     $env:RUST_LOG = "debug"; .\target\release\xauusd-fetcher.exe --verbose
         Or run python debug_ws.py to see raw frames
```

### Repeated disconnections

```
Cause:   Basic plan allows only 1 concurrent WebSocket connection
Fix:     Wait ~10 seconds after Ctrl+C before restarting
         Never run two instances simultaneously
         Check Twelve Data dashboard for active connection count
```

### `os error 32` / locked file (Windows)

```
Fix:     Get-Process | Where-Object {$_.Name -match "rustc|cargo"} | Stop-Process -Force
         Remove-Item -Recurse -Force .\target
         cargo build --release
```

### API key rejected (401 / auth error)

```
Fix:     Open .env — confirm API_KEY is your real key with no surrounding quotes
         Verify at: twelvedata.com/account  or  finnhub.io/dashboard
```

### Build fails

```
Fix:     cargo clean
         rustup update stable
         cargo build --release
```

---

## 🗺️ Roadmap

### Near-term

- [ ] `simd-json` integration — AVX2/SSE4.2 accelerated JSON parsing
- [ ] `parking_lot::Mutex` — lower-overhead locking for the output writer
- [ ] CPU thread pinning via `core_affinity`
- [ ] Linux performance mode — `chrt -f 99`, `taskset`, `isolcpus`

### Medium-term

- [ ] Multi-symbol streaming — single connection, multiple subscriptions
- [ ] Binary tick recorder — FlatBuffers format for compact on-disk storage
- [ ] TimescaleDB / ClickHouse sink
- [ ] Redis pub/sub publisher for downstream consumers

### Advanced

- [ ] Broker API integration — OANDA streaming REST or Interactive Brokers TWS
- [ ] Paper trading engine with P&L tracking on live stream
- [ ] gRPC tick server — publish to strategy microservices
- [ ] Backtester — replay saved `.ndjson` files through strategy logic

---

## 📜 License

MIT — free to use, modify, and distribute.

---

## ⚠️ Disclaimer

For **educational and research purposes only**. Free-tier WebSocket feeds have data quality and reliability limitations versus professional market data vendors. Do not use this as the sole basis for live trading decisions.

---

_Built with 🦀 Rust · Powered by Twelve Data & Finnhub_
