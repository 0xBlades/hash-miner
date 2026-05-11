# ⛏️ Hash256 GPU Miner

GPU miner untuk hash256 proof-of-work smart contract di Ethereum. Dioptimasi untuk **FCFS (First Come First Served)** — nonce langsung di-broadcast ke mempool tanpa tunggu konfirmasi.

## Features

- 🔥 **Dual/Multi-GPU** — Single instance, otomatis detect semua GPU
- ⚡ **11 RPC Providers** — 2 MEV (Flashbots, MEVBlocker) + 9 regular, fire paralel
- 🔒 **MEV Protection** — TX dikirim ke private mempool duluan (anti front-run)
- 💰 **Dynamic Gas** — Auto-estimate gas limit, cache 30 detik
- 🎯 **Zero-Hop Claim** — GPU thread langsung claim tanpa async hop
- 📊 **Real-time Hashrate** — Log per GPU tiap 10 detik

## Performance

| GPU | Hashrate |
|-----|----------|
| RTX 4090 | ~3.9 GH/s |
| RTX 3090 | ~1.6 GH/s |
| 2x RTX 3090 | ~3.2 GH/s |

## Requirements

- **OS:** Linux (Ubuntu 20.04/22.04/22.04+)
- **GPU:** NVIDIA dengan OpenCL support (RTX series recommended)
- **Rust:** 1.75+ (install via [rustup](https://rustup.rs/))
- **OpenCL:** `nvidia-opencl-dev`
- **ETH:** Minimal 0.005 ETH di wallet untuk gas fee

## Installation

### 1. Install System Dependencies

```bash
# Update system
apt update && apt upgrade -y

# Install build essentials
apt install -y build-essential pkg-config

# Install OpenCL (NVIDIA)
apt install -y nvidia-opencl-dev clinfo

# Install Python (untuk dashboard & notification)
apt install -y python3 python3-pip
```

### 2. Install NVIDIA Driver (jika belum ada)

```bash
# Cek driver
nvidia-smi

# Jika belum ada, install:
apt install -y nvidia-driver-535
reboot
```

### 3. Install Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source $HOME/.cargo/env

# Verify
rustc --version
cargo --version
```

### 4. Verifikasi OpenCL

```bash
# Cek GPU terdeteksi
clinfo | head -20

# Atau
nvidia-smi
```

Expected output:
```
+---------------------------+
| GPU Name        Memory    |
| 0  RTX 3090    24576 MiB  |
| 1  RTX 3090    24576 MiB  |
+---------------------------+
```

### 5. Clone & Build

```bash
git clone https://github.com/0xBlades/hash-miner.git
cd hash-miner

# Build (release mode — lebih cepat)
cargo build --release
```

Binary akan ada di `./target/release/hash256-gpu-miner`

**Build time:** ~30 detik (dependencies sudah cache) hingga ~3 menit (fresh install)

### 6. Setup Environment

Buat file `.env`:

```bash
cat > .env << 'EOF'
# === WALLET ===
PRIVATE_KEY=0xYOUR_PRIVATE_KEY_HERE

# === RPC PROVIDERS (semua di-fire paralel) ===
# Regular RPCs
RPC_URL=https://ethereum.blockpi.network/v1/rpc/YOUR_KEY
RPC_URL_2=https://ethereum.blockpi.network/v1/rpc/YOUR_KEY_2
RPC_URL_3=https://rpc.ankr.com/eth
RPC_URL_4=https://ethereum-rpc.publicnode.com
RPC_URL_5=https://eth.drpc.org

# MEV/Private RPCs (fire duluan, anti front-run)
RPC_MEV_1=https://rpc.mevblocker.io/fast
RPC_MEV_2=https://rpc.flashbots.net/fast

# === GAS SETTINGS ===
GAS_GWEI=5
GAS_LIMIT=200000
GAS_CACHE_TTL=30
EOF
```

> ⚠️ **Jangan pernah commit `.env` ke git!** Sudah di-gitignore.

### RPC Providers

| Provider | Type | URL |
|----------|------|-----|
| BlockPI | Regular | `https://ethereum.blockpi.network/v1/rpc/YOUR_KEY` |
| Ankr | Regular | `https://rpc.ankr.com/eth` |
| PublicNode | Regular | `https://ethereum-rpc.publicnode.com` |
| dRPC | Regular | `https://eth.drpc.org` |
| Chainstack | Regular | `https://ethereum-mainnet.core.chainstack.com/YOUR_KEY` |
| OnFinality | Regular | `https://eth.api.onfinality.io/public` |
| BlockRazor | Regular | `https://eth.blockrazor.xyz` |
| Tatum | Regular | `https://ethereum-mainnet.gateway.tatum.io` |
| MEVBlocker | **MEV** | `https://rpc.mevblocker.io/fast` |
| Flashbots | **MEV** | `https://rpc.flashbots.net/fast` |

> 💡 **Tips:** Minimal punya 2-3 RPC. MEV providers (Flashbots/MEVBlocker) sangat disarankan untuk FCFS.

## Configuration

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `PRIVATE_KEY` | (required) | Wallet private key, awalan `0x` |
| `RPC_URL` | (required) | Primary RPC endpoint |
| `RPC_URL_2` - `RPC_URL_10` | (optional) | Additional RPC endpoints |
| `RPC_MEV_1` - `RPC_MEV_4` | (optional) | MEV/private mempool endpoints |
| `GAS_GWEI` | `5` | Gas price dalam Gwei |
| `GAS_LIMIT` | `200000` | Gas limit fallback (jika estimasi gagal) |
| `GAS_CACHE_TTL` | `30` | Cache duration untuk gas estimation (detik) |

### Gas Settings

```
GAS_GWEI=5          # Gas price (5 Gwei = murah tapi tetap masuk)
GAS_LIMIT=200000    # Fallback gas limit
GAS_CACHE_TTL=30    # Refresh gas estimation tiap 30 detik
```

- **GAS_GWEI:** Semakin tinggi = semakin cepat TX di-mining. 5 Gwei biasanya cukup.
- **GAS_LIMIT:** Dynamic estimation akan override ini. Ini hanya fallback.
- **GAS_CACHE_TTL:** Berapa lama gas estimation di-cache. 30 detik = balance antara akurasi dan overhead.

## Running

### Basic Run

```bash
./target/release/hash256-gpu-miner
```

### Run with Screen (Recommended untuk 24/7)

```bash
screen -dmS miner bash -c './target/release/hash256-gpu-miner 2>&1 | tee -a miner.log'
```

**Screen commands:**
- **Attach:** `screen -r miner`
- **Detach:** `Ctrl+A` lalu `D` (jangan `Ctrl+C`!)
- **List sessions:** `screen -ls`
- **Kill session:** `screen -S miner -X quit`

### Run with Systemd (Auto-restart)

```bash
cat > /etc/systemd/system/miner.service << 'EOF'
[Unit]
Description=Hash256 GPU Miner
After=network.target

[Service]
Type=simple
WorkingDirectory=/root/hash-miner
ExecStart=/root/hash-miner/target/release/hash256-gpu-miner
Restart=always
RestartSec=5
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable miner
systemctl start miner

# Cek status
systemctl status miner
journalctl -u miner -f
```

## Multi-GPU Setup

Miner otomatis detect semua GPU yang tersedia. Tidak perlu konfigurasi tambahan.

```bash
# Cek GPU terdeteksi
nvidia-smi

# Output miner akan menunjukkan:
# GPU OpenCL tersedia: 2 device
# [GPU0] 1610 MH/s
# [GPU1] 1503 MH/s
```

**Supported:**
- 1 GPU → Single GPU mining
- 2 GPU → Parallel mining, first nonce wins
- 3+ GPU → Semua GPU parallel

## Monitoring

### Cek Real-time Log

```bash
# Tail log
tail -f miner.log

# Cek hashrate
grep "HASHRATE\|GPU" miner.log | tail -20

# Cek claim events
grep "CLAIM" miner.log | tail -10
```

### Cek GPU Status

```bash
# Utilization & temperature
nvidia-smi

# Detailed
nvidia-smi --query-gpu=index,name,utilization.gpu,temperature.gpu,power.draw --format=csv
```

### Dashboard (Web)

```bash
screen -dmS dashboard bash -c 'python3 dashboard.py'
```

Akses: `http://localhost:8080`
API: `http://localhost:8080/api/stats`

## Notifications (Telegram)

Notifikasi otomatis ke Telegram saat nonce ditemukan.

### 1. Setup Hermes Agent

Ikuti installasi di https://github.com/NousResearch/hermes-agent

### 2. Buat Cron Job

```bash
hermes cron create "every 1m" \
  --prompt "Check script output and send to Telegram" \
  --script ~/.hermes/scripts/check_events.py \
  --name miner-notifications
```

### Events yang di-notif:

| Event | Format |
|-------|--------|
| ⛏️ Nonce Found | Nonce + waktu + Etherscan link |
| ❌ Claim Failed | Nonce + error detail |
| ⚠️ RPC Error | Detail error |

## Troubleshooting

### GPU Not Found

```bash
# Check OpenCL
clinfo

# Check NVIDIA driver
nvidia-smi

# Reinstall OpenCL
apt install -y nvidia-opencl-dev
```

### TX Error: Insufficient Funds

Top up ETH ke wallet minimal **0.005 ETH** untuk gas fee.

### Low Hashrate

```bash
# Pastikan GPU utilization 100%
nvidia-smi

# Cek temperature (< 85°C ideal)
nvidia-smi --query-gpu=temperature.gpu --format=csv

# Jika temperature tinggi, kurangi power limit
nvidia-smi -pl 300
```

### RPC Error / Timeout

```bash
# Test RPC connectivity
curl -X POST https://ethereum-rpc.publicnode.com \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'

# Ganti RPC URL di .env jika perlu
```

### Miner Crash / Restart

```bash
# Cek log error
tail -50 miner.log | grep -i "error\|panic"

# Restart
screen -S miner -X quit
screen -dmS miner bash -c './target/release/hash256-gpu-miner 2>&1 | tee -a miner.log'
```

### Build Error

```bash
# Clean build
cargo clean
cargo build --release

# Jika dependency error
rm -rf target/
cargo build --release
```

## Architecture

```
┌─────────────────────────────────────────────┐
│              Main Loop                       │
│  ┌─────────┐    ┌──────────┐                │
│  │ RPC Call │───>│ Challenge│───> Work       │
│  └─────────┘    └──────────┘                │
│                      │                       │
│         ┌────────────┴────────────┐         │
│         ▼                         ▼         │
│   ┌──────────┐             ┌──────────┐     │
│   │  GPU 0   │             │  GPU 1   │     │
│   │ (1610    │             │ (1503    │     │
│   │  MH/s)   │             │  MH/s)   │     │
│   └────┬─────┘             └────┬─────┘     │
│        │                        │            │
│        └────────┬───────────────┘            │
│                 ▼                            │
│        ┌────────────────┐                    │
│        │ Nonce Found!   │                    │
│        │ (first wins)   │                    │
│        └────────┬───────┘                    │
│                 ▼                            │
│        ┌────────────────┐                    │
│        │  ClaimEngine   │                    │
│        │  (parallel)    │                    │
│        └────────┬───────┘                    │
│                 ▼                            │
│    ┌──────┬──────┬──────┬──────┐            │
│    ▼      ▼      ▼      ▼      ▼            │
│  MEV1   MEV2   RPC1   RPC2  ...RPC9        │
│  (fast) (fast)                               │
└─────────────────────────────────────────────┘
```

## Contract Info

- **Contract:** `0xAC7b5d06fa1e77D08aea40d46cB7C5923A87A0cc`
- **Function:** `mine(uint256 nonce)`
- **Network:** Ethereum Mainnet

## License

MIT
