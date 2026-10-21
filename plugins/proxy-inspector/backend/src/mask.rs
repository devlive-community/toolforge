//! 凭据脱敏：URL 中的 userinfo 密码遮罩；Rust 解析，界面只拿脱敏后的值。

/// 遮罩文本（定长，避免泄露长度）
const MASK: &str = "••••";

/// 遮罩 URL 中的密码部分：scheme://user:pass@host → scheme://user:••••@host。
/// 没有 scheme 或没有 userinfo 的输入原样返回。
pub fn mask_url(input: &str) -> String {
    let Some((scheme, rest)) = input.split_once("://") else {
        return input.to_owned();
    };
    // userinfo 只可能在最后一个 '/' 之前（host 段内），路径里出现的 '@' 不属于 userinfo
    let authority_end = rest.find('/').unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    let tail = &rest[authority_end..];
    match authority.rsplit_once('@') {
        Some((userinfo, host)) => match userinfo.split_once(':') {
            Some((user, _)) => format!("{scheme}://{user}:{MASK}@{host}{tail}"),
            None => input.to_owned(),
        },
        None => input.to_owned(),
    }
}

/// 遮罩 host:port 形式之外的一般地址（host 里不会出现 '@'；出现即按 userinfo 处理）
#[cfg(any(target_os = "macos", target_os = "linux", test))]
pub fn mask_host(input: &str) -> String {
    match input.rsplit_once('@') {
        Some((userinfo, host)) => match userinfo.split_once(':') {
            Some((user, _)) => format!("{user}:{MASK}@{host}"),
            // 整段都是用户名（无冒号）视为异常输入，整体遮罩用户段
            None => format!("{MASK}@{host}"),
        },
        None => input.to_owned(),
    }
}

/// 原始 JSON 递归遮罩：字符串值里含 "://" 或 '@' 的按 URL 处理；
/// 超长文本截断，防止报告膨胀。
pub fn mask_raw(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::String(text) => {
            let masked = if text.contains("://") || text.contains('@') {
                mask_url(text)
            } else {
                text.clone()
            };
            serde_json::Value::String(truncate_text(masked))
        }
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.iter().take(64).map(mask_raw).collect())
        }
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.iter()
                .map(|(key, value)| (key.clone(), mask_raw(value)))
                .collect(),
        ),
        other => other.clone(),
    }
}

fn truncate_text(text: String) -> String {
    if text.len() > 512 {
        let mut cut = 512;
        while !text.is_char_boundary(cut) {
            cut -= 1;
        }
        format!("{}…", &text[..cut])
    } else {
        text
    }
}

#[cfg(test)]
#[path = "mask_test.rs"]
mod tests;
