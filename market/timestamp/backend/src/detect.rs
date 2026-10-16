//! 剪贴板识别：2000–2100 年范围内的 Unix 时间戳，或可解析的日期时间

use jiff::Timestamp;
use tf_plugin_api::Detection;

const MIN: i128 = 946_684_800;
const MAX: i128 = 4_102_444_800;

pub fn detect(text: &str) -> Option<Detection> {
    if (9..=19).contains(&text.len()) && text.bytes().all(|b| b.is_ascii_digit()) {
        let value: i128 = text.parse().ok()?;
        let (unit, divisor) = match text.len() {
            0..=11 => ("s", 1),
            12..=14 => ("ms", 1_000),
            15..=17 => ("us", 1_000_000),
            _ => ("ns", 1_000_000_000),
        };
        let seconds = value / divisor;
        if !(MIN..=MAX).contains(&seconds) {
            return None;
        }
        let utc = Timestamp::from_second(seconds as i64)
            .ok()?
            .strftime("%Y-%m-%d %H:%M:%S UTC")
            .to_string();
        return Some(
            Detection::new(85, "timestamp")
                .with("unit", unit)
                .with("utc", utc),
        );
    }
    let date_like = text.len() >= 10
        && text.len() <= 40
        && text.as_bytes()[..4].iter().all(u8::is_ascii_digit)
        && text.as_bytes()[4] == b'-';
    let parsed = text.parse::<Timestamp>().is_ok()
        || text.parse::<jiff::civil::DateTime>().is_ok()
        || text.parse::<jiff::civil::Date>().is_ok();
    (date_like && parsed).then(|| Detection::new(65, "date"))
}

#[cfg(test)]
#[path = "detect_test.rs"]
mod tests;
