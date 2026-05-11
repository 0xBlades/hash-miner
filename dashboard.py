#!/usr/bin/env python3
"""Real-time mining dashboard for hash256-gpu-miner"""
import os
import re
import json
import subprocess
from datetime import datetime
from http.server import HTTPServer, BaseHTTPRequestHandler
from string import Template

LOG_PATH = "/root/hash256-gpu-miner/miner.log"
PORT = 8080


def parse_log():
    """Parse miner.log and return latest stats."""
    data = {
        "status": "offline",
        "hashrate": 0.0,
        "total_hashes": 0,
        "gpu_name": "Unknown",
        "gpu_util": 0,
        "gpu_power": 0,
        "gpu_temp": 0,
        "era": "-",
        "epoch": "-",
        "reward": "-",
        "difficulty": "-",
        "challenge": "-",
        "last_update": "-",
        "uptime_sec": 0,
        "found_nonces": [],
    }

    if os.path.exists(LOG_PATH):
        with open(LOG_PATH, "r") as f:
            raw_lines = f.readlines()
    else:
        return data

    # Deduplicate consecutive identical lines (caused by tee)
    lines = []
    prev = None
    for line in raw_lines:
        if line != prev:
            lines.append(line)
            prev = line

    # Parse hashrate lines (take latest)
    for line in reversed(lines):
        line = line.strip()
        if "[HASHRATE]" in line:
            m = re.search(r"([\d.]+)\s+MH/s", line)
            if m:
                data["hashrate"] = float(m.group(1))
            m2 = re.search(r"Total hashes:\s*(\d+)", line)
            if m2:
                data["total_hashes"] = int(m2.group(1))
            m3 = re.search(r"\[(\d{4}-\d{2}-\d{2}\s+\d{2}:\d{2}:\d{2})\]", line)
            if m3:
                data["last_update"] = m3.group(1)
                try:
                    dt = datetime.strptime(m3.group(1), "%Y-%m-%d %H:%M:%S")
                    data["uptime_sec"] = int((datetime.now() - dt).total_seconds())
                except Exception:
                    pass
            data["status"] = "mining"
            break

    # Parse era/epoch/difficulty/challenge (take latest)
    for line in reversed(lines):
        line = line.strip()
        if line.startswith("["):
            if "Era:" in line and data["era"] == "-":
                m = re.search(r"Era:\s*(\d+)", line)
                if m:
                    data["era"] = m.group(1)
            elif "Epoch:" in line and data["epoch"] == "-":
                m = re.search(r"Epoch:\s*(\d+)", line)
                if m:
                    data["epoch"] = m.group(1)
            elif "Reward:" in line and data["reward"] == "-":
                m = re.search(r"Reward:\s*([\d.]+)", line)
                if m:
                    data["reward"] = m.group(1) + " HASH"
            elif "Difficulty:" in line and data["difficulty"] == "-":
                m = re.search(r"Difficulty:\s*(\d+)", line)
                if m:
                    data["difficulty"] = m.group(1)
            elif "Challenge:" in line and data["challenge"] == "-":
                m = re.search(r"Challenge:\s*(0x[0-9a-fA-F]+)", line)
                if m:
                    data["challenge"] = m.group(1)

    # Parse FOUND nonce events (deduplicated)
    for line in lines:
        if "FOUND nonce" in line:
            m = re.search(r"\[(\d{4}-\d{2}-\d{2}\s+\d{2}:\d{2}:\d{2})\].*FOUND nonce:\s*(\d+)", line)
            if m:
                nonce_entry = {"time": m.group(1), "nonce": m.group(2)}
                # Avoid duplicate nonce entries
                if not data["found_nonces"] or data["found_nonces"][-1]["nonce"] != nonce_entry["nonce"]:
                    data["found_nonces"].append(nonce_entry)
    data["found_nonces"] = data["found_nonces"][-10:]

    # GPU info via nvidia-smi
    try:
        out = subprocess.check_output(
            ["nvidia-smi", "--query-gpu=name,utilization.gpu,power.draw,temperature.gpu",
             "--format=csv,noheader,nounits"],
            stderr=subprocess.DEVNULL, text=True
        )
        parts = out.strip().split(", ")
        if len(parts) >= 4:
            data["gpu_name"] = parts[0]
            data["gpu_util"] = int(parts[1])
            data["gpu_power"] = float(parts[2])
            data["gpu_temp"] = int(parts[3])
    except Exception:
        pass

    return data


def format_hashes(n):
    """Format large hash count into human-readable form."""
    if n >= 1_000_000_000_000:
        return f"{n/1_000_000_000_000:.2f} T"
    elif n >= 1_000_000_000:
        return f"{n/1_000_000_000:.2f} G"
    elif n >= 1_000_000:
        return f"{n/1_000_000:.2f} M"
    elif n >= 1_000:
        return f"{n/1_000:.2f} K"
    return str(n)


HTML_TEMPLATE = Template("""<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Hash256 GPU Miner Dashboard</title>
<meta http-equiv="refresh" content="10">
<style>
  :root { --bg:#0b0c15; --card:#121320; --accent:#00e676; --accent2:#00bcd4; --warn:#ffab00; --danger:#ff5252; --text:#c7c9d3; --muted:#6c6f82; --border:#1e213a; }
  * { box-sizing:border-box; margin:0; padding:0; }
  body { background:var(--bg); color:var(--text); font-family:'Segoe UI',system-ui,sans-serif; padding:20px; min-height:100vh; }
  .header { display:flex; align-items:center; justify-content:space-between; margin-bottom:20px; max-width:1200px; margin-left:auto; margin-right:auto; }
  h1 { font-size:1.4rem; color:#fff; }
  .header-badge { background:var(--card); border:1px solid var(--border); padding:6px 14px; border-radius:20px; font-size:.75rem; color:var(--accent); }
  .grid { display:grid; grid-template-columns:repeat(auto-fit,minmax(200px,1fr)); gap:14px; max-width:1200px; margin:0 auto 16px; }
  .card { background:var(--card); border-radius:12px; padding:18px; border:1px solid var(--border); transition:border-color .2s; }
  .card:hover { border-color:#2a2d4a; }
  .card h3 { font-size:.7rem; text-transform:uppercase; letter-spacing:1.2px; color:var(--muted); margin-bottom:6px; }
  .big { font-size:1.8rem; font-weight:700; color:#fff; word-break:break-word; }
  .hash-big { font-size:2.2rem; font-weight:800; }
  .accent { color:var(--accent); }
  .accent2 { color:var(--accent2); }
  .warn { color:var(--warn); }
  .danger { color:var(--danger); }
  .muted { color:var(--muted); font-size:.8rem; }
  .status-dot { display:inline-block; width:10px; height:10px; border-radius:50%; margin-right:8px; vertical-align:middle; }
  .online { background:var(--accent); box-shadow:0 0 8px var(--accent); animation:pulse 2s infinite; }
  .offline { background:var(--danger); box-shadow:0 0 8px var(--danger); }
  @keyframes pulse { 0%,100%{opacity:1} 50%{opacity:.6} }
  .row { display:flex; gap:8px; align-items:center; flex-wrap:wrap; margin-bottom:6px; }
  .badge { background:#1a1b2e; padding:4px 10px; border-radius:16px; font-size:.72rem; color:var(--text); border:1px solid var(--border); white-space:nowrap; }
  .gpu-badge { background:linear-gradient(135deg,#1a1b2e,#1e2040); border:1px solid #2a2d5a; }
  table { width:100%; border-collapse:collapse; margin-top:10px; font-size:.82rem; }
  th,td { padding:10px 12px; text-align:left; border-bottom:1px solid var(--border); }
  th { color:var(--muted); font-weight:600; font-size:.72rem; text-transform:uppercase; letter-spacing:.5px; }
  .mono { font-family:ui-monospace,SFMono-Regular,Menlo,monospace; font-size:.78rem; word-break:break-all; color:var(--accent2); }
  .footer { text-align:center; color:var(--muted); font-size:.7rem; margin-top:20px; padding-top:12px; border-top:1px solid var(--border); max-width:1200px; margin-left:auto; margin-right:auto; }
  .stat-row { display:flex; justify-content:space-between; align-items:center; padding:4px 0; }
  .stat-label { color:var(--muted); font-size:.78rem; }
  .stat-value { color:#fff; font-size:.85rem; font-weight:500; }
  .progress-bar { width:100%; height:6px; background:#1a1b2e; border-radius:3px; margin-top:8px; overflow:hidden; }
  .progress-fill { height:100%; border-radius:3px; transition:width .3s; }
  .temp-bar { background:linear-gradient(90deg,var(--accent),var(--warn),var(--danger)); }
</style>
</head>
<body>
  <div class="header">
    <h1>⚡ Hash256 GPU Miner</h1>
    <span class="header-badge" id="refresh-indicator">● Live</span>
  </div>

  <div class="grid">
    <!-- Status Card -->
    <div class="card">
      <h3>Status</h3>
      <div class="row">
        <span class="status-dot ${status_class}"></span>
        <span class="big" style="font-size:1.3rem;">${status}</span>
      </div>
      <div style="margin-top:8px;">
        <div class="stat-row">
          <span class="stat-label">GPU Util</span>
          <span class="stat-value">${gpu_util}%</span>
        </div>
        <div class="progress-bar">
          <div class="progress-fill ${util_color}" style="width:${gpu_util}%"></div>
        </div>
        <div class="stat-row" style="margin-top:6px;">
          <span class="stat-label">Temperature</span>
          <span class="stat-value">${gpu_temp}°C</span>
        </div>
        <div class="progress-bar">
          <div class="progress-fill temp-bar" style="width:${temp_pct}%"></div>
        </div>
      </div>
    </div>

    <!-- Hashrate Card -->
    <div class="card">
      <h3>Hashrate</h3>
      <div class="big hash-big accent">${hashrate}</div>
      <div style="margin-top:2px;color:var(--accent);font-size:.85rem;font-weight:600;">MH/s</div>
      <div class="badge" style="margin-top:8px;">≈ ${ghashrate} GH/s</div>
    </div>

    <!-- Total Hashes Card -->
    <div class="card">
      <h3>Total Hashes</h3>
      <div class="big accent2">${total_hashes_human}</div>
      <div class="muted" style="margin-top:4px;">${total_hashes_raw}</div>
      <div class="badge" style="margin-top:6px;">Last: ${last_update}</div>
    </div>

    <!-- Power Card -->
    <div class="card">
      <h3>Power</h3>
      <div class="big">${gpu_power}<span style="font-size:1rem;color:var(--muted);"> W</span></div>
      <div class="badge" style="margin-top:8px;">Efficiency: ${efficiency} H/W</div>
    </div>

    <!-- GPU Card -->
    <div class="card">
      <h3>GPU</h3>
      <div style="font-size:1rem;font-weight:600;color:#fff;">${gpu_name}</div>
      <div class="row" style="margin-top:8px;">
        <span class="badge gpu-badge">🖥️ OpenCL</span>
        <span class="badge gpu-badge">🔥 ${gpu_temp}°C</span>
        <span class="badge gpu-badge">⚡ ${gpu_power}W</span>
      </div>
    </div>

    <!-- Contract Card -->
    <div class="card">
      <h3>Contract</h3>
      <div class="row">
        <span class="badge">Era ${era}</span>
        <span class="badge">Epoch ${epoch}</span>
      </div>
      <div class="stat-row" style="margin-top:8px;">
        <span class="stat-label">Reward</span>
        <span class="stat-value accent">${reward}</span>
      </div>
    </div>
  </div>

  <!-- Challenge & Difficulty -->
  <div class="grid">
    <div class="card" style="grid-column:1/-1;">
      <h3>Current Challenge</h3>
      <div class="mono">${challenge}</div>
    </div>
    <div class="card" style="grid-column:1/-1;">
      <h3>Difficulty</h3>
      <div class="mono">${difficulty}</div>
    </div>
  </div>

  <!-- Found Nonces -->
  <div class="card" style="max-width:1200px; margin:0 auto 16px;">
    <h3>Found Nonces</h3>
    <table>
      <tr><th>Time</th><th>Nonce</th></tr>
      ${nonce_rows}
    </table>
  </div>

  <div class="footer">
    ⚡ Hash256 GPU Miner Dashboard v2.0 &nbsp;|&nbsp; Auto-refresh 10s &nbsp;|&nbsp; RTX 4090
  </div>
</body>
</html>""")


class DashboardHandler(BaseHTTPRequestHandler):
    def log_message(self, format, *args):
        pass

    def do_GET(self):
        if self.path == "/api/stats":
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Access-Control-Allow-Origin", "*")
            self.end_headers()
            self.wfile.write(json.dumps(parse_log()).encode())
            return

        data = parse_log()
        status_class = "online" if data["status"] == "mining" else "offline"
        total_human = format_hashes(data["total_hashes"])
        ghashrate = f"{data['hashrate']/1000:.2f}"
        uptime_str = f"Updated {data['uptime_sec']}s ago" if data["uptime_sec"] < 120 else "Running stable"

        # GPU util color
        util_color = "accent" if data["gpu_util"] >= 90 else ("warn" if data["gpu_util"] >= 50 else "danger")

        # Temperature percentage (0-100°C scale)
        temp_pct = min(data["gpu_temp"], 100)

        # Efficiency (hashes per watt)
        if data["gpu_power"] > 0:
            efficiency = f"{data['hashrate'] * 1_000_000 / data['gpu_power']:,.0f}"
        else:
            efficiency = "-"

        nonce_rows = ""
        if data["found_nonces"]:
            for n in reversed(data["found_nonces"]):
                nonce_rows += f"<tr><td>{n['time']}</td><td class='mono'>{n['nonce']}</td></tr>"
        else:
            nonce_rows = "<tr><td colspan='2' style='color:var(--muted);text-align:center;padding:20px;'>No nonces found yet — keep mining! ⛏️</td></tr>"

        html = HTML_TEMPLATE.substitute(
            status=data["status"].upper(),
            status_class=status_class,
            hashrate=f"{data['hashrate']:.2f}",
            ghashrate=ghashrate,
            total_hashes_human=total_human,
            total_hashes_raw=f"{data['total_hashes']:,}",
            last_update=data["last_update"],
            gpu_name=data["gpu_name"],
            gpu_util=data["gpu_util"],
            gpu_temp=data["gpu_temp"],
            gpu_power=f"{data['gpu_power']:.1f}",
            era=data["era"],
            epoch=data["epoch"],
            reward=data["reward"],
            challenge=data["challenge"] if len(data["challenge"]) < 80 else data["challenge"][:76] + "...",
            difficulty=data["difficulty"] if len(data["difficulty"]) < 80 else data["difficulty"][:76] + "...",
            nonce_rows=nonce_rows,
            uptime=uptime_str,
            util_color=util_color,
            temp_pct=temp_pct,
            efficiency=efficiency,
        )

        self.send_response(200)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.end_headers()
        self.wfile.write(html.encode())


def run():
    server = HTTPServer(("0.0.0.0", PORT), DashboardHandler)
    print(f"[DASHBOARD] Running at http://0.0.0.0:{PORT}")
    server.serve_forever()


if __name__ == "__main__":
    run()
