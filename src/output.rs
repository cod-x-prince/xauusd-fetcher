// src/output.rs
// Buffered, thread-safe stdout writer.
// BufWriter batches small writes into fewer syscalls while per-tick flush
// guarantees downstream consumers see updates in real time.

use std::io::{self, BufWriter, Write};
use std::sync::Mutex;

use anyhow::Result;
use once_cell::sync::Lazy;

use crate::types::Tick;

// Lazily-initialised, globally-shared buffered stdout.
// The Mutex ensures only one write is in flight at a time (safe for future
// multi-symbol / multi-thread expansion).
static OUT: Lazy<Mutex<BufWriter<io::Stdout>>> =
    Lazy::new(|| Mutex::new(BufWriter::with_capacity(8 * 1024, io::stdout())));

/// Write a single tick to stdout in the requested format.
///
/// Formats:
///   "csv"  → timestamp,symbol,bid,ask,spread  (one line)
///   *      → compact JSON (NDJSON / JSON Lines, default)
///
/// This is on the hot path — kept as lean as possible.
pub fn emit(tick: &Tick, format: &str) -> Result<()> {
    let mut out = OUT.lock().unwrap();

    match format {
        "csv" => {
            writeln!(
                out,
                "{},{},{:.5},{:.5},{:.5}",
                tick.timestamp.to_rfc3339(),
                tick.symbol,
                tick.bid,
                tick.ask,
                tick.spread,
            )?;
        }
        _ => {
            // serde_json writes directly into the BufWriter — no intermediate String.
            serde_json::to_writer(&mut *out, tick)?;
            writeln!(out)?; // NDJSON record delimiter
        }
    }

    // Flush once per tick.
    // Cost: ~1 write(2) syscall; acceptable at typical forex tick rates (< 1 000/s).
    out.flush()?;

    Ok(())
}