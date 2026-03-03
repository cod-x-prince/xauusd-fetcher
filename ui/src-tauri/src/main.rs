// src-tauri/src/main.rs
// Prevents a console window from appearing on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    xauusd_ui_lib::run();
}
