//! 字符检查插件后端：逐字符分析文本，找出不可见字符、双向控制符、形近字与特殊空格，
//! 清理文本，并按码位或名称查找字符。

mod analyze;
mod lookup;

use serde::Deserialize;
use serde_json::Value;
use tf_plugin_api::{
    Detection, Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value,
    unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");
/// 分析的文本上限（字节）
const MAX_TEXT: usize = 4 * 1024 * 1024;

#[derive(Deserialize)]
struct TextArgs {
    text: String,
}

#[derive(Deserialize)]
struct CleanArgs {
    text: String,
    #[serde(flatten)]
    options: analyze::CleanOptions,
}

#[derive(Deserialize)]
struct LookupArgs {
    query: String,
}

fn check_size(text: &str) -> PluginResult<()> {
    if text.len() > MAX_TEXT {
        return Err(PluginError::new("unicode.too_large").with("limit", "4 MB"));
    }
    Ok(())
}

pub struct UnicodeInspector {
    manifest: Manifest,
}

impl Default for UnicodeInspector {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for UnicodeInspector {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// 剪贴板里有看不见的字符或双向控制符时推荐本工具
    fn detect(&self, text: &str) -> Option<Detection> {
        let (mut hidden, mut bidi) = (0, 0);
        for c in text.chars() {
            if analyze::is_bidi(c) {
                bidi += 1;
            } else if analyze::is_invisible(c) && !matches!(c, '\u{200d}' | '\u{fe0f}' | '\u{fe0e}')
            {
                hidden += 1;
            }
        }
        if bidi > 0 {
            Some(Detection::new(80, "bidi").with("count", bidi))
        } else if hidden > 0 {
            Some(Detection::new(60, "hidden").with("count", hidden))
        } else {
            None
        }
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "analyze" => {
                let args: TextArgs = parse_args(args)?;
                check_size(&args.text)?;
                to_value(analyze::analyze(&args.text))
            }
            "clean" => {
                let args: CleanArgs = parse_args(args)?;
                check_size(&args.text)?;
                to_value(analyze::clean(&args.text, &args.options))
            }
            "lookup" => {
                let args: LookupArgs = parse_args(args)?;
                to_value(lookup::lookup(&args.query))
            }
            other => Err(unknown_function(other)),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
