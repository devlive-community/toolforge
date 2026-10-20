//! 环境变量来源：进程启动继承的 HTTP_PROXY、HTTPS_PROXY、ALL_PROXY、NO_PROXY
//! 及小写形式；大小写并存时各值保留，不发明跨客户端统一优先级。

use std::env;

use super::model::EnvVar;

/// 大写与小写形式都收集；同名（大小写不同）各保留
pub const PROXY_VARS: [(&str, bool); 8] = [
    ("HTTP_PROXY", false),
    ("HTTPS_PROXY", false),
    ("ALL_PROXY", false),
    ("NO_PROXY", true),
    ("http_proxy", false),
    ("https_proxy", false),
    ("all_proxy", false),
    ("no_proxy", true),
];

/// 读取并脱敏；空值跳过
pub fn collect() -> Vec<EnvVar> {
    PROXY_VARS
        .iter()
        .filter_map(|(name, is_exclusions)| {
            let value = env::var(name).ok()?;
            let value = value.trim().to_owned();
            (!value.is_empty()).then(|| EnvVar {
                name: (*name).to_owned(),
                value: if *is_exclusions {
                    value
                } else {
                    super::mask::mask_url(&value)
                },
                is_exclusions: *is_exclusions,
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "env_test.rs"]
mod tests;
