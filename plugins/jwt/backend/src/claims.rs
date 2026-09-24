use serde::Serialize;
use serde_json::Value;

/// 时间类注册声明
const TIME_CLAIMS: &[&str] = &["exp", "nbf", "iat"];

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TimeClaim {
    pub name: String,
    pub value: i64,
    /// UTC（RFC 3339）
    pub iso: Option<String>,
    /// 相对当前时间的秒数（正数表示未来）
    pub relative_seconds: i64,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub times: Vec<TimeClaim>,
    /// exp 已过期；没有 exp 时为 None
    pub expired: Option<bool>,
    /// nbf 尚未生效；没有 nbf 时为 None
    pub not_yet_valid: Option<bool>,
}

pub fn status(payload: &Value, now: i64) -> Status {
    let times: Vec<TimeClaim> = TIME_CLAIMS
        .iter()
        .filter_map(|name| {
            let value = payload.get(*name)?.as_f64()? as i64;
            Some(TimeClaim {
                name: (*name).to_owned(),
                value,
                iso: jiff::Timestamp::from_second(value)
                    .ok()
                    .map(|t| t.to_string()),
                relative_seconds: value - now,
            })
        })
        .collect();
    let find = |name: &str| times.iter().find(|t| t.name == name).map(|t| t.value);
    Status {
        expired: find("exp").map(|exp| exp <= now),
        not_yet_valid: find("nbf").map(|nbf| nbf > now),
        times,
    }
}

#[cfg(test)]
#[path = "claims_test.rs"]
mod tests;
