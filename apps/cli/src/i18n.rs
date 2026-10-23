//! 命令行的文案：按前端 i18next 的规则依次查找插件语言包、命令行语言包与应用 common 语言包。

use serde_json::{Map, Value};

const CLI_EN: &str = include_str!("../locales/en-US.json");
const CLI_ZH: &str = include_str!("../locales/zh-CN.json");
/// 应用语言包；只用其中的 common 命名空间（插件错误码等通用文案）
const APP_EN: &str = include_str!("../../desktop/src/locales/en-US.json");
const APP_ZH: &str = include_str!("../../desktop/src/locales/zh-CN.json");

/// 按 `--lang`、`TOOLFORGE_LANG`、`LC_ALL`/`LANG` 的顺序决定语言
pub fn detect_lang(flag: Option<&str>) -> &'static str {
    let pick = |value: &str| {
        let value = value.to_ascii_lowercase();
        (!value.is_empty()).then(|| {
            if value.starts_with("zh") {
                "zh-CN"
            } else {
                "en-US"
            }
        })
    };
    flag.and_then(pick)
        .or_else(|| {
            ["TOOLFORGE_LANG", "LC_ALL", "LANG"]
                .iter()
                .find_map(|name| std::env::var(name).ok().and_then(|v| pick(&v)))
        })
        .unwrap_or("en-US")
}

/// 文案查找链：插件语言包 → 命令行语言包 → 应用 common 语言包
pub struct Messages {
    lang: &'static str,
    plugin: Value,
    cli: Value,
    common: Value,
}

impl Messages {
    pub fn new(lang: &'static str) -> Self {
        let zh = lang == "zh-CN";
        let parse = |raw: &str| serde_json::from_str::<Value>(raw).unwrap_or_default();
        Self {
            lang,
            plugin: Value::Null,
            cli: parse(if zh { CLI_ZH } else { CLI_EN }),
            common: parse(if zh { APP_ZH } else { APP_EN })
                .get("common")
                .cloned()
                .unwrap_or_default(),
        }
    }

    pub fn lang(&self) -> &'static str {
        self.lang
    }

    /// 换上某个插件的语言包
    pub fn with_plugin(mut self, plugin: Value) -> Self {
        self.plugin = plugin;
        self
    }

    fn find<'a>(layers: &[&'a Value], key: &str) -> Option<&'a str> {
        layers
            .iter()
            .find_map(|layer| lookup(layer, key).and_then(Value::as_str))
    }

    /// 插件提供的文案（日志、阶段、接口说明）：插件 → 命令行 → common
    pub fn get(&self, key: &str, params: &Map<String, Value>) -> Option<String> {
        Self::find(&[&self.plugin, &self.cli, &self.common], key)
            .map(|template| interpolate(template, params))
    }

    /// 命令行自身的界面文案：只查命令行与 common，插件语言包里的同名键不会覆盖它；
    /// 没有译文时返回键本身
    pub fn text(&self, key: &str) -> String {
        self.text_with(key, &[])
    }

    pub fn text_with(&self, key: &str, params: &[(&str, Value)]) -> String {
        let params: Map<String, Value> = params
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect();
        Self::find(&[&self.cli, &self.common], key)
            .map_or_else(|| key.to_owned(), |template| interpolate(template, &params))
    }

    /// 小节标题，带本地化的冒号
    pub fn heading(&self, key: &str) -> String {
        self.text_with("heading", &[("title", self.text(key).into())])
    }

    /// manifest 里 `i18n:` 开头的值按插件语言包翻译，其余原样返回
    pub fn manifest_text(&self, value: &str) -> String {
        match value.strip_prefix("i18n:") {
            Some(key) => self.get(key, &Map::new()).unwrap_or_else(|| key.to_owned()),
            None => value.to_owned(),
        }
    }

    /// 错误码 → `errors.<code>`；没有译文时返回错误码
    pub fn error(&self, code: &str, params: &Map<String, Value>) -> String {
        self.get(&format!("errors.{code}"), params)
            .unwrap_or_else(|| code.to_owned())
    }
}

/// 与 i18next 的 deepFind 相同：键按 `.` 分段，某层找不到时尝试把剩余各段重新拼起来，
/// 所以 `errors.hash.no_files` 能找到 `{"errors": {"hash.no_files": "..."}}`
pub fn lookup<'a>(node: &'a Value, key: &str) -> Option<&'a Value> {
    if let Some(found) = node.get(key) {
        return Some(found);
    }
    let parts: Vec<&str> = key.split('.').collect();
    for split in (1..parts.len()).rev() {
        let head = parts[..split].join(".");
        if let Some(child) = node.get(&head)
            && let Some(found) = lookup(child, &parts[split..].join("."))
        {
            return Some(found);
        }
    }
    None
}

/// 代入 `{{name}}` 与 `{{name, format}}`；缺少的参数保持原样
pub fn interpolate(template: &str, params: &Map<String, Value>) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let Some(end) = rest[start..].find("}}") else {
            out.push_str(&rest[start..]);
            return out;
        };
        let inner = &rest[start + 2..start + end];
        let name = inner.split(',').next().unwrap_or_default().trim();
        match params.get(name) {
            Some(Value::String(s)) => out.push_str(s),
            Some(value) => out.push_str(&value.to_string()),
            None => out.push_str(&rest[start..start + end + 2]),
        }
        rest = &rest[start + end + 2..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
#[path = "i18n_test.rs"]
mod tests;
