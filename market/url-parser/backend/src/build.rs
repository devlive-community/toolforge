//! 用编辑后的参数重新生成 URL。

use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use serde::Deserialize;
use tf_plugin_api::PluginResult;

use crate::parse::{self, Param};

/// 查询参数中保留不编码的字符（RFC 3986 unreserved）
const QUERY: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildArgs {
    pub input: String,
    pub params: Vec<Param>,
    /// 空格写成 +（表单格式），否则写成 %20
    #[serde(default)]
    pub space_as_plus: bool,
    /// 为空时保留原片段；空字符串表示去掉片段
    #[serde(default)]
    pub fragment: Option<String>,
}

fn encode(text: &str, plus: bool) -> String {
    let encoded = utf8_percent_encode(text, QUERY).to_string();
    if plus {
        encoded.replace("%20", "+")
    } else {
        encoded
    }
}

pub fn build(args: &BuildArgs) -> PluginResult<String> {
    let (mut url, _) = parse::url(&args.input)?;
    let query: Vec<String> = args
        .params
        .iter()
        .filter(|p| !p.key.is_empty() || !p.value.is_empty())
        .map(|p| {
            let key = encode(&p.key, args.space_as_plus);
            if p.value.is_empty() {
                key
            } else {
                format!("{key}={}", encode(&p.value, args.space_as_plus))
            }
        })
        .collect();
    url.set_query((!query.is_empty()).then(|| query.join("&")).as_deref());
    if let Some(fragment) = &args.fragment {
        url.set_fragment((!fragment.is_empty()).then_some(fragment.as_str()));
    }
    Ok(url.to_string())
}

#[cfg(test)]
#[path = "build_test.rs"]
mod tests;
