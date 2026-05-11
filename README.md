# ⛏️ Hash256 GPU Miner

GPU miner untuk hash256 proof-of-work smart contract di Ethereum. Dioptimasi untuk **FCFS (First Come First Served)** — nonce langsung di-broadcast ke mempool tanpa tunggu konfirmasi.

## Performance

| GPU | Hashrate |
|-----|----------|
| RTX 4090 | ~3.9 GH/s |

## Requirements

- **OS:** Linux (Ubuntu 20.04/22.04)
- **GPU:** NVIDIA (OpenCL driver) atau AMD (ROCm)
- **Rust:** 1.75+ (install via [rustup](https://rustup.rs/))
- **OpenCL:** `apt install nvidia-opencl-dev` (NVIDIA) atau `rocm-dev` (AMD)
- **ETH:** Minimal 0.005 ETH di wallet untuk gas fee

## Installation

### 1. Install Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

### 2. Install OpenCL (NVIDIA)

```bash
apt update
apt install -y nvidia-opencl-dev clinfo
```

Verifikasi:
```bash
clinfo | head -20
```

### 3. Clone Repository

```bash
git clone https://github.com/0xBlades/hash-miner.git
cd hash-miner
```

### 4. Build

```bash
cargo build --release
```

Binary akan ada di `./target/release/hash256-gpu-miner`

### 5. Setup Environment

Buat file `.env`:

```bash
cat > .env << 'EOF'
PRIVATE_KEY=0xYOUR_PRIVATE_KEY_HERE
RPC_URL=https://ethereum-rpc.publicnode.com
EOF
```

> ⚠️ **Jangan pernah commit `.env` ke git!**

Ganti RPC URL dengan provider favorit kamu:
- Public: `https://ethereum-rpc.publicnode.com`
- Alchemy: `https://eth-mainnet.g.alchemy.com/v2/YOUR_KEY`
- Infura: `https://mainnet.infura.io/v3/YOUR_KEY`
- BlockPI: `https://ethereum.blockpi.network/v1/rpc/YOUR_KEY`

## Running

### Basic Run

```bash
./target/release/hash256-gpu-miner
```

### Run with Screen (24/7)

```bash
screen -dmS miner bash -c 'stdbuf -oL ./target/release/hash256-gpu-miner 2>&1 | tee -a miner.log'
```

Commands:
- **Attach:** `screen -r miner`
- **Detach:** `Ctrl+A` lalu `D` (jangan `Ctrl+C`!)
- **List:** `screen -ls`

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

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable miner
systemctl start miner
```

## Dashboard

Jalankan dashboard untuk monitoring real-time:

```bash
screen -dmS dashboard bash -c 'cd /root/hash-miner && python3 dashboard.py'
```

Akses: `http://localhost:8080`

API endpoint: `http://localhost:8080/api/stats`

## Notifications (Telegram)

Notifikasi otomatis ke Telegram saat nonce ditemukan:

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

## Configuration

### Gas Price

Default: **1 Gwei** (FCFS optimized)

Ubah di `src/main.rs`:
```rust
gas_price: U256::from(1_000_000_000u64), // 1 Gwei
```

### Mining Parameters

```rust
let work_size: usize = 1 << 20;  // 1M work items per launch
let batch_size: u32 = 1024;       // 1024 nonces per work item
// Total per launch: ~1.07 billion hashes
```

## Troubleshooting

### GPU Not Found
```bash
# Check OpenCL
clinfo

# Check NVIDIA driver
nvidia-smi
```

### TX Error: Insufficient Funds
Top up ETH ke wallet minimal **0.005 ETH** untuk gas fee.

### Low Hashrate
- Pastikan GPU utilization 100%: `nvidia-smi`
- Cek temperature < 80°C
- Nonaktifkan display manager kalau ada

### RPC Error
Ganti RPC URL di `.env` ke provider lain.

## Contract Info

- **Contract:** `0xAC7b5d06fa1e77D08aea40d46cB7C5923A87A0cc`
- **Function:** `mine(uint256 nonce)`
- **Network:** Ethereum Mainnet

## License

MIT
