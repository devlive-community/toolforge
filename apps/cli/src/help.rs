//! 帮助文本：全部由 manifest 的接口声明与语言包生成，不需要另写帮助。

use serde_json::Value;
use tf_plugin_api::Manifest;
use tf_plugin_api::interface::Interface;

use crate::args::{kebab, positionals};
use crate::catalog::{cli_functions, slug};
use crate::i18n::Messages;
use crate::render::{pad, width};

/// 用法行里的占位词：插件、函数、选项
fn placeholders(messages: &Messages) -> (String, String, String) {
    (
        messages.text("placeholder.plugin"),
        messages.text("placeholder.function"),
        messages.text("placeholder.options"),
    )
}

/// 两列对齐的条目
fn rows(items: &[(String, String)]) -> Vec<String> {
    let left = items.iter().map(|(l, _)| width(l)).max().unwrap_or(0);
    items
        .iter()
        .map(|(l, r)| {
            if r.is_empty() {
                format!("  {l}")
            } else {
                format!("  {}  {r}", pad(l, left))
            }
        })
        .collect()
}

fn section(out: &mut Vec<String>, title: String, items: &[(String, String)]) {
    if items.is_empty() {
        return;
    }
    out.push(String::new());
    out.push(title);
    out.extend(rows(items));
}

fn global_options(messages: &Messages) -> Vec<(String, String)> {
    [
        ("--json", "global.json"),
        ("--args <JSON|@file>", "global.args"),
        ("--lang <zh-CN|en-US>", "global.lang"),
        ("-v, --verbose", "global.verbose"),
        ("-h, --help", "global.help"),
    ]
    .iter()
    .map(|(flag, key)| ((*flag).to_owned(), messages.text(key)))
    .collect()
}

fn display(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Array(items) => items.iter().map(display).collect::<Vec<_>>().join(","),
        other => other.to_string(),
    }
}

/// 参数的说明：描述 + 必填 / 标准输入 / 可选值 / 默认值
fn describe(schema: &Value, required: bool, messages: &Messages) -> String {
    let mut notes = Vec::new();
    if required {
        notes.push(messages.text("required"));
    }
    if schema["x-stdin"] == true {
        notes.push(messages.text("fromStdin"));
    }
    let item = if schema["type"] == "array" {
        notes.push(messages.text("list"));
        &schema["items"]
    } else {
        schema
    };
    if let Some(values) = item["enum"].as_array() {
        let values = values.iter().map(display).collect::<Vec<_>>().join(", ");
        notes.push(messages.text_with("choices", &[("values", values.into())]));
    }
    if let Some(default) = schema.get("default") {
        notes.push(messages.text_with("default", &[("value", display(default).into())]));
    }
    let description = schema["description"]
        .as_str()
        .map(|d| messages.manifest_text(d))
        .unwrap_or_default();
    let notes = notes.join(&messages.text("noteSeparator"));
    match (description.is_empty(), notes.is_empty()) {
        (_, true) => description,
        (true, false) => messages.text_with("notesOnly", &[("notes", notes.into())]),
        (false, false) => messages.text_with(
            "withNotes",
            &[("text", description.into()), ("notes", notes.into())],
        ),
    }
}

/// `tfp` 的总帮助：子命令与可用插件
pub fn global(plugins: &[(Manifest, Value)], messages: &Messages) -> String {
    let (plugin, function, options) = placeholders(messages);
    let mut out = vec![
        format!("tfp — ToolForge {}", env!("CARGO_PKG_VERSION")),
        String::new(),
        messages.heading("usage"),
        format!("  tfp <{plugin}> <{function}> [{options}]"),
        format!("  tfp-<{plugin}> <{function}> [{options}]"),
    ];
    let commands: Vec<(String, String)> = [
        ("list", "command.list"),
        ("help [plugin]", "command.help"),
        ("version", "command.version"),
    ]
    .iter()
    .map(|(name, key)| ((*name).to_owned(), messages.text(key)))
    .chain(std::iter::once((
        format!("<{plugin}> <{function}>"),
        messages.text("command.run"),
    )))
    .collect();
    section(&mut out, messages.heading("commands"), &commands);
    out.push(String::new());
    out.push(list(plugins, messages));
    section(
        &mut out,
        messages.heading("globalOptions"),
        &global_options(messages),
    );
    out.join("\n")
}

/// `tfp list`：插件名与显示名
pub fn list(plugins: &[(Manifest, Value)], messages: &Messages) -> String {
    if plugins.is_empty() {
        return messages.text("noPlugins");
    }
    let items: Vec<(String, String)> = plugins
        .iter()
        .map(|(m, locale)| {
            let own = Messages::new(messages.lang()).with_plugin(locale.clone());
            (slug(&m.id).to_owned(), own.manifest_text(&m.name))
        })
        .collect();
    let mut out = vec![messages.heading("plugins")];
    out.extend(rows(&items));
    out.push(String::new());
    out.push(messages.text("pluginHelp"));
    out.join("\n")
}

/// 某个插件的帮助：函数列表
pub fn plugin(command: &str, manifest: &Manifest, messages: &Messages) -> String {
    let name = messages.manifest_text(&manifest.name);
    let description = messages.manifest_text(&manifest.description);
    let functions = cli_functions(manifest);
    let mut out = vec![format!("{command} — {name}")];
    if !description.is_empty() {
        out.push(description);
    }
    out.push(String::new());
    out.push(messages.heading("usage"));
    let (_, function, options) = placeholders(messages);
    out.push(format!("  {command} <{function}> [{options}]"));
    let items: Vec<(String, String)> = functions
        .iter()
        .map(|(name, _, interface)| ((*name).clone(), messages.manifest_text(&interface.summary)))
        .collect();
    section(&mut out, messages.heading("functions"), &items);
    out.push(String::new());
    out.push(messages.text_with("functionHelp", &[("command", command.into())]));
    out.join("\n")
}

/// 某个函数的帮助：位置参数、选项、通用选项与示例
pub fn function(command: &str, name: &str, interface: &Interface, messages: &Messages) -> String {
    let params = &interface.params;
    let required: Vec<&str> = params["required"]
        .as_array()
        .map(|r| r.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let positional = positionals(params);
    let mut usage = format!("  {command} {name}");
    let mut arguments = Vec::new();
    for (param, schema) in &positional {
        let is_required = required.contains(&param.as_str()) && schema["x-stdin"] != true;
        let shown = if schema["type"] == "array" {
            format!("<{param}>...")
        } else {
            format!("<{param}>")
        };
        usage.push(' ');
        usage.push_str(&if is_required {
            shown.clone()
        } else {
            format!("[{shown}]")
        });
        arguments.push((shown, describe(schema, is_required, messages)));
    }
    usage.push_str(&format!(" [{}]", messages.text("placeholder.options")));

    let options: Vec<(String, String)> = params["properties"]
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(param, _)| !positional.iter().any(|(p, _)| p == param))
        .map(|(param, schema)| {
            let flag = format!("--{}", kebab(param));
            let flag = match schema["type"].as_str() {
                Some("boolean") => flag,
                Some("array") => format!("{flag} <{}>...", kebab(param)),
                _ => format!("{flag} <{}>", kebab(param)),
            };
            (
                flag,
                describe(schema, required.contains(&param.as_str()), messages),
            )
        })
        .collect();

    let mut out = vec![
        format!(
            "{command} {name} — {}",
            messages.manifest_text(&interface.summary)
        ),
        String::new(),
        messages.heading("usage"),
        usage,
    ];
    section(&mut out, messages.heading("arguments"), &arguments);
    section(&mut out, messages.heading("options"), &options);
    section(
        &mut out,
        messages.heading("globalOptions"),
        &global_options(messages),
    );
    let examples: Vec<(String, String)> = interface
        .examples
        .iter()
        .map(|e| (e.clone(), String::new()))
        .collect();
    section(&mut out, messages.heading("examples"), &examples);
    out.join("\n")
}

#[cfg(test)]
#[path = "help_test.rs"]
mod tests;
