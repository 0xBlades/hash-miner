#!/usr/bin/env python3
"""Check miner.log for new events since last check."""
import os
import json

LOG = "/root/hash256-gpu-miner/miner.log"
STATE = "/tmp/miner_notif_state"

def get_last_pos():
    try:
        with open(STATE) as f:
            return int(f.read().strip())
    except:
        return 0

def save_pos(pos):
    with open(STATE, "w") as f:
        f.write(str(pos))

pos = get_last_pos()
if not os.path.exists(LOG):
    print("NO_EVENTS")
    exit(0)

with open(LOG) as f:
    f.seek(pos)
    new = f.readlines()
    new_pos = f.tell()

if not new:
    print("NO_EVENTS")
    exit(0)

save_pos(new_pos)

events = []
for line in new:
    line = line.strip()
    if "FOUND nonce" in line:
        parts = line.split("FOUND nonce: ")[1]
        nonce = parts.split(" ")[0]
        elapsed = parts.split("elapsed: ")[1].rstrip(")") if "elapsed:" in parts else "?"
        events.append(f"⛏️ *Nonce Found!*\n`{nonce}`\n⏱ {elapsed}")
    elif "TX broadcast" in line:
        tx = line.split("TX broadcast: ")[1].split(" ")[0]
        events.append(f"📡 *TX Broadcasted*\n`{tx}`")
    elif "TX send error" in line:
        err = line.split("TX send error: ")[1][:150]
        events.append(f"❌ *TX Error*\n`{err}`")
    elif "TX failed" in line:
        err = line.split("TX failed ")[1][:150]
        events.append(f"❌ *TX Failed*\n`{err}`")
    elif "Confirmed block" in line:
        blk = line.split("block: ")[1].split("|")[0].strip() if "block:" in line else "?"
        events.append(f"✅ *Block Confirmed!*\nBlock: {blk}")

if events:
    print(json.dumps(events))
else:
    print("NO_EVENTS")
