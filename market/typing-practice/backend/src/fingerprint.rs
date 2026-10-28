//! 内容指纹：为练习文本生成稳定的短标识，用于把个人纪录绑定到具体文本版本。
//! 文本一旦改变，指纹随之改变，旧纪录不会与新文本比较。
//! 使用 FNV-1a（64 位），纯计算、与平台无关、结果固定。

const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

/// 返回 8 位小写十六进制指纹。
pub fn fingerprint(text: &str) -> String {
    let mut hash = FNV_OFFSET;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("{:08x}", (hash & 0xffff_ffff) as u32)
}

#[cfg(test)]
#[path = "fingerprint_test.rs"]
mod tests;
