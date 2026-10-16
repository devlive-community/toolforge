//! 生成本地格式测试卡与校验卡号。生成的数据只满足号段、长度与 Luhn 规则，用于表单与格式测试，
//! 不是任何支付平台的可用测试卡。

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tf_plugin_api::{PluginError, PluginResult};

use crate::brand::{self, Brand};
use crate::sandbox;

pub const MAX_COUNT: usize = 1000;
pub const MAX_VALIDATE: usize = 10_000;
/// 可复现算法（第 1 版）：ChaCha20 种子 = SHA-256("toolforge-testcard-v1\n" + 种子文本)
pub const ALGORITHM: &str = "toolforge-testcard-v1";

const FIRST: &[&str] = &[
    "ALEX", "JORDAN", "TAYLOR", "MORGAN", "CASEY", "RILEY", "JAMIE", "AVERY", "QUINN", "SKYLER",
    "DREW", "ROBIN", "KAI", "LEE", "SAM", "NOAH", "EMMA", "LIAM", "OLIVIA", "MIA",
];
const LAST: &[&str] = &[
    "TESTER",
    "SAMPLE",
    "EXAMPLE",
    "DEMO",
    "MOCK",
    "PLACEHOLDER",
    "SANDBOX",
    "FIXTURE",
    "PROTO",
    "DUMMY",
];

fn rng(seed: Option<&str>) -> ChaCha20Rng {
    match seed.map(str::trim).filter(|s| !s.is_empty()) {
        Some(seed) => {
            let digest = Sha256::digest(format!("{ALGORITHM}\n{seed}").as_bytes());
            let mut bytes = [0u8; 32];
            bytes.copy_from_slice(&digest);
            ChaCha20Rng::from_seed(bytes)
        }
        None => ChaCha20Rng::from_os_rng(),
    }
}

/// 当前年月（UTC），用于生成未来的有效期
fn current_month() -> (i32, u32) {
    let days = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() / 86_400)
        .unwrap_or(0) as i64;
    // Howard Hinnant 的 civil_from_days
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = (yoe + era * 400 + i64::from(month <= 2)) as i32;
    (year, month)
}

#[derive(Debug, Deserialize)]
pub struct GenerateArgs {
    pub brand: Brand,
    #[serde(default = "one")]
    pub count: usize,
    pub seed: Option<String>,
}

fn one() -> usize {
    1
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Card {
    pub number: String,
    pub brand: Brand,
    pub holder: String,
    /// MM/YY
    pub expiry: String,
    pub cvc: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Generated {
    pub cards: Vec<Card>,
    pub seeded: bool,
    pub algorithm: &'static str,
    pub csv: String,
    pub json: String,
}

fn card(brand: Brand, rng: &mut ChaCha20Rng, now: (i32, u32)) -> Card {
    let prefixes = brand.prefixes();
    let prefix = &prefixes[rng.random_range(0..prefixes.len())];
    let length = brand.default_length();
    let mut body = prefix.clone();
    while body.len() < length - 1 {
        body.push(char::from(b'0' + rng.random_range(0..10u8)));
    }
    let number = format!("{body}{}", brand::check_digit(&body));
    // 有效期：当前月份之后 1–60 个月
    let ahead = rng.random_range(1..=60u32);
    let total = now.0 * 12 + now.1 as i32 - 1 + ahead as i32;
    let expiry = format!("{:02}/{:02}", total % 12 + 1, (total / 12) % 100);
    let cvc: String = (0..brand.cvc_length())
        .map(|_| char::from(b'0' + rng.random_range(0..10u8)))
        .collect();
    let holder = format!(
        "{} {}",
        FIRST[rng.random_range(0..FIRST.len())],
        LAST[rng.random_range(0..LAST.len())]
    );
    Card {
        number,
        brand,
        holder,
        expiry,
        cvc,
    }
}

fn csv_line(fields: &[&str]) -> String {
    fields.join(",") + "\n"
}

pub fn generate(args: GenerateArgs) -> PluginResult<Generated> {
    if !(1..=MAX_COUNT).contains(&args.count) {
        return Err(PluginError::new("card.invalid_count").with("max", MAX_COUNT));
    }
    let mut rng = rng(args.seed.as_deref());
    let now = current_month();
    let cards: Vec<Card> = (0..args.count)
        .map(|_| card(args.brand, &mut rng, now))
        .collect();
    let mut csv = csv_line(&[
        "purpose", "mode", "brand", "number", "holder", "expiry", "cvc",
    ]);
    for c in &cards {
        let brand = serde_json::to_value(c.brand)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default();
        csv.push_str(&csv_line(&[
            "test-only",
            "local-format",
            &brand,
            &c.number,
            &c.holder,
            &c.expiry,
            &c.cvc,
        ]));
    }
    let json = serde_json::to_string_pretty(&serde_json::json!({
        "purpose": "test-only",
        "mode": "local-format",
        "note": "Format test data only: not issued cards and not sandbox cards of any payment platform",
        "cards": cards,
    }))
    .map_err(|e| PluginError::new("card.export_failed").with("detail", e.to_string()))?;
    Ok(Generated {
        seeded: args.seed.as_deref().is_some_and(|s| !s.trim().is_empty()),
        algorithm: ALGORITHM,
        cards,
        csv,
        json,
    })
}

#[derive(Debug, Deserialize)]
pub struct ValidateArgs {
    pub input: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Problem {
    /// 含有数字、空格、连字符以外的字符
    Characters,
    /// 无法按号段识别品牌
    UnknownBrand,
    /// 长度与品牌规则不符
    Length,
    /// Luhn 校验失败
    Luhn,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Checked {
    pub line: usize,
    pub input: String,
    pub digits: String,
    pub brand: Option<Brand>,
    pub length: usize,
    pub allowed_lengths: Vec<usize>,
    pub luhn: bool,
    pub problems: Vec<Problem>,
    /// 是官方沙箱用例时给出平台与场景
    pub sandbox: Option<sandbox::Case>,
}

pub fn validate(args: ValidateArgs) -> PluginResult<Vec<Checked>> {
    let lines: Vec<(usize, &str)> = args
        .input
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim()))
        .filter(|(_, l)| !l.is_empty())
        .collect();
    if lines.len() > MAX_VALIDATE {
        return Err(PluginError::new("card.too_many").with("max", MAX_VALIDATE));
    }
    Ok(lines
        .into_iter()
        .map(|(line, text)| {
            let mut problems = Vec::new();
            if !text
                .chars()
                .all(|c| c.is_ascii_digit() || c == ' ' || c == '-')
            {
                problems.push(Problem::Characters);
            }
            let digits: String = text.chars().filter(char::is_ascii_digit).collect();
            let brand = brand::detect(&digits);
            let allowed_lengths = brand.map(|b| b.lengths().to_vec()).unwrap_or_default();
            match brand {
                None => problems.push(Problem::UnknownBrand),
                Some(b) if !b.lengths().contains(&digits.len()) => problems.push(Problem::Length),
                _ => {}
            }
            let luhn = brand::luhn_valid(&digits);
            if !luhn {
                problems.push(Problem::Luhn);
            }
            Checked {
                line,
                input: text.to_owned(),
                length: digits.len(),
                sandbox: sandbox::CASES.iter().find(|c| c.number == digits).cloned(),
                digits,
                brand,
                allowed_lengths,
                luhn,
                problems,
            }
        })
        .collect())
}

#[cfg(test)]
#[path = "card_test.rs"]
mod tests;
