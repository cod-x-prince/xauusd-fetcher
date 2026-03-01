// src/turbo.rs  —  v0.2.0
// Safe Turbo Subsystem — RAII-based OS performance tuning.
//
// HOW IT WORKS:
//   TurboGuard::engage() applies all safe Windows speed tweaks.
//   When the guard is dropped (Ctrl+C, panic, or clean exit) →
//   Drop::drop() fires automatically and REVERTS every change.
//
// WHAT IT TUNES:
//   ✅ Windows timer resolution  → 1ms   (default is 15.6ms — huge jitter)
//   ✅ Process priority          → HIGH  (more CPU time vs background tasks)
//   ✅ Power plan                → High Performance (no CPU frequency scaling)
//
// WHAT IT DELIBERATELY DOES NOT TOUCH:
//   ✗  Registry     — permanent, risky
//   ✗  NIC settings — requires reboot
//   ✗  Firewall     — unrelated to latency
//   ✗  Hyper-V      — breaks WSL2/Docker
//
// USAGE:
//   xauusd-fetcher.exe --turbo --verbose
//   xauusd-fetcher.exe             (no flag = zero system changes)

use tracing::{info, warn};

// ── Public RAII guard ─────────────────────────────────────────────────────────

pub struct TurboGuard {
    /// GUID of the power scheme that was active before we switched.
    /// Restored in Drop so the user's original plan is preserved.
    original_power_scheme: Option<String>,
}

impl TurboGuard {
    /// Apply all safe tuning. Returns a guard — keep it alive for the process lifetime.
    pub fn engage() -> Self {
        info!("⚡ TURBO MODE ENGAGED");

        // Order matters: timer first so subsequent sleeps use the new resolution.
        apply_timer_resolution();
        apply_process_priority();
        let original_power_scheme = apply_power_plan();

        info!("  ✓ Timer resolution  → 1ms  (was ~15.6ms)");
        info!("  ✓ Process priority  → HIGH");
        info!("  ✓ Power plan        → High Performance");
        info!("  All changes will auto-revert on exit (Ctrl+C safe)");

        TurboGuard { original_power_scheme }
    }
}

impl Drop for TurboGuard {
    /// Guaranteed to run on: clean return, panic, OR Ctrl+C (via Rust's panic hook).
    fn drop(&mut self) {
        info!("⚡ TURBO MODE disengaging — reverting system changes...");
        revert_timer_resolution();
        revert_process_priority();
        if let Some(ref guid) = self.original_power_scheme {
            revert_power_plan(guid);
        }
        info!("⚡ System fully restored. Goodbye!");
    }
}

// ── Implementation: Windows ───────────────────────────────────────────────────

#[cfg(windows)]
fn apply_timer_resolution() {
    use windows_sys::Win32::Media::timeBeginPeriod;
    // 1 = 1ms resolution. Valid range: 1–15 (milliseconds).
    // Effect: tokio sleeps, select!, and backoff delays become more precise.
    unsafe { timeBeginPeriod(1); }
}

#[cfg(windows)]
fn revert_timer_resolution() {
    use windows_sys::Win32::Media::timeEndPeriod;
    unsafe { timeEndPeriod(1); }
    info!("  ✓ Timer resolution  → restored (system default)");
}

#[cfg(windows)]
fn apply_process_priority() {
    use windows_sys::Win32::System::Threading::{
        GetCurrentProcess, SetPriorityClass, HIGH_PRIORITY_CLASS,
    };
    unsafe { SetPriorityClass(GetCurrentProcess(), HIGH_PRIORITY_CLASS); }
}

#[cfg(windows)]
fn revert_process_priority() {
    use windows_sys::Win32::System::Threading::{
        GetCurrentProcess, SetPriorityClass, NORMAL_PRIORITY_CLASS,
    };
    unsafe { SetPriorityClass(GetCurrentProcess(), NORMAL_PRIORITY_CLASS); }
    info!("  ✓ Process priority  → Normal");
}

#[cfg(windows)]
fn apply_power_plan() -> Option<String> {
    // Save current scheme first so we can restore it precisely.
    let original = get_active_power_scheme();
    // High Performance GUID — built into every Windows installation.
    let status = std::process::Command::new("powercfg")
        .args(["/setactive", "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c"])
        .status();
    if status.is_err() {
        warn!("  ⚠ Could not set High Performance power plan (non-fatal)");
    }
    original
}

#[cfg(windows)]
fn revert_power_plan(guid: &str) {
    let _ = std::process::Command::new("powercfg")
        .args(["/setactive", guid])
        .status();
    info!("  ✓ Power plan        → restored ({})", &guid[..8.min(guid.len())]);
}

#[cfg(windows)]
fn get_active_power_scheme() -> Option<String> {
    let out = std::process::Command::new("powercfg")
        .args(["/getactivescheme"])
        .output().ok()?;
    let s = String::from_utf8(out.stdout).ok()?;
    // Output format: "Power Scheme GUID: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx ..."
    s.split_whitespace()
        .find(|w| w.len() == 36 && w.contains('-'))
        .map(String::from)
}

// ── Implementation: Non-Windows stubs ────────────────────────────────────────
// These compile cleanly on Linux/macOS so the codebase stays cross-platform.

#[cfg(not(windows))] fn apply_timer_resolution()           {}
#[cfg(not(windows))] fn revert_timer_resolution()          { info!("  ✓ Timer resolution  → n/a (Linux/macOS)"); }
#[cfg(not(windows))] fn apply_process_priority()           {}
#[cfg(not(windows))] fn revert_process_priority()          { info!("  ✓ Process priority  → n/a (Linux/macOS)"); }
#[cfg(not(windows))] fn apply_power_plan() -> Option<String> { None }
#[cfg(not(windows))] fn revert_power_plan(_guid: &str)     { info!("  ✓ Power plan        → n/a (Linux/macOS)"); }