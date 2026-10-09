//! 随机数来源：不填种子时用系统的安全随机数；填了种子时结果可复现，任何人用同样的种子与输入都能验证。
//!
//! 可复现算法（第 1 版，不要改动，否则旧的抽签结果无法验证）：
//! ChaCha20 的 32 字节种子 = SHA-256("toolforge-random-v1\n" + 操作名 + "\n" + 种子文本)。

use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use sha2::{Digest, Sha256};

pub const VERSION: &str = "toolforge-random-v1";

pub fn make(operation: &str, seed: Option<&str>) -> ChaCha20Rng {
    match seed.map(str::trim).filter(|s| !s.is_empty()) {
        Some(seed) => {
            let digest = Sha256::digest(format!("{VERSION}\n{operation}\n{seed}").as_bytes());
            let mut bytes = [0u8; 32];
            bytes.copy_from_slice(&digest);
            ChaCha20Rng::from_seed(bytes)
        }
        None => ChaCha20Rng::from_os_rng(),
    }
}

#[cfg(test)]
#[path = "rng_test.rs"]
mod tests;
