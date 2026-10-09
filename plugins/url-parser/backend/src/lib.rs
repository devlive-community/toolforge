//! URL 解析插件后端：拆分 URL 各部分并解码，识别国际化域名与默认端口，
//! 编辑查询参数后重新生成 URL，并能去掉常见的跟踪参数。

mod build;
mod parse;
mod tracking;

use serde::Deserialize;
use serde_json::{Value, json};
use tf_plugin_api::{Manifest, PluginResult, ToolPlugin, parse_args, to_value, unknown_function};

const MANIFEST: &str = include_str!("../../manifest.json");

#[derive(Deserialize)]
struct InputArgs {
    input: String,
}

pub struct UrlParser {
    manifest: Manifest,
}

impl Default for UrlParser {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for UrlParser {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn detect(&self, text: &str) -> Option<tf_plugin_api::Detection> {
        let text = text.trim();
        if text.contains(char::is_whitespace)
            || !(text.starts_with("http://") || text.starts_with("https://"))
        {
            return None;
        }
        let url = url::Url::parse(text).ok()?;
        // 带查询参数的链接更可能需要拆开看
        let score = if url.query().is_some() { 70 } else { 40 };
        Some(
            tf_plugin_api::Detection::new(score, "url")
                .with("host", url.host_str().unwrap_or_default()),
        )
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "parse" => to_value(parse::parse(&parse_args::<InputArgs>(args)?.input)?),
            "build" => Ok(json!({ "href": build::build(&parse_args(args)?)? })),
            "strip_tracking" => {
                let args: InputArgs = parse_args(args)?;
                let (url, _) = parse::url(&args.input)?;
                let all = parse::params(&url);
                let removed = all.iter().filter(|p| p.tracking).count();
                let href = build::build(&build::BuildArgs {
                    input: args.input,
                    params: all
                        .into_iter()
                        .filter(|p| !p.tracking)
                        .map(|p| parse::Param {
                            key: p.key,
                            value: p.value,
                        })
                        .collect(),
                    space_as_plus: false,
                    fragment: None,
                })?;
                Ok(json!({ "href": href, "removed": removed }))
            }
            other => Err(unknown_function(other)),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
