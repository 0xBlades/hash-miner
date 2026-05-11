use anyhow::Result;
use anyhow::Context as _;
use dotenv::dotenv;
use ethers::abi::Abi;
use ethers::contract::Contract;
use ethers::middleware::SignerMiddleware;
use ethers::providers::{Http, Provider};
use ethers::signers::{LocalWallet, Signer};
use ethers::types::{Address, H256, U256, transaction::eip2718::TypedTransaction};
use ethers::utils::format_units;
use ethers::middleware::Middleware;
use ocl::{Buffer, ProQue};
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

struct Work {
    challenge: [u8; 32],
    difficulty_bytes: [u8; 32],
    difficulty: U256,
}

/// Pre-encoded call data for mine(uint256) — avoids re-encoding every time
struct PreparedCall {
    selector: [u8; 4],       // 0xfe0c46af
    chain_id: u64,
    contract_addr: Address,
    gas_price: U256,         // 1 Gwei
    gas_limit: U256,         // 150,000
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

    let rpc_url = env::var("RPC_URL").context("Isi RPC_URL di .env")?;
    let private_key = env::var("PRIVATE_KEY").context("Isi PRIVATE_KEY di .env")?;
    if !private_key.starts_with("0x") {
        anyhow::bail!("PRIVATE_KEY harus diawali 0x");
    }

    let provider = Provider::<Http>::try_from(rpc_url)?;
    let wallet: LocalWallet = private_key.parse()?;
    let client = Arc::new(SignerMiddleware::new(provider, wallet.clone()));
    let address: Address = CONTRACT_ADDRESS.parse()?;
    let abi: Abi = serde_json::from_str(ABI_JSON)?;
    let contract = Contract::new(address, abi, client.clone());

    log_line(&format!("Wallet: 0x{:x}", wallet.address()));
    log_line(&format!("Contract: {}", CONTRACT_ADDRESS));
    log_line("⚡ FCFS mode: skip CPU verify, pre-signed TX, 1 Gwei gas");

    let gpu = setup_opencl();
    match &gpu {
        Ok(_) => log_line("GPU OpenCL tersedia."),
        Err(e) => {
            log_line(&format!("GPU tidak tersedia: {}", e));
            anyhow::bail!("GPU OpenCL diperlukan. Fallback CPU dinonaktifkan.");
        }
    }

    // Pre-prepare call data template (mine selector = 0xfe0c46af)
    let chain_id = client.provider().get_chainid().await.unwrap_or(U256::from(1)).as_u64();
    let prepared = PreparedCall {
        selector: [0xfe, 0x0c, 0x46, 0xaf],
        chain_id,
        contract_addr: address,
        gas_price: U256::from(1_000_000_000u64), // 1 Gwei — fast inclusion
        gas_limit: U256::from(150_000u64),
    };

    let stop_flag = Arc::new(AtomicBool::new(false));

    loop {
        let mining_state = match contract
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
        let challenge_h256: H256 = match contract
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

        let start_time = Instant::now();

        let result = mine_gpu(gpu.as_ref().unwrap(), &work, stop_flag.clone()).await;

        match result {
            Some(nonce) => {
                let elapsed = start_time.elapsed();

                // === FCFS: Sign + broadcast as fast as possible ===
                let t_sign_start = Instant::now();

                // Build call data: selector(4) + nonce(32)
                let mut call_data = Vec::with_capacity(36);
                call_data.extend_from_slice(&prepared.selector);
                let mut nonce_buf = [0u8; 32];
                U256::from(nonce).to_big_endian(&mut nonce_buf);
                call_data.extend_from_slice(&nonce_buf);

                // Use contract method with pre-set gas price — clone to avoid borrow issues
                let t_signed = t_sign_start.elapsed();

                let call = match contract.clone().method::<U256, ()>("mine", U256::from(nonce)) {
                    Ok(c) => c.gas_price(prepared.gas_price).gas(prepared.gas_limit),
                    Err(e) => {
                        log_line(&format!("❌ CLAIM FAILED | nonce={} | error={}", nonce, e));
                        continue;
                    }
                };

                // Send TX — just broadcast, don't await receipt (FCFS speed)
                let tx_result = call.send().await;
                match tx_result {
                    Ok(pending) => {
                        let tx_hash = pending.tx_hash();
                        let eth_link = format!("https://etherscan.io/tx/0x{:x}", tx_hash);
                        log_line(&format!("✅ CLAIM OK | nonce={} | tx=0x{:x} | link={} | elapsed={:?}", nonce, tx_hash, eth_link, elapsed));
                    }
                    Err(e) => {
                        log_line(&format!("❌ CLAIM FAILED | nonce={} | error={}", nonce, e));
                    }
                }
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

    let stop_flag = Arc::new(AtomicBool::new(false));
    let start = Instant::now();

    let nonce = match setup_opencl() {
        Ok(ref gpu) => {
            log_line("GPU available, running mine_gpu...");
            mine_gpu(gpu, &work, stop_flag.clone()).await
        }
        Err(e) => {
            anyhow::bail!("GPU OpenCL required but unavailable: {}", e);
        }
    };

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

fn setup_opencl() -> Result<ProQue> {
    Ok(ProQue::builder().src(KERNEL_SRC).build()?)
}

async fn mine_gpu(proque: &ProQue, work: &Work, stop_flag: Arc<AtomicBool>) -> Option<u64> {
    let challenge_buf: Buffer<u8> = Buffer::builder()
        .queue(proque.queue().clone())
        .flags(ocl::flags::MEM_READ_ONLY)
        .len(32)
        .copy_host_slice(&work.challenge)
        .build()
        .ok()?;

    let diff_buf: Buffer<u8> = Buffer::builder()
        .queue(proque.queue().clone())
        .flags(ocl::flags::MEM_READ_ONLY)
        .len(32)
        .copy_host_slice(&work.difficulty_bytes)
        .build()
        .ok()?;

    let result_buf: Buffer<u64> = Buffer::builder()
        .queue(proque.queue().clone())
        .flags(ocl::flags::MEM_WRITE_ONLY)
        .len(1)
        .fill_val(0u64)
        .build()
        .ok()?;

    let found_buf: Buffer<i32> = Buffer::builder()
        .queue(proque.queue().clone())
        .flags(ocl::flags::MEM_READ_WRITE)
        .len(1)
        .fill_val(0i32)
        .build()
        .ok()?;

    let mut base_nonce: u64 = random();
    let work_size: usize = 1 << 20; // 1.048.576 work items
    let batch_size: u32 = 1024;     // 1.024 nonces per work-item
    let hashes_per_launch = (work_size as u64) * (batch_size as u64);

    let total_hashes = Arc::new(AtomicU64::new(0));
    let total_hashes_clone = total_hashes.clone();
    let stop_flag_clone = stop_flag.clone();

    // Spawn hashrate logger task
    let hashrate_handle = tokio::spawn(async move {
        let mut last_hashes = 0u64;
        let mut last_time = Instant::now();
        let log_interval = Duration::from_secs(10);

        loop {
            sleep(log_interval).await;
            if stop_flag_clone.load(Ordering::Relaxed) {
                break;
            }
            let current_hashes = total_hashes_clone.load(Ordering::Relaxed);
            let elapsed = last_time.elapsed().as_secs_f64();
            if elapsed > 0.0 {
                let hashes_delta = current_hashes.saturating_sub(last_hashes);
                let mhps = (hashes_delta as f64) / elapsed / 1_000_000.0;
                log_line(&format!(
                    "[HASHRATE] {:.2} MH/s | Total hashes: {} | Elapsed: {:.1}s",
                    mhps,
                    current_hashes,
                    last_time.elapsed().as_secs_f64()
                ));
                last_hashes = current_hashes;
                last_time = Instant::now();
            }
        }
    });

    loop {
        if stop_flag.load(Ordering::Relaxed) {
            let _ = hashrate_handle.await;
            return None;
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
            .ok()?;

        unsafe { kernel.enq().ok()?; }

        total_hashes.fetch_add(hashes_per_launch, Ordering::Relaxed);

        let mut found = vec![0i32; 1];
        found_buf.read(&mut found).enq().ok()?;

        if found[0] != 0 {
            let mut result = vec![0u64; 1];
            result_buf.read(&mut result).enq().ok()?;
            stop_flag.store(true, Ordering::Relaxed);
            let _ = hashrate_handle.await;
            return Some(result[0]);
        }

        base_nonce = base_nonce.wrapping_add(hashes_per_launch);
    }
}
