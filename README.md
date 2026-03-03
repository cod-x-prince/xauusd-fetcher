# ⚡ XAU/USD Live Trading Terminal

> A production-grade, ultra-low-latency market data terminal for **Gold (XAU/USD)** and other
> instruments — built from scratch in **Rust + Tauri v2 + Vanilla JS**.
>
> **30µs processing latency. 57 ticks/min. Live canvas chart. OS keychain security.**

```
┌─────────────────────────────────────────────────────────────────────────┐
│ Au XAU/USD LIVE  │ BTC/USD │                      ● LIVE  ⚙ SETTINGS   │
├──────────────────┼─────────────────────────────────────────────────────-┤
│                  │                                            68210.90  │
│  68183.64        │    ╭─╮                                               │
│  -20.36(-0.030%) │   ╱   ╲      ╭──╮                         68191.00  │
│                  │  ╱     ╲    ╱    ╲    ╭──╮                           │
│  BID   │  ASK    │ ╱       ╲──╯      ╲──╯   ╲               68163.38  │
│ 68183  │ 68183   │                           ╲──                        │
│ SPREAD: 0.0000   │                                            68149.23  │
├──────────────────┤                                                       │
│ HIGH    │ LOW    │                                                       │
│68284.04 │68149.23│                                                       │
│ TPM: 57 │ 30µs   │                                                       │
├──────────────────┴──────────────────────────────────────────────────────┤
│ FEED TWELVEDATA  SYMBOL BTC/USD  TOTAL 202 TICKS  UTC 08:13:52          │
└─────────────────────────────────────────────────────────────────────────┘
```

![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange?style=flat-square&logo=rust)
![Tauri](https://img.shields.io/badge/Tauri-v2-blue?style=flat-square)
![Version](https://img.shields.io/badge/Version-3.0.0-purple?style=flat-square)
![Latency](https://img.shields.io/badge/Processing%20Latency-30µs-gold?style=flat-square)
![License](https://img.shields.io/badge/License-MIT-green?style=flat-square)

---

## Table of Contents

- [The Journey — v0.1.0 → v3.0.0](#the-journey)
- [What We Built](#what-we-built)
- [Network Latency vs Processing Latency](#network-latency-vs-processing-latency)
- [How We Achieved 30µs Processing Latency](#how-we-achieved-30µs-processing-latency)
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

This project was built iteratively — every version solving a real problem discovered in the previous one.

---

### v0.1.0 — First connection

The starting point: a basic Rust CLI that opened a WebSocket to Twelve Data and printed raw JSON to stdout. No latency measurement, no reconnection, no optimisations. It worked, but ticks were slow (~1–5ms) and the connection would drop silently.

```
WebSocket frame → serde_json::from_str → println!
```

Problems discovered: Nagle algorithm buffering small TCP frames for up to 200ms. No error recovery meant one network blip killed the process. Timestamp captured after parsing meant we were measuring nothing useful.

---

### v0.2.0 — Performance Engineering

A deep dive into systems programming. This version introduced every major speed optimisation.

**What changed:**
- `TCP_NODELAY` — disabled Nagle algorithm, eliminating artificial 40–200ms buffering delay
- `simd-json` with AVX2/SSE4.2 — hardware-accelerated JSON parsing via CPU vector instructions
- `core_affinity` — pinned the hot path thread to a dedicated CPU core, eliminating OS scheduler jitter
- `parking_lot::Mutex` — replaced `std::sync::Mutex`, dropped lock/unlock cost from ~100ns to ~5ns
- Thread-local buffers — eliminated heap allocations in the tick loop
- Timestamp captured **before** `serde_json::from_str` — measuring true arrival-to-ready latency
- **Turbo Mode** — a RAII subsystem applying Windows OS-level performance tuning:
  - `timeBeginPeriod(1)` — raised Windows timer resolution from 15ms → 0.5ms
  - `SetPriorityClass(REALTIME_PRIORITY_CLASS)` — pre-empted all other processes
  - All settings automatically reverted on exit via Rust `Drop` trait

**Results:**
```
Before v0.2.0:   1,000 – 5,000 µs per tick
Standard mode:     300 –   500 µs per tick
Turbo mode:        100 –   250 µs per tick
```

Tagged `v0.2.0` on branch `v2/ultra-fast`.

---

### v2.0.0 — Production CLI

Cleaned up the entire codebase, resolved all warnings, and documented every optimisation. Added:

- Exponential backoff reconnection (250ms → 30s ceiling) via the `backoff` crate
- `WebSocketConfig` frame size caps — prevents head-of-line blocking on large frames
- Dual output: JSON Lines (NDJSON) and CSV
- `#[inline(always)]` on `handle_text()` — folds the entire hot path into the receive loop
- `biased` tokio `select!` — stop signal checked before every WebSocket frame
- `current_thread` tokio runtime — no work-stealing jitter from thread migration
- Production README with architecture diagrams

Tagged `v2.0.0` on branch `v2/ultra-fast`.

---

### v3.0.0 — Secure Cross-Platform UI (This Release)

The biggest leap: a full native desktop trading terminal. Rust backend runs the WebSocket feed; Vanilla JS frontend renders ticks with sub-5ms tick-to-pixel latency.

**What changed:**
- Monorepo restructure — `cli/` and `ui/` subfolders in one repo
- Tauri v2 app — ~5MB binary, no Electron, no Node.js server
- `tauri-plugin-stronghold` — API key stored in OS keychain (Windows DPAPI / macOS Keychain)
- `Zeroizing<String>` — API key bytes wiped from RAM on replace or drop
- `get_config()` returns only `key_is_set: bool` — key never returns to JS under any circumstances
- Hardened `.gitignore` — blocks `.env`, `.stronghold`, `node_modules`, all build artifacts
- Live canvas chart — hardware-accelerated 2D, no charting library
- Tick feed with per-row latency display
- Session high/low, ticks/min, avg latency stats
- Bloomberg terminal aesthetic — gold accents, Space Mono monospace font

**Achieved in production: 30µs processing latency, 57 ticks/min on BTC/USD**

Tagged `v3.0.0` on branch `v3/secure-ui`.

---

## What We Built

### CLI Tool (`cli/`)

A standalone Rust binary. Streams live price ticks to stdout as NDJSON or CSV. Designed for piping into files, databases, or downstream tools.

```powershell
cd cli
cargo build --release
.\target\release\xauusd-fetcher.exe --verbose

# Save to file
.\target\release\xauusd-fetcher.exe | Tee-Object "ticks.ndjson"
```

Sample output:
```json
{"timestamp":"2026-03-03T08:13:52Z","symbol":"BTC/USD","bid":68183.64,"ask":68183.64,"spread":0.0,"latency_us":31}
```

### Desktop UI (`ui/`)

A native Tauri v2 app. The Rust backend runs the same WebSocket code as the CLI. The JS frontend renders it — zero framework, pure DOM + Canvas 2D.

```powershell
cd ui
npm install
npm run dev       # dev mode with hot reload
npm run build     # production .exe + NSIS installer
```

---

## Network Latency vs Processing Latency

This is the most important concept in the project. They are completely different things and require completely different solutions.

---

### Network Latency

```
Your machine  ──── Internet ────  Twelve Data servers
              <── tick arrives ──
```

**Definition:** The time for a price update to travel from the provider's server across the internet to your network card.

**Typical values:**
- Home broadband (same continent): 20–80ms
- Data centre co-location (same city as exchange): 0.1–2ms
- Cross-continent: 100–300ms

**You cannot optimise this in code.** It is determined entirely by physics — the speed of light through fibre optic cable — and your ISP's routing. The only way to reduce network latency is to physically move your machine closer to the server (co-location).

This is why our **30µs number does NOT mean a tick arrived in 30µs.** The tick still took ~20–80ms to cross the internet. The 30µs is what happened after it arrived at your network card.

---

### Processing Latency

```
Network card delivers frame to OS TCP stack
           │
           ▼  ← this is where we start measuring
tokio wakes up, frame delivered to app
           │
           ▼
Instant::now() captured          ← HERE — timestamp before any work
           │
           ▼
serde_json::from_str()           ← parse JSON (~5–15µs)
           │
           ▼
Tick struct constructed
           │
           ▼
app.emit("tick") → JS            ← IPC crossing (~1–3ms)
           │
           ▼
requestAnimationFrame → canvas   ← pixel on screen (~1–2ms)
```

**Definition:** The time between the WebSocket frame arriving at our application and the parsed tick being ready for use.

**Our measured value: 30µs average**

This is what all our code optimisations target. It is fully within our control.

---

### The Full Latency Stack

```
Price changes at exchange
        │  exchange matching engine (~µs)
        ▼
Twelve Data receives and processes trade
        │  Twelve Data internal processing + WebSocket push (~1–10ms)
        ▼
Internet transit
        │  NETWORK LATENCY (~20–80ms) ← cannot be reduced without co-location
        ▼
Your network card (NIC)
        │  Windows TCP stack (~100µs floor, OS-imposed minimum)
        ▼
tokio async runtime delivers frame
        │
        ▼  ← Instant::now() captured HERE
PROCESSING LATENCY (30µs) ← this is what our code optimises
        │  serde_json parse: ~5–15µs
        │  struct construction: ~1–3µs
        │  app.emit() IPC: ~1–3ms
        ▼
JS event handler executes
        │  DOM update: ~0.1ms
        │  requestAnimationFrame canvas redraw: ~1–2ms
        ▼
Pixel appears on screen

Total end-to-end: ~25–85ms
  Network:    ~20–80ms   (physics — irreducible without co-location)
  Processing: ~30µs      (our code — highly optimised)
  Render:     ~2–5ms     (browser compositing)
```

The 30µs we achieved means our code adds virtually zero overhead on top of what the OS delivers. This is the software floor — as fast as userspace code can go without kernel bypass (DPDK/io_uring).

---

## How We Achieved 30µs Processing Latency

Every technique below contributed. Together they took us from ~5ms down to ~30µs — a 150x improvement.

### 1. TCP_NODELAY — Biggest Single Win

```rust
tcp.set_nodelay(true)
```

Nagle's algorithm holds small outgoing TCP segments for up to 200ms, waiting to batch them together to reduce packet count. This is great for bulk data transfer (HTTP file downloads) but catastrophic for real-time streaming. A WebSocket tick message is only ~50–100 bytes. Without `TCP_NODELAY`, every tick was delayed by up to 200ms of artificial batching.

Setting `TCP_NODELAY = true` tells the OS to send every segment immediately. The fix requires unwrapping 3 layers of the TLS stack to reach the raw `TcpStream`:

```rust
// tokio_native_tls → native_tls → AllowStd → TcpStream
tls.get_ref().get_ref().get_ref().set_nodelay(true)
```

**Impact: eliminated up to 200ms of artificial delay on every tick.**

---

### 2. Timestamp Before Parse

```rust
// hot loop — order matters
let parse_start = Instant::now();     // ← captured BEFORE serde_json
let timestamp   = Utc::now();

if let Ok(raw) = serde_json::from_str::<RawTick>(&text) {
    latency_us: parse_start.elapsed().as_micros() as u64,
}
```

If we captured the timestamp after parsing, we'd measure nearly 0µs because `Instant::now()` is just reading a register. Capturing before parse measures the actual cost of the entire hot path — JSON parsing, struct construction, and any allocations.

---

### 3. Zero-Copy JSON Parsing

```rust
serde_json::from_str::<RawTick>(text.as_str())
//                               ^^^^^^^^^^^^
//                               borrows &str from WS frame buffer
```

We borrow `&str` directly from the WebSocket frame buffer — no `String::from()`, no `to_owned()`, no intermediate heap allocation. Serde reads the bytes in-place and constructs the struct fields directly. The data never moves in memory.

---

### 4. `biased` tokio `select!`

```rust
tokio::select! {
    biased;  // ← always check stop signal first, deterministically
    _ = stop.recv() => { /* clean shutdown */ }
    msg = ws.next() => { /* process tick */ }
}
```

Without `biased`, tokio randomly picks which branch to poll first on each iteration. Under high tick volume (57/min), the stop signal could be delayed by hundreds of ticks before being noticed. `biased` makes branch priority deterministic — stop is always checked first.

---

### 5. `parking_lot::Mutex` on Hot-Path State

```rust
use parking_lot::Mutex as PLMutex;
api_key: Arc<PLMutex<Zeroizing<String>>>,
```

`std::sync::Mutex` uses OS syscalls (`futex` on Linux, `SRWLock` on Windows) and can poison on panic. `parking_lot` uses a userspace spinlock for the uncontended case — which is always the case in a single-connection feed with no lock contention. Lock + unlock cost drops from ~100ns to ~5ns.

---

### 6. `current_thread` Tokio Runtime

```rust
#[tokio::main(flavor = "current_thread")]
```

The default multi-thread runtime spawns a thread pool and uses work-stealing — tasks can migrate between OS threads mid-execution. For a single WebSocket connection this adds CPU cache invalidation and scheduling jitter with zero throughput benefit. `current_thread` keeps everything on one thread, eliminating migration overhead entirely.

---

### 7. `#[inline(always)]` on `handle_text`

```rust
#[inline(always)]
fn handle_text(text: &str, app: &AppHandle) { ... }
```

Forces the compiler to paste the function body directly into the call site in the receive loop — eliminating the function call overhead (stack frame setup, register saves) and enabling the optimiser to see across the call boundary for better instruction scheduling.

---

### 8. Release Profile Compiler Flags

```toml
[profile.release]
opt-level     = 3      # maximum speed: vectorisation, loop unrolling, inlining
lto           = true   # link-time optimisation: inline across crate boundaries
codegen-units = 1      # single compilation unit: whole-program analysis
panic         = "abort" # no unwinding tables, no landing pads, smaller + faster
strip         = true   # strip debug symbols: smaller binary, faster OS load
```

LTO alone can improve hot-path speed by 5–15% by inlining functions across crate boundaries — for example, inlining `serde_json`'s internal functions directly into our parse call.

---

### 9. Frontend: `requestAnimationFrame` + Canvas 2D

```javascript
// Called on every tick — schedules redraw, not executes it immediately
requestAnimationFrame(drawChart);
```

Instead of redrawing the chart synchronously on every tick (wasting CPU between display refreshes), we schedule redraws via `rAF`. The browser batches them to the monitor's refresh rate (60fps = 16.6ms budget per frame). Canvas 2D is GPU-composited — no layout recalculation, no reflow, no DOM diffing. A full chart redraw takes ~0.5ms.

---

## Security Model

Security was treated with the same engineering rigour as performance. The API key has multiple independent protection layers.

### How the Key Flows

```
User types API key in Settings panel
        │
        │  invoke("start_feed", { api_key: "sk-...", ws_url, symbol })
        ▼
Rust start_feed() command receives FeedConfigIn
        │
        ├── api_key stored as Zeroizing<String>
        │   (old key bytes overwritten with 0x00 automatically)
        │
        ├── WebSocket URL built: Zeroizing::new(format!("{}?apikey={}", url, key))
        │
        ▼
run_feed() connects using URL
        │
        ├── Connection established
        ├── drop(url) — Zeroizing wipes URL bytes immediately
        │
        ▼
Feed runs — key never referenced again
        │
        ▼
JS calls get_config()
        │
        ▼  api_key is NEVER in this response
Returns { ws_url, symbol, key_is_set: true }
        │
        ▼
JS shows "● Key configured" — never sees the value
```

### Protection Layers

| Layer | Mechanism | Protects Against |
|---|---|---|
| In-memory | `Zeroizing<String>` | Key bytes persisting in RAM after replace/drop |
| At rest | `tauri-plugin-stronghold` | Key readable from disk (OS DPAPI/Keychain encrypts vault) |
| IPC | `get_config()` returns `key_is_set: bool` only | Key leaking back to JS renderer |
| URL string | `Zeroizing::new()` + explicit `drop()` | WebSocket URL with embedded key lingering in heap |
| Git | `.env` + `.stronghold` in `.gitignore` | Accidental commit |
| Frontend | Key never written to `localStorage` | Key readable in browser DevTools |

---

## Repository Structure

```
xauusd-fetcher/
│
├── cli/                        Rust WebSocket CLI fetcher
│   ├── src/
│   │   ├── main.rs             Entry point, current_thread Tokio runtime
│   │   ├── config.rs           Config (clap + env vars + .env file)
│   │   ├── connection.rs       WebSocket hot loop — TCP_NODELAY, backoff, SIMD
│   │   ├── types.rs            RawTick, Tick, SubscribeMsg
│   │   ├── output.rs           BufWriter stdout + per-tick flush
│   │   └── turbo.rs            RAII Windows performance tuning (Turbo Mode)
│   ├── Cargo.toml
│   └── .env.example
│
├── ui/                         Tauri v2 native desktop terminal
│   ├── src/
│   │   └── index.html          Complete UI — HTML + CSS + JS, no framework
│   ├── src-tauri/
│   │   ├── src/
│   │   │   ├── main.rs         #![windows_subsystem = "windows"] entry
│   │   │   ├── lib.rs          Tauri commands, AppState, security model
│   │   │   └── fetcher.rs      WebSocket feed — adapted from cli/connection.rs
│   │   ├── capabilities/
│   │   │   └── default.json    Minimal Tauri v2 permission surface
│   │   ├── icons/icon.ico
│   │   ├── Cargo.toml
│   │   ├── build.rs
│   │   └── tauri.conf.json
│   └── package.json
│
├── .gitignore                  Blocks .env, .stronghold, node_modules, targets
└── README.md                   This file
```

---

## Prerequisites

| Tool | Min Version | Install |
|---|---|---|
| Rust | 1.75 | `rustup update stable` |
| Node.js | 18 | [nodejs.org](https://nodejs.org) |
| WebView2 | any | Pre-installed Windows 10/11 |
| API Key | — | [Twelve Data](https://twelvedata.com) or [Finnhub](https://finnhub.io) free tier |

---

## Running the UI

```powershell
cd ui
npm install         # first time only (~1 min)
npm run dev         # compiles Rust + opens app (~5 min first time, fast after)
npm run build       # production build
# Output: ui\src-tauri\target\release\xauusd-ui.exe
```

**Steps to connect:**
1. Click **⚙ SETTINGS** top right
2. Enter your API key
3. Select provider + symbol
4. Click **▶ CONNECT**

Use **BTC/USD** for 24/7 testing — gold only streams Mon–Fri.

---

## Running the CLI

```powershell
cd cli
cargo build --release

# Basic run
.\target\release\xauusd-fetcher.exe

# Verbose with latency output
.\target\release\xauusd-fetcher.exe --verbose

# Save ticks to file
$ts = Get-Date -Format "yyyy-MM-dd_HHmmss"
.\target\release\xauusd-fetcher.exe | Tee-Object "ticks_$ts.ndjson"
```

---

## Providers & Symbols

| Provider | Symbol | Description | Hours |
|---|---|---|---|
| Twelve Data | `XAU/USD` | Gold | Mon–Fri |
| Twelve Data | `XAG/USD` | Silver | Mon–Fri |
| Twelve Data | `BTC/USD` | Bitcoin | 24/7 |
| Twelve Data | `ETH/USD` | Ethereum | 24/7 |
| Twelve Data | `EUR/USD` | Euro | Mon–Fri |
| Finnhub | `OANDA:XAU_USD` | Gold via OANDA | 24/7 |
| Finnhub | `BINANCE:BTCUSDT` | Bitcoin | 24/7 |

---

## Market Hours

| Session | UTC | IST |
|---|---|---|
| Sydney open | Sun 22:00 | Mon 03:30 |
| Tokyo open | Mon 00:00 | Mon 05:30 |
| London open | Mon 08:00 | Mon 13:30 |
| New York open | Mon 13:00 | Mon 18:30 |
| Market close | Fri 22:00 | Sat 03:30 |

---

## Version History

| Version | Branch | Latency | What Changed |
|---|---|---|---|
| v0.1.0 | `main` | ~1–5ms | Basic WebSocket CLI |
| v0.2.0 | `v2/ultra-fast` | 100–500µs | SIMD JSON, core pinning, Turbo Mode |
| v2.0.0 | `v2/ultra-fast` | 100–300µs | TCP_NODELAY, backoff, dual output, production README |
| v3.0.0 | `v3/secure-ui` | **30µs** | Tauri UI, OS keychain, monorepo |

---

## Roadmap

- [ ] Linux Turbo Mode — `chrt -f 99`, `taskset`, `isolcpus`
- [ ] `simd-json` in UI fetcher (CLI already has it)
- [ ] CPU core pinning in UI fetcher via `core_affinity`
- [ ] Multi-symbol streaming — parallel subscriptions
- [ ] Tick recorder — binary FlatBuffers format to disk
- [ ] Broker API integration — OANDA streaming REST
- [ ] Android + iOS builds via Tauri v2 mobile targets
- [ ] TimescaleDB / ClickHouse sink for historical analysis

---

## Disclaimer

For educational and research purposes only. Free-tier WebSocket feeds have data quality limitations compared to institutional market data vendors (Bloomberg, Refinitiv). Do not use this as the sole basis for live trading decisions.

---

*Built with Rust · Tauri v2 · Vanilla JS · Powered by Twelve Data & Finnhub*
