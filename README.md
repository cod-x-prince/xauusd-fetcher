# ⚡ XAU/USD Ultra-Low-Latency Data Fetcher

> A production-grade, sub-millisecond market data feed for **Gold (XAU/USD)** and other instruments, built in **Rust** using async WebSocket streaming.

```
{"timestamp":"2026-03-02T00:01:23.441Z","symbol":"XAU/USD","bid":2938.42,"ask":2938.42,"spread":0.0}
{"timestamp":"2026-03-02T00:01:23.887Z","symbol":"XAU/USD","bid":2938.45,"ask":2938.45,"spread":0.0}
{"timestamp":"2026-03-02T00:01:24.102Z","symbol":"XAU/USD","bid":2938.39,"ask":2938.39,"spread":0.0}
```

---

## 📋 Table of Contents

- [Features](#-features)
- [Architecture](#-architecture)
- [Project Structure](#-project-structure)
- [Prerequisites](#-prerequisites)
- [Installation](#-installation)
- [Configuration](#-configuration)
- [Running the Tool](#-running-the-tool)
- [Output Formats](#-output-formats)
- [Saving Data to File](#-saving-data-to-file)
- [Supported Providers](#-supported-providers)
- [Supported Symbols](#-supported-symbols)
- [Latency Optimisations](#-latency-optimisations)
- [Market Hours](#-market-hours)
- [Troubleshooting](#-troubleshooting)
- [Next Steps](#-next-steps)

---

## ✨ Features

| Feature | Details |
|---|---|
| 🚀 **Ultra-low latency** | Sub-millisecond processing per tick |
| 🔌 **WebSocket streaming** | Persistent connection, no polling |
| 🔄 **Auto-reconnect** | Exponential backoff on disconnect |
| 📡 **Multi-provider** | Twelve Data + Finnhub supported |
| 💾 **Dual output** | JSON Lines (NDJSON) or CSV |
| 🛡️ **TCP_NODELAY** | Nagle algorithm disabled for minimal delay |
| 📝 **Structured logging** | Tracing with configurable verbosity |
| ⚙️ **Flexible config** | CLI flags + environment variables + `.env` file |
| 🏗️ **Release optimised** | LTO + single codegen unit + panic=abort |

---

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────────┐
│                   xauusd-fetcher                    │
│                                                     │
│  main.rs          Bootstrap, CLI, Tracing           │
│  config.rs        All settings (CLI / ENV / .env)   │
│  connection.rs    WebSocket lifecycle + hot loop    │
│  types.rs         Wire types + normalised Tick      │
│  output.rs        Buffered stdout (JSON / CSV)      │
└──────────────────────┬──────────────────────────────┘
                       │  WebSocket (wss://)
          ┌────────────┴────────────┐
          │                         │
   ┌──────▼──────┐           ┌──────▼──────┐
   │ Twelve Data │           │   Finnhub   │
   │  Basic 8+   │           │  Free tier  │
   └─────────────┘           └─────────────┘
```

**Data flow per tick:**
```
Network frame arrives
      │
      ▼
Timestamp captured  ← before parsing (true latency measurement)
      │
      ▼
serde_json parse    ← zero-copy borrow from frame buffer
      │
      ▼
Tick normalised     ← bid / ask / spread unified
      │
      ▼
BufWriter stdout    ← batched write + per-tick flush
```

---

## 📁 Project Structure

```
xauusd-fetcher/
├── Cargo.toml          # Dependencies & release profile
├── Cargo.lock          # Locked dependency versions
├── .env                # Your config (not committed to git)
├── .env.example        # Template — copy and fill in
├── README.md           # This file
├── data/               # Tick data output (auto-created)
│   └── ticks_YYYY-MM-DD_HHmmss.ndjson
├── logs/               # Connection logs (auto-created)
│   └── conn_YYYY-MM-DD_HHmmss.log
└── src/
    ├── main.rs         # Entry point
    ├── config.rs       # Config struct (clap + env)
    ├── connection.rs   # WebSocket + TCP_NODELAY + backoff
    ├── types.rs        # RawTick, Tick, FinnhubMsg structs
    └── output.rs       # Buffered stdout writer
```

---

## 📦 Prerequisites

| Requirement | Version | Install |
|---|---|---|
| **Rust** | stable ≥ 1.75 | `rustup update stable` |
| **Cargo** | (bundled with Rust) | — |
| **API Key** | Twelve Data or Finnhub | See below |

### Get a Free API Key

**Option A — Twelve Data** (recommended for XAU/USD)
1. Sign up at [twelvedata.com](https://twelvedata.com) — free Basic 8 plan
2. Go to **Dashboard → API Keys** → copy your key
3. ⚠️ XAU/USD streams only during **forex market hours** (Mon–Fri)

**Option B — Finnhub** (best for 24/7 testing)
1. Sign up at [finnhub.io](https://finnhub.io) — free forever tier
2. Go to **Dashboard** → copy your API key
3. Use symbol `OANDA:XAU_USD` with Finnhub

---

## 🔧 Installation

```bash
# 1. Clone the repository
git clone https://github.com/yourname/xauusd-fetcher
cd xauusd-fetcher

# 2. Copy and edit the config file
cp .env.example .env
# Open .env and paste your API key

# 3. Build the optimised release binary
cargo build --release
```

Build time: ~30–60 seconds (first build downloads all dependencies).

---

## ⚙️ Configuration

All settings can be provided via **`.env` file**, **environment variables**, or **CLI flags** — in that order of precedence.

### `.env` file (recommended)

```bash
# ── Provider: Twelve Data ──────────────────────────────
API_KEY=your_twelve_data_api_key_here
WS_URL=wss://ws.twelvedata.com/v1/quotes/price
SYMBOL=XAU/USD
PROVIDER=twelvedata

# ── Provider: Finnhub (uncomment to switch) ────────────
# API_KEY=your_finnhub_api_key_here
# WS_URL=wss://ws.finnhub.io
# SYMBOL=OANDA:XAU_USD
# PROVIDER=finnhub

# ── General settings ───────────────────────────────────
BACKOFF_MAX_SECS=60     # Max reconnect wait (seconds)
OUTPUT_FORMAT=json      # "json" or "csv"
# RUST_LOG=info         # Uncomment for verbose logs
```

### All Available Options

| Flag | Env Var | Default | Description |
|---|---|---|---|
| `--api-key` | `API_KEY` | *(required)* | Provider API key |
| `--ws-url` | `WS_URL` | Twelve Data WSS | WebSocket endpoint |
| `--symbol` | `SYMBOL` | `XAU/USD` | Instrument to stream |
| `--provider` | `PROVIDER` | `twelvedata` | `twelvedata` or `finnhub` |
| `--output-format` | `OUTPUT_FORMAT` | `json` | `json` or `csv` |
| `--backoff-max-secs` | `BACKOFF_MAX_SECS` | `60` | Max reconnect backoff |
| `--verbose` / `-v` | `RUST_LOG=info` | `false` | Show connection logs |

---

## 🚀 Running the Tool

### Basic run
```powershell
.\target\release\xauusd-fetcher.exe
```

### With verbose connection logging
```powershell
.\target\release\xauusd-fetcher.exe --verbose
```

### Override symbol on the fly
```powershell
$env:SYMBOL = "BTC/USD"
.\target\release\xauusd-fetcher.exe --verbose
```

### Using cargo (development)
```powershell
cargo run --release -- --verbose
```

### Full CLI override (no .env needed)
```powershell
.\target\release\xauusd-fetcher.exe `
  --api-key  "your_key"   `
  --symbol   "XAU/USD"    `
  --provider "twelvedata" `
  --output-format json    `
  --verbose
```

---

## 📄 Output Formats

### JSON Lines (NDJSON) — default
One JSON object per line. Easy to parse, stream, and pipe to other tools.

```json
{"timestamp":"2026-03-02T00:01:23.441Z","symbol":"XAU/USD","bid":2938.42,"ask":2938.42,"spread":0.0}
{"timestamp":"2026-03-02T00:01:24.102Z","symbol":"XAU/USD","bid":2938.39,"ask":2938.39,"spread":0.0}
```

### CSV
```
2026-03-02T00:01:23.441Z,XAU/USD,2938.42000,2938.42000,0.00000
2026-03-02T00:01:24.102Z,XAU/USD,2938.39000,2938.39000,0.00000
```

Switch format:
```powershell
.\target\release\xauusd-fetcher.exe --output-format csv
```

### Field Reference

| Field | Type | Description |
|---|---|---|
| `timestamp` | ISO 8601 UTC | Wall-clock time tick was **received** by this machine |
| `symbol` | string | Instrument (e.g. `XAU/USD`, `BTC/USD`) |
| `bid` | float | Bid price (or last trade price if bid unavailable) |
| `ask` | float | Ask price (or last trade price if ask unavailable) |
| `spread` | float | `ask − bid` (0.0 when only single price available) |

---

## 💾 Saving Data to File

### Recommended — timestamped session files
```powershell
mkdir -Force data, logs | Out-Null
$ts = Get-Date -Format "yyyy-MM-dd_HHmmss"
.\target\release\xauusd-fetcher.exe --verbose `
  2>"logs\conn_$ts.log" | `
  Tee-Object -Append -FilePath "data\ticks_$ts.ndjson"
```

This command:
- ✅ Creates `data/` and `logs/` folders automatically
- ✅ Streams ticks live to your terminal
- ✅ Saves every tick to a timestamped `.ndjson` file
- ✅ Saves connection events to a separate `.log` file
- ✅ Never overwrites previous sessions

### Simple append
```powershell
.\target\release\xauusd-fetcher.exe >> data\ticks.ndjson
```

### CSV recording
```powershell
.\target\release\xauusd-fetcher.exe --output-format csv >> data\ticks.csv
```

### Reading saved data
```powershell
# Count total ticks captured
(Get-Content data\ticks_*.ndjson).Count

# Show last 10 ticks
Get-Content data\ticks_*.ndjson | Select-Object -Last 10

# Pretty-print with jq (install: winget install jqlang.jq)
Get-Content data\ticks_*.ndjson | jq '.'

# Filter ticks where price moved more than $5
Get-Content data\ticks_*.ndjson | jq 'select(.bid > 2940)'
```

---

## 📡 Supported Providers

### Twelve Data
| Property | Value |
|---|---|
| WebSocket URL | `wss://ws.twelvedata.com/v1/quotes/price` |
| Symbol format | `XAU/USD`, `BTC/USD`, `EUR/USD` |
| Free plan | Basic 8 — 1 WS connection, 8 symbols |
| XAU/USD available | ✅ Yes (market hours only) |
| Data type | Single `price` field per tick |
| Pricing | [twelvedata.com/pricing](https://twelvedata.com/pricing) |

### Finnhub
| Property | Value |
|---|---|
| WebSocket URL | `wss://ws.finnhub.io` |
| Symbol format | `OANDA:XAU_USD`, `BINANCE:BTCUSDT` |
| Free plan | ✅ Completely free |
| XAU/USD available | ✅ Yes (24/7 via OANDA feed) |
| Data type | Trade price (no bid/ask spread) |
| Pricing | [finnhub.io/pricing](https://finnhub.io/pricing) |

---

## 🔤 Supported Symbols

### Twelve Data
```
XAU/USD    Gold vs US Dollar       ← primary target
XAG/USD    Silver vs US Dollar
BTC/USD    Bitcoin vs US Dollar    ← 24/7, good for testing
ETH/USD    Ethereum vs US Dollar
EUR/USD    Euro vs US Dollar
GBP/USD    British Pound vs USD
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

This tool applies multiple layers of latency reduction:

### Network Layer
| Technique | Code Location | Effect |
|---|---|---|
| **TCP_NODELAY** | `connection.rs` | Disables Nagle algorithm — removes 40–200ms ACK batching |
| **TLS native** | `Cargo.toml` | Uses OS TLS (SChannel on Windows) — no extra overhead |
| **Frame size cap** | `connection.rs` | 16KB frames — prevents head-of-line blocking |

### Runtime Layer
| Technique | Code Location | Effect |
|---|---|---|
| **`current_thread` runtime** | `main.rs` | No cross-thread work-stealing, zero migration jitter |
| **Timestamp before parse** | `connection.rs` | Captures true network arrival time |
| **Zero-copy JSON** | `connection.rs` | Borrows `&str` from frame buffer — no allocation |
| **`BufWriter` stdout** | `output.rs` | Batches writes, one `write(2)` syscall per tick |

### Compiler Layer
| Technique | `Cargo.toml` Setting | Effect |
|---|---|---|
| **LTO** | `lto = true` | Cross-crate inlining |
| **Single codegen unit** | `codegen-units = 1` | Better inlining across modules |
| **Full optimisation** | `opt-level = 3` | Maximum speed |
| **Abort on panic** | `panic = "abort"` | No unwinding overhead |
| **Strip symbols** | `strip = true` | Smaller binary |

---

## 🕐 Market Hours

### Gold (XAU/USD) — Forex Market Schedule

```
┌──────────────┬─────────────────┬─────────────────┐
│   Session    │   UTC           │   IST (India)   │
├──────────────┼─────────────────┼─────────────────┤
│ Sydney open  │ Sun 22:00       │ Mon 03:30 AM    │
│ Tokyo open   │ Mon 00:00       │ Mon 05:30 AM    │
│ London open  │ Mon 08:00       │ Mon 13:30 PM    │
│ New York open│ Mon 13:00       │ Mon 18:30 PM    │
│ Market close │ Fri 22:00       │ Sat 03:30 AM    │
└──────────────┴─────────────────┴─────────────────┘
Weekend: CLOSED (Saturday & Sunday) — no ticks will arrive
```

### Crypto (BTC/USD, ETH/USD)
```
✅ Trades 24 hours/day, 7 days/week — perfect for testing!
```

---

## 🔧 Troubleshooting

### No ticks appearing (connected but silent)
```
Cause 1: Market is closed (weekend / holiday)
Fix:     Test with BTC/USD which runs 24/7
         $env:SYMBOL = "BTC/USD"; .\target\release\xauusd-fetcher.exe --verbose

Cause 2: Wrong provider/symbol combination
Fix:     Check PROVIDER and SYMBOL match (see Supported Symbols above)

Cause 3: Hidden error frames
Fix:     Enable debug logging to see all raw frames
         $env:RUST_LOG = "debug"; .\target\release\xauusd-fetcher.exe --verbose
```

### Repeated disconnections
```
Cause:   Only 1 WebSocket connection allowed on Basic plan
Fix:     Wait 10 seconds after Ctrl+C before restarting
         Never run two instances simultaneously
         Check Twelve Data dashboard for connection count
```

### os error 32 (file locked) on Windows
```
Cause:   Previous rustc process still running / antivirus scanning
Fix:     Get-Process | Where-Object {$_.Name -match "rustc|cargo"} | Stop-Process -Force
         Remove-Item -Recurse -Force .\target
         cargo build --release
```

### API key rejected (401 error)
```
Cause:   Wrong key or placeholder not replaced
Fix:     Check .env file — ensure API_KEY has your real key (no quotes needed)
         Verify key at twelvedata.com/account or finnhub.io/dashboard
```

### Build fails with dependency errors
```
Fix:     cargo clean
         cargo build --release
```

---

## 🚀 Next Steps

Once you have XAU/USD streaming, here are natural extensions to build:

### 📊 Analytics (Easy)
```rust
// Track price movement statistics
// Calculate rolling average, std deviation
// Detect momentum shifts
```

### 📈 Signal Generation (Medium)
```rust
// Moving average crossover (EMA 9 / EMA 21)
// RSI calculation on tick stream
// Bollinger Band squeeze detection
// Volume spike alerts
```

### 🤖 Strategy Simulation (Advanced)
```rust
// Paper trading engine with P&L tracking
// Backtest on saved .ndjson tick files
// Multi-symbol correlation monitor
// Order book simulation
```

### 🏗️ Infrastructure (Advanced)
```rust
// Write ticks to TimescaleDB / ClickHouse
// Publish to Redis pub/sub for downstream consumers
// gRPC stream to trading strategy microservice
// Binary tick format (FlatBuffers) for max throughput
```

---

## 📜 License

MIT — free to use, modify, and distribute.

---

## ⚠️ Disclaimer

This tool is for **educational and research purposes only**. It is not financial advice. Always paper-trade and backtest thoroughly before risking real capital. Past performance of any strategy does not guarantee future results.

---

*Built with 🦀 Rust | Powered by Twelve Data & Finnhub*