//! 函数接口声明（manifest `functions.<fn>.interface`）的严格校验。
//!
//! 宿主侧解析是宽松的（声明有误只是不暴露该函数），所以错误必须在开发期拦下。
//! 参数 Schema 只允许一层扁平属性，便于 CLI 映射为参数；更复杂的输入用 `--args` JSON 传入。

use std::collections::HashSet;

use serde_json::{Map, Value};

use crate::rules::Violation;

const RULE: &str = "plugin-interface";
const INTERFACE_KEYS: &[&str] = &[
    "expose", "summary", "effect", "uses", "confirm", "params", "output", "examples",
];
const PARAM_KEYS: &[&str] = &[
    "type",
    "description",
    "enum",
    "default",
    "minimum",
    "maximum",
    "items",
    "format",
    "x-access",
    "x-positional",
    "x-stdin",
    "x-sensitive",
];
const PARAM_TYPES: &[&str] = &["string", "integer", "number", "boolean", "array"];
const ITEM_KEYS: &[&str] = &["type", "enum", "format", "x-access"];
const ITEM_TYPES: &[&str] = &["string", "integer", "number"];
pub const FS_PERMISSION: &str = "fs:user-selected";

/// 被检查的插件：manifest 与各语言包
pub struct Plugin<'a> {
    pub relative: &'a str,
    pub manifest: &'a Value,
    pub locales: &'a [(String, Value)],
}

struct Checker<'a> {
    plugin: &'a Plugin<'a>,
    violations: Vec<Violation>,
}

impl Checker<'_> {
    fn report(&mut self, function: &str, message: impl Into<String>) {
        self.violations.push(Violation {
            rule: RULE,
            message: format!("functions.{function}.interface: {}", message.into()),
            location: format!("{}:1", self.plugin.relative),
        });
    }

    /// i18n 键必须在每个语言包中都存在
    fn i18n(&mut self, function: &str, field: &str, value: Option<&Value>) {
        let Some(key) = value
            .and_then(Value::as_str)
            .and_then(|k| k.strip_prefix("i18n:"))
        else {
            self.report(function, format!("{field} must be an `i18n:` key"));
            return;
        };
        for (locale, messages) in self.plugin.locales {
            let found = key
                .split('.')
                .try_fold(messages, |node, part| node.get(part))
                .is_some_and(Value::is_string);
            if !found {
                self.report(
                    function,
                    format!("{field} key `{key}` is missing in {locale}"),
                );
            }
        }
    }

    fn interface(&mut self, function: &str, interface: &Map<String, Value>) {
        for key in interface.keys() {
            if !INTERFACE_KEYS.contains(&key.as_str()) {
                self.report(function, format!("unknown field `{key}`"));
            }
        }

        match interface.get("expose").and_then(Value::as_array) {
            Some(channels) if !channels.is_empty() => {
                let mut seen = HashSet::new();
                for channel in channels {
                    let name = channel.as_str().unwrap_or_default();
                    if !matches!(name, "cli" | "agent") || !seen.insert(name) {
                        self.report(function, format!("expose has invalid entry {channel}"));
                    }
                }
            }
            _ => self.report(function, "expose must list `cli` and/or `agent`"),
        }

        self.i18n(function, "summary", interface.get("summary"));

        let effect = interface.get("effect").and_then(Value::as_str);
        if !matches!(effect, Some("read" | "write" | "external")) {
            self.report(function, "effect must be read, write or external");
        }

        let permissions: Vec<&str> = self
            .plugin
            .manifest
            .get("permissions")
            .and_then(Value::as_array)
            .map(|p| p.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        let mut uses = Vec::new();
        match interface.get("uses") {
            None => {}
            Some(Value::Array(items)) => {
                for item in items {
                    match item.as_str() {
                        Some(p) if permissions.contains(&p) => uses.push(p),
                        _ => self.report(
                            function,
                            format!("uses entry {item} is not one of the plugin permissions"),
                        ),
                    }
                }
            }
            Some(_) => self.report(function, "uses must be an array of permissions"),
        }

        if interface.get("confirm").is_some_and(|v| !v.is_boolean()) {
            self.report(function, "confirm must be a boolean");
        }

        let access = match interface.get("params").and_then(Value::as_object) {
            Some(params) => self.params(function, params),
            None => {
                self.report(function, "params must be a JSON Schema object");
                Vec::new()
            }
        };
        if !access.is_empty() && !uses.contains(&FS_PERMISSION) {
            self.report(
                function,
                format!("path parameters require uses `{FS_PERMISSION}`"),
            );
        }
        if access.iter().any(|a| a == "write") && effect != Some("write") {
            self.report(function, "x-access write requires effect write");
        }

        if let Some(output) = interface.get("output") {
            self.output(function, output);
        }

        let command = format!("tfp-{}", self.slug());
        match interface.get("examples") {
            None => {}
            Some(Value::Array(examples)) => {
                for example in examples {
                    let ok = example
                        .as_str()
                        .is_some_and(|e| e.split_whitespace().any(|word| word == command));
                    if !ok {
                        self.report(function, format!("example {example} must call `{command}`"));
                    }
                }
            }
            Some(_) => self.report(function, "examples must be an array of strings"),
        }
    }

    fn slug(&self) -> String {
        self.plugin
            .manifest
            .get("id")
            .and_then(Value::as_str)
            .and_then(|id| id.rsplit('.').next())
            .unwrap_or_default()
            .to_owned()
    }

    /// 校验参数 Schema，返回路径参数的 x-access 取值
    fn params(&mut self, function: &str, params: &Map<String, Value>) -> Vec<String> {
        let mut access = Vec::new();
        for key in params.keys() {
            if !matches!(key.as_str(), "type" | "properties" | "required") {
                self.report(function, format!("params has unsupported keyword `{key}`"));
            }
        }
        if params.get("type").and_then(Value::as_str) != Some("object") {
            self.report(function, "params.type must be object");
        }
        let empty = Map::new();
        let properties = match params.get("properties") {
            Some(Value::Object(properties)) => properties,
            None => &empty,
            Some(_) => {
                self.report(function, "params.properties must be an object");
                &empty
            }
        };
        if let Some(required) = params.get("required") {
            let ok = required.as_array().is_some_and(|items| {
                items
                    .iter()
                    .all(|r| r.as_str().is_some_and(|r| properties.contains_key(r)))
            });
            if !ok {
                self.report(function, "params.required must name declared properties");
            }
        }

        let mut positions = HashSet::new();
        let mut stdin = 0;
        for (name, schema) in properties {
            let Some(schema) = schema.as_object() else {
                self.report(function, format!("param `{name}` must be an object"));
                continue;
            };
            let at = format!("param `{name}`");
            for key in schema.keys() {
                if !PARAM_KEYS.contains(&key.as_str()) {
                    self.report(function, format!("{at} has unsupported keyword `{key}`"));
                }
            }
            let kind = schema
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if !PARAM_TYPES.contains(&kind) {
                self.report(
                    function,
                    format!("{at} type must be one of {PARAM_TYPES:?}"),
                );
            }
            self.i18n(
                function,
                &format!("{at} description"),
                schema.get("description"),
            );

            let mut path_schema = schema;
            if kind == "array" {
                match schema.get("items").and_then(Value::as_object) {
                    Some(items) => {
                        for key in items.keys() {
                            if !ITEM_KEYS.contains(&key.as_str()) {
                                self.report(
                                    function,
                                    format!("{at} items has unsupported keyword `{key}`"),
                                );
                            }
                        }
                        let item = items
                            .get("type")
                            .and_then(Value::as_str)
                            .unwrap_or_default();
                        if !ITEM_TYPES.contains(&item) {
                            self.report(function, format!("{at} items type must be scalar"));
                        }
                        path_schema = items;
                    }
                    None => self.report(function, format!("{at} needs items")),
                }
            } else if schema.contains_key("items") {
                self.report(function, format!("{at} items is only for arrays"));
            }

            match path_schema.get("format").map(|f| f.as_str()) {
                None => {
                    if path_schema.contains_key("x-access") {
                        self.report(function, format!("{at} x-access needs format path"));
                    }
                }
                Some(Some("path")) => match path_schema.get("x-access").and_then(Value::as_str) {
                    Some(mode @ ("read" | "write")) => access.push(mode.to_owned()),
                    _ => self.report(function, format!("{at} path needs x-access read or write")),
                },
                Some(_) => self.report(function, format!("{at} format must be path")),
            }

            if let Some(position) = schema.get("x-positional") {
                match position.as_u64() {
                    Some(p) if positions.insert(p) => {}
                    _ => self.report(
                        function,
                        format!("{at} x-positional must be a unique non-negative integer"),
                    ),
                }
            }
            if let Some(flag) = schema.get("x-stdin") {
                if flag != &Value::Bool(true) || kind != "string" {
                    self.report(function, format!("{at} x-stdin is only `true` on strings"));
                }
                stdin += 1;
            }
            if schema.get("x-sensitive").is_some_and(|v| !v.is_boolean()) {
                self.report(function, format!("{at} x-sensitive must be a boolean"));
            }
            if let Some(values) = schema.get("enum")
                && !values.as_array().is_some_and(|v| !v.is_empty())
            {
                self.report(function, format!("{at} enum must be a non-empty array"));
            }
        }
        if stdin > 1 {
            self.report(function, "at most one param may read stdin");
        }
        if positions.len() > 1 {
            let mut sorted: Vec<u64> = positions.into_iter().collect();
            sorted.sort_unstable();
            if sorted.iter().enumerate().any(|(i, p)| *p != i as u64) {
                self.report(
                    function,
                    "x-positional indexes must start at 0 without gaps",
                );
            }
        }
        access
    }

    fn output(&mut self, function: &str, output: &Value) {
        let pointer = |v: Option<&Value>| {
            v.and_then(Value::as_str)
                .is_some_and(|p| p.is_empty() || p.starts_with('/'))
        };
        let Some(output) = output.as_object() else {
            self.report(function, "output must be an object");
            return;
        };
        match (output.get("text"), output.get("table")) {
            (Some(text), None) if pointer(Some(text)) && output.len() == 1 => {}
            (None, Some(table)) if pointer(Some(table)) => {
                for key in output.keys() {
                    if !matches!(key.as_str(), "table" | "columns") {
                        self.report(function, format!("output has unknown field `{key}`"));
                    }
                }
                match output.get("columns").and_then(Value::as_array) {
                    Some(columns) if !columns.is_empty() => {
                        for column in columns {
                            if column.get("key").and_then(Value::as_str).is_none() {
                                self.report(function, "output column needs key");
                            }
                            self.i18n(function, "output column label", column.get("label"));
                        }
                    }
                    _ => self.report(function, "output.table needs columns"),
                }
            }
            _ => self.report(
                function,
                "output must be {\"text\": pointer} or {\"table\": pointer, \"columns\": [...]}",
            ),
        }
    }
}

/// require_decided：新增或递增过版本的插件，每个函数都必须写明 interface（声明或 false）
pub fn check(plugin: &Plugin<'_>, require_decided: bool) -> Vec<Violation> {
    let mut checker = Checker {
        plugin,
        violations: Vec::new(),
    };
    let Some(functions) = plugin.manifest.get("functions").and_then(Value::as_object) else {
        return checker.violations;
    };
    for (name, spec) in functions {
        match spec.get("interface") {
            None if require_decided => checker.report(
                name,
                "new or updated plugins must declare an interface or set it to false",
            ),
            None | Some(Value::Bool(false)) => {}
            Some(Value::Object(interface)) => checker.interface(name, interface),
            Some(_) => checker.report(name, "must be an object or false"),
        }
    }
    checker.violations
}

#[cfg(test)]
#[path = "interfaces_test.rs"]
mod tests;
