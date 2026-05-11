use anyhow::Result;
use anyhow::Context as _;
use dotenv::dotenv;
use ethers::abi::Abi;
use ethers::contract::Contract;
use ethers::middleware::SignerMiddleware;
use ethers::providers::{Http, Provider};
use ethers::signers::{LocalWallet, Signer};
use ethers::types::{Address, H256, U256};
use ethers::utils::format_units;
use ethers::middleware::Middleware;

use ocl::{Buffer, Device, Platform, ProQue};
use rand::random;
use sha3::{Digest, Keccak256};
use std::env;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;


const CONTRACT_ADDRESS: &str = "0xAC7b5d06fa1e77D08aea40d46cB7C5923A87A0cc";

const ABI_JSON: &str = r#"[
  {"inputs":[{"internalType":"address","name":"miner","type":"address"}],"name":"getChallenge","outputs":[{"internalType":"bytes32","name":"","type":"bytes32"}],"stateMutability":"view","type":"function"},
  {"inputs":[],"name":"miningState","outputs":[{"internalType":"uint256","name":"era","type":"uint256"},{"internalType":"uint256","name":"reward","type":"uint256"},{"internalType":"uint256","name":"difficulty","type":"uint256"},{"internalType":"uint256","name":"minted","type":"uint256"},{"internalType":"uint256","name":"remaining","type":"uint256"},{"internalType":"uint256","name":"epoch","type":"uint256"},{"internalType":"uint256","name":"epochBlocksLeft_","type":"uint256"}],"stateMutability":"view","type":"function"},
  {"inputs":[{"internalType":"uint256","name":"nonce","type":"uint256"}],"name":"mine","outputs":[],"stateMutability":"nonpayable","type":"function"}
]"#;

const KERNEL_SRC: &str = r#"
__constant ulong RC[24] = {
    0x0000000000000001UL, 0x0000000000008082UL, 0x800000000000808aUL,
    0x8000000080008000UL, 0x000000000000808bUL, 0x0000000080000001UL,
    0x8000000080008081UL, 0x8000000000008009UL, 0x000000000000008aUL,
    0x0000000000000088UL, 0x0000000080008009UL, 0x000000008000000aUL,
    0x000000008000808bUL, 0x800000000000008bUL, 0x8000000000008089UL,
    0x8000000000008003UL, 0x8000000000008002UL, 0x8000000000000080UL,
    0x000000000000800aUL, 0x800000008000000aUL, 0x8000000080008081UL,
    0x8000000000008080UL, 0x0000000080000001UL, 0x8000000080008008UL
};

static inline ulong rotl64(ulong x, int n) {
    return (x << n) | (x >> (64 - n));
}

static inline void set_byte(ulong *s, int idx, uchar val) {
    int word = idx / 8;
    int shift = (idx % 8) * 8;
    ulong mask = 0xFFUL << shift;
    s[word] = (s[word] & ~mask) | ((ulong)val << shift);
}

static inline uchar get_byte(const ulong *s, int idx) {
    int word = idx / 8;
    int shift = (idx % 8) * 8;
    return (uchar)((s[word] >> shift) & 0xFFUL);
}

void keccak_f1600(ulong *s) {
    for (int round = 0; round < 24; round++) {
        ulong C0 = s[0] ^ s[5] ^ s[10] ^ s[15] ^ s[20];
        ulong C1 = s[1] ^ s[6] ^ s[11] ^ s[16] ^ s[21];
        ulong C2 = s[2] ^ s[7] ^ s[12] ^ s[17] ^ s[22];
        ulong C3 = s[3] ^ s[8] ^ s[13] ^ s[18] ^ s[23];
        ulong C4 = s[4] ^ s[9] ^ s[14] ^ s[19] ^ s[24];

        ulong D0 = rotl64(C1, 1) ^ C4;
        ulong D1 = rotl64(C2, 1) ^ C0;
        ulong D2 = rotl64(C3, 1) ^ C1;
        ulong D3 = rotl64(C4, 1) ^ C2;
        ulong D4 = rotl64(C0, 1) ^ C3;

        s[0] ^= D0; s[5] ^= D0; s[10] ^= D0; s[15] ^= D0; s[20] ^= D0;
        s[1] ^= D1; s[6] ^= D1; s[11] ^= D1; s[16] ^= D1; s[21] ^= D1;
        s[2] ^= D2; s[7] ^= D2; s[12] ^= D2; s[17] ^= D2; s[22] ^= D2;
        s[3] ^= D3; s[8] ^= D3; s[13] ^= D3; s[18] ^= D3; s[23] ^= D3;
        s[4] ^= D4; s[9] ^= D4; s[14] ^= D4; s[19] ^= D4; s[24] ^= D4;

        ulong B0  = s[0];
        ulong B10 = rotl64(s[1], 1);
        ulong B20 = rotl64(s[2], 62);
        ulong B5  = rotl64(s[3], 28);
        ulong B15 = rotl64(s[4], 27);

        ulong B16 = rotl64(s[5], 36);
        ulong B1  = rotl64(s[6], 44);
        ulong B11 = rotl64(s[7], 6);
        ulong B21 = rotl64(s[8], 55);
        ulong B6  = rotl64(s[9], 20);

        ulong B7  = rotl64(s[10], 3);
        ulong B17 = rotl64(s[11], 10);
        ulong B2  = rotl64(s[12], 43);
        ulong B12 = rotl64(s[13], 25);
        ulong B22 = rotl64(s[14], 39);

        ulong B23 = rotl64(s[15], 41);
        ulong B8  = rotl64(s[16], 45);
        ulong B18 = rotl64(s[17], 15);
        ulong B3  = rotl64(s[18], 21);
        ulong B13 = rotl64(s[19], 8);

        ulong B14 = rotl64(s[20], 18);
        ulong B24 = rotl64(s[21], 2);
        ulong B9  = rotl64(s[22], 61);
        ulong B19 = rotl64(s[23], 56);
        ulong B4  = rotl64(s[24], 14);

        s[0]  = B0  ^ (~B1  & B2 );
        s[1]  = B1  ^ (~B2  & B3 );
        s[2]  = B2  ^ (~B3  & B4 );
        s[3]  = B3  ^ (~B4  & B0 );
        s[4]  = B4  ^ (~B0  & B1 );
        s[5]  = B5  ^ (~B6  & B7 );
        s[6]  = B6  ^ (~B7  & B8 );
        s[7]  = B7  ^ (~B8  & B9 );
        s[8]  = B8  ^ (~B9  & B5 );
        s[9]  = B9  ^ (~B5  & B6 );
        s[10] = B10 ^ (~B11 & B12);
        s[11] = B11 ^ (~B12 & B13);
        s[12] = B12 ^ (~B13 & B14);
        s[13] = B13 ^ (~B14 & B10);
        s[14] = B14 ^ (~B10 & B11);
        s[15] = B15 ^ (~B16 & B17);
        s[16] = B16 ^ (~B17 & B18);
        s[17] = B17 ^ (~B18 & B19);
        s[18] = B18 ^ (~B19 & B15);
        s[19] = B19 ^ (~B15 & B16);
        s[20] = B20 ^ (~B21 & B22);
        s[21] = B21 ^ (~B22 & B23);
        s[22] = B22 ^ (~B23 & B24);
        s[23] = B23 ^ (~B24 & B20);
        s[24] = B24 ^ (~B20 & B21);

        s[0] ^= RC[round];
    }
}

__kernel void hash256_mine(
    __global const uchar *challenge,
    __global const uchar *difficulty,
    ulong base_nonce,
    uint batch_size,
    __global ulong *result_nonce,
    __global int *found_flag
) {
    ulong gid = get_global_id(0);
    ulong start = base_nonce + gid * (ulong)batch_size;

    uchar input[64];
    for (int i = 0; i < 32; i++) input[i] = challenge[i];

    for (uint offset = 0; offset < batch_size; offset++) {
        if (*found_flag) return;

        ulong nonce = start + (ulong)offset;

        #pragma unroll
        for (int i = 0; i < 8; i++) {
            input[63 - i] = (uchar)(nonce >> (i * 8));
        }
        #pragma unroll
        for (int i = 8; i < 32; i++) {
            input[63 - i] = 0;
        }

        ulong st[25];
        for (int i = 0; i < 25; i++) st[i] = 0;

        for (int i = 0; i < 64; i++) {
            uchar b = input[i];
            int word = i / 8;
            int shift = (i % 8) * 8;
            st[word] ^= ((ulong)b) << shift;
        }

        st[8] ^= 0x01UL;
        st[16] ^= 0x8000000000000000UL;

        keccak_f1600(st);

        bool lt = false;
        bool gt = false;
        for (int i = 0; i < 32; i++) {
            uchar hb = get_byte(st, i);
            uchar db = difficulty[i];
            if (hb < db) { lt = true; break; }
            if (hb > db) { gt = true; break; }
        }

        if (lt && !gt) {
            int was_found = atomic_xchg(found_flag, 1);
            if (was_found == 0) {
                *result_nonce = nonce;
            }
            return;
        }
    }
}
"#;

#[derive(Clone)]
struct Work {
    challenge: [u8; 32],
    difficulty_bytes: [u8; 32],
    difficulty: U256,
}

/// Claim engine: sends mine() TX to ALL providers in parallel.
struct ClaimEngine {
    /// MEV/Flashbots endpoints — fired FIRST (priority, faster block inclusion)
    priority_contracts: Vec<(String, Contract<SignerMiddleware<Provider<Http>, LocalWallet>>)>,
    /// Regular RPC endpoints
    contracts: Vec<(String, Contract<SignerMiddleware<Provider<Http>, LocalWallet>>)>,
    gas_price: U256,
    gas_limit: U256,
    /// Cached gas estimate + when it was last refreshed
    cached_gas_limit: std::sync::Mutex<(U256, Instant)>,
    /// How often to refresh gas estimate (default: 5 min)
    gas_cache_ttl: Duration,
    /// Raw HTTP client for fastest possible broadcast (HTTP/2, connection pooling)
    http_client: reqwest::Client,
    /// Raw RPC URLs for direct JSON-RPC broadcast
    rpc_urls: Vec<String>,
    /// Wallet for raw signing
    wallet: LocalWallet,
    /// Chain ID
    chain_id: u64,
    /// Contract address
    contract_addr: Address,
}

impl ClaimEngine {
    /// Get gas limit — returns cached estimate if fresh, otherwise re-estimates.
    async fn gas_limit_for(&self, nonce: u64) -> U256 {
        // Check cache first
        {
            let cache = self.cached_gas_limit.lock().unwrap();
            if cache.1.elapsed() < self.gas_cache_ttl {
                return cache.0;
            }
        }
        // Cache stale — re-estimate
        if let Some(estimated) = self.estimate_gas_now(nonce).await {
            let mut cache = self.cached_gas_limit.lock().unwrap();
            *cache = (estimated, Instant::now());
            estimated
        } else {
            self.gas_limit // fallback
        }
    }

    /// Actually call eth_estimateGas (slow — RPC roundtrip).
    async fn estimate_gas_now(&self, nonce: u64) -> Option<U256> {
        let c = &self.contracts.first()?.1;
        let call = c.method::<U256, ()>("mine", U256::from(nonce)).ok()?;
        let estimated = call.estimate_gas().await.ok()?;
        Some(estimated * U256::from(120) / U256::from(100)) // +20% buffer
    }

    /// Fire mine(nonce) to ALL providers (MEV first, then regular).
    /// Returns on first MEV success — don't wait for regular RPCs.
    async fn claim(&self, nonce: u64) -> (usize, usize) {
        let nonce_u256 = U256::from(nonce);

        // Get gas limit (cached)
        let gas_limit = self.gas_limit_for(nonce).await;

        // Fire MEV/Flashbots FIRST — these are the critical ones
        let mut mev_futs = Vec::new();
        for (label, c) in &self.priority_contracts {
            let c = c.clone();
            let label = label.to_string();
            let gp = self.gas_price;
            let gl = gas_limit;
            mev_futs.push(async move {
                match c.method::<U256, ()>("mine", nonce_u256) {
                    Ok(call) => match call.gas_price(gp).gas(gl).send().await {
                        Ok(pending) => {
                            let hash = pending.tx_hash();
                            log_line(&format!("  ✅ [{}] sent | tx=0x{:x}", label, hash));
                            Ok(label)
                        }
                        Err(e) => {
                            log_line(&format!("  ⚠️ [{}] failed: {}", label, e));
                            Err(label)
                        }
                    },
                    Err(e) => {
                        log_line(&format!("  ❌ [{}] build: {}", label, e));
                        Err(label)
                    }
                }
            });
        }

        // Wait for FIRST MEV success — then return immediately
        let mev_results = futures::future::join_all(mev_futs).await;
        let mev_ok = mev_results.iter().filter(|r| r.is_ok()).count();
        let mev_fail = mev_results.len() - mev_ok;

        // Fire regular RPCs in background (don't wait)
        let mut regular_futs = Vec::new();
        for (label, c) in &self.contracts {
            let c = c.clone();
            let label = label.to_string();
            let gp = self.gas_price;
            let gl = gas_limit;
            regular_futs.push(async move {
                match c.method::<U256, ()>("mine", nonce_u256) {
                    Ok(call) => match call.gas_price(gp).gas(gl).send().await {
                        Ok(pending) => {
                            let hash = pending.tx_hash();
                            log_line(&format!("  ✅ [{}] sent | tx=0x{:x}", label, hash));
                            Ok(label)
                        }
                        Err(e) => {
                            log_line(&format!("  ⚠️ [{}] failed: {}", label, e));
                            Err(label)
                        }
                    },
                    Err(e) => {
                        log_line(&format!("  ❌ [{}] build: {}", label, e));
                        Err(label)
                    }
                }
            });
        }
        // Spawn regular RPCs in background — don't block claim
        tokio::spawn(async move {
            let results = futures::future::join_all(regular_futs).await;
            let ok = results.iter().filter(|r| r.is_ok()).count();
            let fail = results.len() - ok;
            if fail > 0 {
                log_line(&format!("  📡 Regular RPCs: {}/{} OK", ok, ok + fail));
            }
        });

        (mev_ok, mev_fail)
    }

    /// Ultra-fast claim: sign TX, broadcast raw JSON-RPC via reqwest (no ethers overhead).
    async fn fast_claim(&self, nonce: u64) -> (usize, usize) {
        let gas_limit = self.gas_limit_for(nonce).await;

        // Build calldata: selector(4) + nonce(32)
        let selector: [u8; 4] = [0xfe, 0x0c, 0x46, 0xaf];
        let mut call_data = Vec::with_capacity(36);
        call_data.extend_from_slice(&selector);
        let mut nonce_buf = [0u8; 32];
        U256::from(nonce).to_big_endian(&mut nonce_buf);
        call_data.extend_from_slice(&nonce_buf);

        // Build raw calldata JSON-RPC call (bypass ethers TX signing entirely)
        let calldata_hex = format!("0x{}", hex::encode(&call_data));
        let from_hex = format!("0x{:x}", self.wallet.address());
        let to_hex = format!("0x{:x}", self.contract_addr);
        let gas_price_hex = format!("0x{:x}", self.gas_price);
        let gas_limit_hex = format!("0x{:x}", gas_limit);

        // Use eth_sendTransaction (node will sign via unlocked account)
        // Or use raw JSON-RPC with pre-built calldata
        let payload = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_sendTransaction",
            "params": [{
                "from": from_hex,
                "to": to_hex,
                "data": calldata_hex,
                "gasPrice": gas_price_hex,
                "gas": gas_limit_hex,
                "value": "0x0"
            }],
            "id": 1
        });


        // Fire to ALL providers simultaneously via reqwest (HTTP/2, connection pooling)
        let futs: Vec<_> = self.rpc_urls.iter().map(|url| {
            let client = self.http_client.clone();
            let payload = payload.clone();
            let url = url.clone();
            async move {
                match client.post(&url).json(&payload).send().await {
                    Ok(resp) => {
                        let status = resp.status();
                        if status.is_success() {
                            let body: serde_json::Value = resp.json().await.unwrap_or_default();
                            if let Some(hash) = body.get("result") {
                                log_line(&format!("  ✅ [{}] sent | tx={}", &url[..url.len().min(40)], hash));
                                return Ok(url);
                            }
                            if let Some(err) = body.get("error") {
                                log_line(&format!("  ⚠️ [{}] rpc: {}", &url[..url.len().min(40)], err));
                                return Err(url);
                            }
                        }
                        log_line(&format!("  ⚠️ [{}] http {}", &url[..url.len().min(40)], status));
                        Err(url)
                    }
                    Err(e) => {
                        log_line(&format!("  ❌ [{}] net: {}", &url[..url.len().min(40)], e));
                        Err(url)
                    }
                }
            }
        }).collect();

        let results = futures::future::join_all(futs).await;
        let ok = results.iter().filter(|r| r.is_ok()).count();
        let fail = results.len() - ok;
        (ok, fail)
    }
}

fn log_line(msg: &str) {
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    let line = format!("[{}] {}\n", timestamp, msg);
    print!("{}", line);
    let _ = OpenOptions::new()
        .create(true)
        .append(true)
        .open("/root/hash256-gpu-miner/miner.log")
        .and_then(|mut f| f.write_all(line.as_bytes()));
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();

    if env::var("TEST_MODE").unwrap_or_default() == "1" {
        return run_test_mode().await;
    }

    let private_key = env::var("PRIVATE_KEY").context("Isi PRIVATE_KEY di .env")?;
    if !private_key.starts_with("0x") {
        anyhow::bail!("PRIVATE_KEY harus diawali 0x");
    }

    let wallet: LocalWallet = private_key.parse()?;
    let address: Address = CONTRACT_ADDRESS.parse()?;
    let abi: Abi = serde_json::from_str(ABI_JSON)?;

    // Build Contract instances for ALL RPC providers (MEV/Flashbots FIRST, then regular)
    let mut priority_contracts: Vec<(String, Contract<SignerMiddleware<Provider<Http>, LocalWallet>>)> = Vec::new();
    let mut regular_contracts: Vec<(String, Contract<SignerMiddleware<Provider<Http>, LocalWallet>>)> = Vec::new();

    // MEV/Flashbots endpoints (priority — fire first for faster block inclusion)
    for key in &["RPC_MEV_1", "RPC_MEV_2", "RPC_MEV_3", "RPC_MEV_4"] {
        if let Ok(url) = env::var(key) {
            if !url.trim().is_empty() {
                match Provider::<Http>::try_from(url.clone()) {
                    Ok(p) => {
                        let client = Arc::new(SignerMiddleware::new(p, wallet.clone()));
                        let contract = Contract::new(address, abi.clone(), client);
                        log_line(&format!("  🔒 MEV RPC: {} ({})", key, &url[..url.len().min(50)]));
                        priority_contracts.push((key.to_string(), contract));
                    }
                    Err(e) => log_line(&format!("  ⚠️ MEV skip ({}): {}", key, e)),
                }
            }
        }
    }

    // Regular RPC endpoints
    for key in &["RPC_URL", "RPC_URL_2", "RPC_URL_3", "RPC_URL_4", "RPC_URL_5", "RPC_URL_6", "RPC_URL_7", "RPC_URL_8", "RPC_URL_9", "RPC_URL_10"] {
        if let Ok(url) = env::var(key) {
            if !url.trim().is_empty() {
                match Provider::<Http>::try_from(url.clone()) {
                    Ok(p) => {
                        let client = Arc::new(SignerMiddleware::new(p, wallet.clone()));
                        let contract = Contract::new(address, abi.clone(), client);
                        let label = key.to_string();
                        log_line(&format!("  🌐 RPC: {} ({})", key, &url[..url.len().min(50)]));
                        regular_contracts.push((label, contract));
                    }
                    Err(e) => log_line(&format!("  ⚠️ RPC skip ({}): {}", key, e)),
                }
            }
        }
    }

    let total_rpcs = priority_contracts.len() + regular_contracts.len();
    if total_rpcs == 0 {
        anyhow::bail!("Minimal 1 RPC_URL diperlukan di .env");
    }

    // Primary contract for reads (miningState, getChallenge) — use first regular
    let read_contract = if !regular_contracts.is_empty() {
        regular_contracts[0].1.clone()
    } else {
        priority_contracts[0].1.clone()
    };

    let gas_gwei: u64 = env::var("GAS_GWEI").ok().and_then(|v| v.parse().ok()).unwrap_or(5);
    let gas_limit_u64: u64 = env::var("GAS_LIMIT").ok().and_then(|v| v.parse().ok()).unwrap_or(200_000);
    let gas_cache_secs: u64 = env::var("GAS_CACHE_TTL").ok().and_then(|v| v.parse().ok()).unwrap_or(300); // 5 min default

    let mev_count = priority_contracts.len();
    let rpc_count = regular_contracts.len();

    // Collect raw RPC URLs for fast_claim
    let mut rpc_urls: Vec<String> = Vec::new();
    for key in &["RPC_MEV_1", "RPC_MEV_2", "RPC_URL", "RPC_URL_2", "RPC_URL_3", "RPC_URL_4", "RPC_URL_5", "RPC_URL_6", "RPC_URL_7", "RPC_URL_8", "RPC_URL_9", "RPC_URL_10"] {
        if let Ok(url) = env::var(key) {
            if !url.trim().is_empty() {
                rpc_urls.push(url);
            }
        }
    }

    let http_client = reqwest::Client::builder()
        .http2_keep_alive_interval(Duration::from_secs(30))
        .pool_max_idle_per_host(4)
        .tcp_keepalive(Duration::from_secs(60))
        .build()
        .expect("reqwest client build failed");

    let default_gas = U256::from(gas_limit_u64);

    // Get chain_id from first regular contract before moving
    let chain_id = if !regular_contracts.is_empty() {
        regular_contracts[0].1.client().provider().get_chainid().await.unwrap_or(U256::from(1)).as_u64()
    } else {
        priority_contracts[0].1.client().provider().get_chainid().await.unwrap_or(U256::from(1)).as_u64()
    };

    let engine = Arc::new(ClaimEngine {
        priority_contracts,
        contracts: regular_contracts,
        gas_price: U256::from(gas_gwei.saturating_mul(1_000_000_000u64)),
        gas_limit: default_gas,
        cached_gas_limit: std::sync::Mutex::new((default_gas, Instant::now() - Duration::from_secs(gas_cache_secs + 1))),
        gas_cache_ttl: Duration::from_secs(gas_cache_secs),
        http_client,
        rpc_urls,
        wallet: wallet.clone(),
        chain_id,
        contract_addr: address,
    });

    log_line(&format!("Wallet: 0x{:x}", wallet.address()));
    log_line(&format!("Contract: {}", CONTRACT_ADDRESS));
    log_line(&format!("⚡ FCFS: {} MEV + {} RPC | {} Gwei | dynamic gas",
        mev_count, rpc_count, gas_gwei));

    let gpus = setup_opencl_all()?;
    if gpus.is_empty() {
        anyhow::bail!("GPU OpenCL diperlukan. Tidak ada device yang tersedia.");
    }
    log_line(&format!("GPU OpenCL tersedia: {} device", gpus.len()));
    let stop_flag = Arc::new(AtomicBool::new(false));

    loop {
        let mining_state = match read_contract
            .method::<(), (U256, U256, U256, U256, U256, U256, U256)>("miningState", ())?
            .call()
            .await {
            Ok(s) => s,
            Err(e) => {
                log_line(&format!("RPC error (miningState): {}, retry in 5s...", e));
                sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        let difficulty = mining_state.2;
        let challenge_h256: H256 = match read_contract
            .method::<Address, H256>("getChallenge", wallet.address())?
            .call()
            .await {
            Ok(c) => c,
            Err(e) => {
                log_line(&format!("RPC error (getChallenge): {}, retry in 5s...", e));
                sleep(Duration::from_secs(5)).await;
                continue;
            }
        };
        let challenge_bytes = challenge_h256.to_fixed_bytes();

        let mut difficulty_bytes = [0u8; 32];
        difficulty.to_big_endian(&mut difficulty_bytes);

        log_line("");
        log_line(&format!("Era: {}", mining_state.0));
        log_line(&format!("Reward: {} HASH", format_units(mining_state.1, 18)?));
        log_line(&format!("Difficulty: {}", difficulty));
        log_line(&format!("Epoch: {}", mining_state.5));
        log_line(&format!("Challenge: 0x{}", hex::encode(challenge_bytes)));

        let work = Work {
            challenge: challenge_bytes,
            difficulty_bytes,
            difficulty,
        };

        let result = mine_gpu_multi(&gpus, &work, engine.clone()).await;

        match result {
            Some(_nonce) => {
                // Claim is now done directly inside mine_gpu_multi — log handled there
            }
            None => {
                log_line("Challenge berubah atau stopped, restart mining...");
            }
        }

        stop_flag.store(false, Ordering::SeqCst);
        sleep(Duration::from_millis(100)).await; // reduced from 500ms to 100ms
    }
}

fn verify_nonce(challenge: [u8; 32], nonce: u64, difficulty: [u8; 32]) -> bool {
    let mut input = [0u8; 64];
    input[..32].copy_from_slice(&challenge);
    U256::from(nonce).to_big_endian(&mut input[32..]);
    let hash = Keccak256::digest(&input);
    let ok = hash.as_slice() < difficulty.as_slice();
    log_line(&format!("  verify hash: 0x{} (ok={})", hex::encode(&hash), ok));
    ok
}

async fn run_test_mode() -> Result<()> {
    log_line("=== TEST MODE ===");
    let mut challenge = [0u8; 32];
    challenge[0] = 0xAB;
    challenge[1] = 0xCD;
    challenge[2] = 0xEF;
    challenge[31] = 0x01;

    let mut difficulty_bytes = [0xFFu8; 32];
    difficulty_bytes[0] = 0x00;
    let difficulty = U256::MAX;

    log_line(&format!("Challenge : 0x{}", hex::encode(challenge)));
    log_line(&format!("Difficulty: 0x{} (MAX => any nonce passes)", hex::encode(difficulty_bytes)));

    let work = Work {
        challenge,
        difficulty_bytes,
        difficulty,
    };

    let start = Instant::now();

    let gpus = setup_opencl_all().unwrap_or_default();
    if gpus.is_empty() {
        anyhow::bail!("GPU OpenCL required but unavailable");
    }
    log_line(&format!("TEST: {} GPU(s) detected", gpus.len()));
    let test_engine = Arc::new(ClaimEngine {
        priority_contracts: Vec::new(),
        contracts: Vec::new(),
        gas_price: U256::from(5u64.saturating_mul(1_000_000_000u64)),
        gas_limit: U256::from(200_000u64),
        cached_gas_limit: std::sync::Mutex::new((U256::from(200_000u64), Instant::now())),
        gas_cache_ttl: Duration::from_secs(300),
        http_client: reqwest::Client::new(),
        rpc_urls: Vec::new(),
        wallet: LocalWallet::new(&mut rand::thread_rng()),
        chain_id: 1,
        contract_addr: Address::zero(),
    });
    let nonce = mine_gpu_multi(&gpus, &work, test_engine).await;

    let elapsed = start.elapsed();
    match nonce {
        Some(n) => {
            log_line(&format!("\nNonce found: {} (elapsed: {:?})", n, elapsed));
            let mut input = [0u8; 64];
            input[..32].copy_from_slice(&challenge);
            U256::from(n).to_big_endian(&mut input[32..]);
            let hash = Keccak256::digest(&input);
            log_line(&format!("CPU hash  : 0x{}", hex::encode(&hash)));
            log_line(&format!("Difficult : 0x{}", hex::encode(difficulty_bytes)));
            let ok = hash.as_slice() < difficulty_bytes.as_slice();
            log_line(&format!("Result    : {}", if ok { "PASS" } else { "FAIL" }));
            if !ok {
                log_line("WARNING: GPU/CPU hash mismatch or nonce ordering bug!");
            }
        }
        None => {
            log_line("No nonce returned (GPU error or stopped).");
        }
    }
    log_line("=== END TEST ===");
    Ok(())
}

/// Enumerate all OpenCL GPU devices and return a ProQue per device.
fn setup_opencl_all() -> Result<Vec<ProQue>> {
    let mut gpus = Vec::new();
    for platform in Platform::list() {
        for device in Device::list_all(platform)? {
            let name = device.name().unwrap_or_else(|_| "unknown".into());
            let proque = ProQue::builder()
                .src(KERNEL_SRC)
                .device(device)
                .build()?;
            log_line(&format!("  OpenCL device: {} (platform {:?})", name, platform));
            gpus.push(proque);
        }
    }
    Ok(gpus)
}

/// Single-GPU mining loop — runs on one device until nonce found or stop_flag set.
fn mine_one_gpu(
    gpu_id: usize,
    proque: ProQue,
    work: &Work,
    stop_flag: Arc<AtomicBool>,
    engine: Arc<ClaimEngine>,
) -> tokio::sync::oneshot::Receiver<Option<u64>> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    let challenge_buf: Buffer<u8> = Buffer::builder()
        .queue(proque.queue().clone())
        .flags(ocl::flags::MEM_READ_ONLY)
        .len(32)
        .copy_host_slice(&work.challenge)
        .build()
        .expect("GPU buffer alloc failed");

    let diff_buf: Buffer<u8> = Buffer::builder()
        .queue(proque.queue().clone())
        .flags(ocl::flags::MEM_READ_ONLY)
        .len(32)
        .copy_host_slice(&work.difficulty_bytes)
        .build()
        .expect("GPU buffer alloc failed");

    let result_buf: Buffer<u64> = Buffer::builder()
        .queue(proque.queue().clone())
        .flags(ocl::flags::MEM_WRITE_ONLY)
        .len(1)
        .fill_val(0u64)
        .build()
        .expect("GPU buffer alloc failed");

    let found_buf: Buffer<i32> = Buffer::builder()
        .queue(proque.queue().clone())
        .flags(ocl::flags::MEM_READ_WRITE)
        .len(1)
        .fill_val(0i32)
        .build()
        .expect("GPU buffer alloc failed");

    let mut base_nonce: u64 = random();
    let work_size: usize = 1 << 20; // 1M work items
    let batch_size: u32 = 1024;     // 1024 nonces per work-item
    let hashes_per_launch = (work_size as u64) * (batch_size as u64);

    let total_hashes = Arc::new(AtomicU64::new(0));
    let th_clone = total_hashes.clone();
    let sf = stop_flag.clone();
    let gpu_id_clone = gpu_id;

    // Hashrate logger per GPU
    let hashrate_handle = tokio::spawn(async move {
        let mut last_hashes = 0u64;
        let mut last_time = Instant::now();
        let log_interval = Duration::from_secs(10);
        loop {
            sleep(log_interval).await;
            if sf.load(Ordering::Relaxed) { break; }
            let cur = th_clone.load(Ordering::Relaxed);
            let elapsed = last_time.elapsed().as_secs_f64();
            if elapsed > 0.0 {
                let delta = cur.saturating_sub(last_hashes);
                let mhps = (delta as f64) / elapsed / 1_000_000.0;
                log_line(&format!(
                    "[GPU{}] {:.2} MH/s | total: {}",
                    gpu_id_clone, mhps, cur,
                ));
                last_hashes = cur;
                last_time = Instant::now();
            }
        }
    });

    // Spawn blocking OpenCL loop on a dedicated thread (ocl is blocking)
    std::thread::spawn(move || {
        loop {
            if stop_flag.load(Ordering::Relaxed) {
                hashrate_handle.abort();
                let _ = tx.send(None);
                return;
            }

            let kernel = proque
                .kernel_builder("hash256_mine")
                .arg(&challenge_buf)
                .arg(&diff_buf)
                .arg(base_nonce)
                .arg(batch_size)
                .arg(&result_buf)
                .arg(&found_buf)
                .global_work_size(work_size)
                .build()
                .expect("kernel build failed");

            unsafe { kernel.enq().expect("kernel enqueue failed"); }

            total_hashes.fetch_add(hashes_per_launch, Ordering::Relaxed);

            let mut found = vec![0i32; 1];
            found_buf.read(&mut found).enq().expect("read found_buf failed");
            if found[0] != 0 {
                let mut result = vec![0u64; 1];
                result_buf.read(&mut result).enq().expect("read result_buf failed");
                let nonce = result[0];
                stop_flag.store(true, Ordering::SeqCst);
                hashrate_handle.abort();

                // === FCFS: Claim DIRECTLY from GPU thread (zero hop) ===
                let t_claim = Instant::now();
                let runtime = tokio::runtime::Handle::current();
                let (ok, fail) = runtime.block_on(engine.fast_claim(nonce));
                let claim_time = t_claim.elapsed();
                log_line(&format!(
                    "🏆 CLAIM DONE | nonce={} | {}/{} OK | claim={:?} | {}",
                    nonce, ok, ok + fail, claim_time,
                    if fail == 0 { "all sent ✅".to_string() } else { format!("{} errors ⚠️", fail) }
                ));

                let _ = tx.send(Some(nonce));
                return;
            }

            base_nonce = base_nonce.wrapping_add(hashes_per_launch);
        }
    });

    rx
}

/// Multi-GPU mining: launch one mine_one_gpu per device, return first nonce found.
async fn mine_gpu_multi(gpus: &[ProQue], work: &Work, engine: Arc<ClaimEngine>) -> Option<u64> {
    let stop_flag = Arc::new(AtomicBool::new(false));
    let mut handles = Vec::new();

    for (i, gpu) in gpus.iter().enumerate() {
        let rx = mine_one_gpu(i, gpu.clone(), work, stop_flag.clone(), engine.clone());
        handles.push(rx);
    }

    // Wait for any GPU to return a nonce (first one wins)
    let mut set = tokio::task::JoinSet::new();
    for rx in handles {
        set.spawn(async move { rx.await });
    }

    while let Some(res) = set.join_next().await {
        match res {
            Ok(Ok(Some(nonce))) => {
                stop_flag.store(true, Ordering::SeqCst);
                // Drain remaining receivers so their threads exit
                while set.join_next().await.is_some() {}
                return Some(nonce);
            }
            _ => continue,
        }
    }
    None
}

