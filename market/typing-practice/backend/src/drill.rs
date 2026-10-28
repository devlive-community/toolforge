//! 易错字符重练：由用户的易错字符生成一段有界、可复现的练习文本。
//!
//! - 只使用给定字符（去重、忽略空白），单词之间用空格分隔，长度钳制到 `MAX_LEN`。
//! - 固定种子的线性同余伪随机，同一输入与种子得到同一结果，便于复现。
//! - 不读取任何外部数据，纯计算。

/// 输出文本的最大字符数
pub const MAX_LEN: usize = 2000;
/// 参与生成的不同字符上限（超出部分忽略，防止无界输入）
const MAX_CHARS: usize = 64;
/// 单词长度（字符数）
const WORD_LEN: usize = 4;

/// 生成重练文本。`chars` 为易错字符，`len` 为期望字符数，`seed` 决定随机序列。
pub fn build(chars: &str, len: usize, seed: u64) -> String {
    let mut pool: Vec<char> = Vec::new();
    for c in chars.chars().filter(|c| !c.is_whitespace()) {
        if !pool.contains(&c) {
            pool.push(c);
            if pool.len() == MAX_CHARS {
                break;
            }
        }
    }
    if pool.is_empty() {
        return String::new();
    }
    let target = len.min(MAX_LEN);
    let mut state = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    let mut next = move || {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (state >> 33) as usize
    };
    // 精确生成 target 个字符：每个单词 WORD_LEN 个字符，单词之间用一个空格分隔；
    // 若恰好在单词边界处结束，则不写结尾空格，保证长度精确且不以空格结束
    let mut out = String::with_capacity(target);
    let mut written = 0usize;
    let mut in_word = 0usize;
    while written < target {
        if in_word == WORD_LEN && written + 1 < target {
            out.push(' ');
            written += 1;
            in_word = 0;
        } else {
            out.push(pool[next() % pool.len()]);
            written += 1;
            in_word += 1;
        }
    }
    out
}

#[cfg(test)]
#[path = "drill_test.rs"]
mod tests;
