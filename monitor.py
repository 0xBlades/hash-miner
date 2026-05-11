#!/usr/bin/env python3
"""Monitor miner.log and send Telegram notifications via Hermes webhook."""
import os
import sys
import time
import json
import subprocess
from datetime import datetime

LOG_PATH = "/root/hash256-gpu-miner/miner.log"
CHECK_INTERVAL = 5  # seconds
LAST_POS_FILE = "/tmp/miner_monitor_pos"

# Patterns to watch
NOTIFY_PATTERNS = {
    "nonce_found": "FOUND nonce",
    "tx_broadcast": "TX broadcast",
    "tx_error": "TX send error",
    "tx_failed": "TX failed",
    "block_confirmed": "Confirmed block",
    "rpc_error": "RPC error",
    "gpu_error": "GPU",
    "challenge_change": "Challenge berubah",
}

def get_last_pos():
    try:
        with open(LAST_POS_FILE, "r") as f:
            return int(f.read().strip())
    except:
        return 0

def save_last_pos(pos):
    with open(LAST_POS_FILE, "w") as f:
        f.write(str(pos))

def send_telegram(message):
    """Send message via Hermes send_message tool (Telegram)."""
    # Write to a temp file and use hermes to send
    # Alternative: use the Telegram Bot API directly
    try:
        # Try using curl to Telegram Bot API if available
        bot_token = os.environ.get("TELEGRAM_BOT_TOKEN", "")
        chat_id = os.environ.get("TELEGRAM_CHAT_ID", "798642547")
        
        if bot_token:
            import urllib.request
            url = f"https://api.telegram.org/bot{bot_token}/sendMessage"
            data = json.dumps({
                "chat_id": chat_id,
                "text": message,
                "parse_mode": "HTML"
            }).encode()
            req = urllib.request.Request(url, data=data, headers={"Content-Type": "application/json"})
            urllib.request.urlopen(req, timeout=10)
            return True
    except Exception as e:
        print(f"[WARN] Telegram API failed: {e}", flush=True)
    
    # Fallback: write to notification file for Hermes to pick up
    try:
        notif_file = "/root/hash256-gpu-miner/pending_notifications.txt"
        with open(notif_file, "a") as f:
            f.write(f"{datetime.now().isoformat()}|{message}\n")
    except:
        pass
    
    return False

def format_message(event_type, line):
    """Format notification message based on event type."""
    ts = datetime.now().strftime("%H:%M:%S")
    
    if event_type == "nonce_found":
        # Extract nonce and elapsed time
        parts = line.split("FOUND nonce: ")
        if len(parts) > 1:
            nonce_info = parts[1].split(" ")[0]
            elapsed = parts[1].split("elapsed: ")[1].split(")")[0] if "elapsed:" in parts[1] else "?"
            return f"⛏️ <b>Nonce Found!</b>\nNonce: <code>{nonce_info}</code>\nTime: {elapsed}"
    
    elif event_type == "tx_broadcast":
        tx_hash = line.split("TX broadcast: ")[1].split(" ")[0] if "TX broadcast:" in line else "?"
        return f"📡 <b>TX Broadcast</b>\n<code>{tx_hash}</code>"
    
    elif event_type == "block_confirmed":
        block_num = line.split("block: ")[1].split("|")[0].strip() if "block:" in line else "?"
        gas_used = line.split("gas used: ")[1] if "gas used:" in line else "?"
        return f"✅ <b>Block Confirmed!</b>\nBlock: {block_num}\nGas: {gas_used}"
    
    elif event_type == "tx_error":
        error = line.split("TX send error: ")[1] if "TX send error:" in line else line
        return f"❌ <b>TX Error!</b>\n<code>{error[:200]}</code>"
    
    elif event_type == "tx_failed":
        error = line.split("TX failed ")[1] if "TX failed " in line else line
        return f"❌ <b>TX Failed!</b>\n<code>{error[:200]}</code>"
    
    elif event_type == "rpc_error":
        return f"⚠️ <b>RPC Error</b>\n<code>{line[:200]}</code>"
    
    elif event_type == "challenge_change":
        return f"🔄 <b>Challenge Changed</b>\nRestarting mining..."
    
    return f"ℹ️ <b>{event_type}</b>\n{line[:200]}"

def monitor():
    print(f"[MONITOR] Watching {LOG_PATH}...", flush=True)
    last_pos = get_last_pos()
    sent_nonce = set()  # Track sent nonces to avoid duplicates
    
    while True:
        try:
            if not os.path.exists(LOG_PATH):
                time.sleep(CHECK_INTERVAL)
                continue
            
            with open(LOG_PATH, "r") as f:
                f.seek(last_pos)
                new_lines = f.readlines()
                new_pos = f.tell()
            
            if new_lines:
                save_last_pos(new_pos)
                
                for line in new_lines:
                    line = line.strip()
                    if not line:
                        continue
                    
                    for event_type, pattern in NOTIFY_PATTERNS.items():
                        if pattern in line:
                            # Avoid duplicate notifications for same nonce
                            if event_type == "nonce_found":
                                nonce = line.split("FOUND nonce: ")[1].split(" ")[0] if "FOUND nonce:" in line else None
                                if nonce and nonce in sent_nonce:
                                    continue
                                if nonce:
                                    sent_nonce.add(nonce)
                                    # Keep only last 100 nonces
                                    if len(sent_nonce) > 100:
                                        sent_nonce = set(list(sent_nonce)[-50:])
                            
                            msg = format_message(event_type, line)
                            print(f"[NOTIFY] {event_type}: {msg[:100]}...", flush=True)
                            send_telegram(msg)
                            break
            
            time.sleep(CHECK_INTERVAL)
            
        except KeyboardInterrupt:
            print("[MONITOR] Stopped.", flush=True)
            break
        except Exception as e:
            print(f"[ERROR] {e}", flush=True)
            time.sleep(CHECK_INTERVAL)

if __name__ == "__main__":
    monitor()
