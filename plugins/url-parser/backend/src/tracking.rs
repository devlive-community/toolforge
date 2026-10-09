//! 常见的广告与分享跟踪参数。

const EXACT: &[&str] = &[
    "fbclid",
    "gclid",
    "gclsrc",
    "dclid",
    "gbraid",
    "wbraid",
    "msclkid",
    "yclid",
    "twclid",
    "ttclid",
    "li_fat_id",
    "mc_cid",
    "mc_eid",
    "igshid",
    "_ga",
    "_gl",
    "_hsenc",
    "_hsmi",
    "mkt_tok",
    "oly_anon_id",
    "oly_enc_id",
    "vero_id",
    "wickedid",
    "rb_clickid",
    // 国内常见：淘宝 / 天猫、哔哩哔哩、微信
    "spm",
    "scm",
    "spm_id_from",
    "vd_source",
    "share_source",
    "share_medium",
    "share_plat",
    "share_session_id",
    "share_tag",
    "unique_k",
    "from_spmid",
    "wxshare_count",
];

/// utm_ 开头或在已知列表中（不区分大小写）
pub fn is_tracking(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    key.starts_with("utm_") || EXACT.contains(&key.as_str())
}

#[cfg(test)]
#[path = "tracking_test.rs"]
mod tests;
