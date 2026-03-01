# debug_ws.py
# Run this to see EXACTLY what Twelve Data is sending you (raw frames)
# Usage: python debug_ws.py

import websocket
import json
import os
from datetime import datetime

# Reads from environment variable — set it in PowerShell first:
# $env:API_KEY = "your_real_key_here"
API_KEY = os.environ.get("API_KEY", "")
if not API_KEY:
    raise SystemExit("❌ Set API_KEY env var first: $env:API_KEY = 'your_key'")

# Test these symbols one by one
SYMBOLS_TO_TEST = [
    "BTC/USD",
    "ETH/USD", 
    "EUR/USD",
    "XAU/USD",
    "AAPL",       # stock
]

SYMBOL = SYMBOLS_TO_TEST[0]  # change index to test different symbols
URL = f"wss://ws.twelvedata.com/v1/quotes/price?apikey={API_KEY}"

def on_open(ws):
    print(f"[{datetime.utcnow()}] ✅ Connected")
    sub = {"action": "subscribe", "params": {"symbols": SYMBOL}}
    ws.send(json.dumps(sub))
    print(f"[{datetime.utcnow()}] 📤 Subscribed to {SYMBOL}")

def on_message(ws, message):
    print(f"[{datetime.utcnow()}] 📨 RAW: {message}")

def on_error(ws, error):
    print(f"[{datetime.utcnow()}] ❌ Error: {error}")

def on_close(ws, code, msg):
    print(f"[{datetime.utcnow()}] 🔌 Closed: code={code} msg={msg}")

if __name__ == "__main__":
    print(f"Testing symbol: {SYMBOL}")
    print(f"URL: {URL[:60]}...")
    print("-" * 50)
    websocket.enableTrace(False)
    ws = websocket.WebSocketApp(
        URL,
        on_open=on_open,
        on_message=on_message,
        on_error=on_error,
        on_close=on_close,
    )
    ws.run_forever()