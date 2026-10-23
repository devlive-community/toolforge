//! 命令分派与执行：解析参数 → 调用插件 → 渲染结果，并给出退出码。
//!
//! 退出码：0 成功；1 插件返回错误；2 用法错误；3 权限不足；4 需要更新应用；130 已取消。

use std::io::{IsTerminal, Read};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::Value;
use tf_core::AppError;

use crate::args::{self, Global};
use crate::catalog::{Catalog, cli_functions};
use crate::context::CliContext;
use crate::help;
use crate::i18n::{Messages, detect_lang};
use crate::render::render;
use tf_plugin_api::interface::Channel;

pub const EXIT_OK: i32 = 0;
pub const EXIT_FAILED: i32 = 1;
pub const EXIT_USAGE: i32 = 2;
pub const EXIT_PERMISSION: i32 = 3;
pub const EXIT_NEEDS_APP: i32 = 4;
pub const EXIT_CANCELLED: i32 = 130;

pub fn exit_code(code: &str) -> i32 {
    match code {
        c if c.starts_with("cli.") => EXIT_USAGE,
        "plugin.permission_denied" | "fs.permission_denied" => EXIT_PERMISSION,
        "market.needs_app" => EXIT_NEEDS_APP,
        "task.cancelled" => EXIT_CANCELLED,
        _ => EXIT_FAILED,
    }
}

/// 输出错误并返回对应的退出码
fn fail(err: &AppError, messages: &Messages) -> i32 {
    let message = messages.error(&err.code, &err.params);
    eprintln!(
        "{}",
        messages.text_with("errorLine", &[("message", message.into())])
    );
    exit_code(&err.code)
}

/// 标准输入不是终端时读取全部内容；输入保持原样，不去掉结尾换行
fn read_stdin() -> Option<String> {
    let mut stdin = std::io::stdin();
    if stdin.is_terminal() {
        return None;
    }
    let mut text = String::new();
    stdin.read_to_string(&mut text).ok().map(|_| text)
}

/// 程序名（去掉目录与 .exe）
pub fn program_name(arg0: &str) -> String {
    let base = Path::new(arg0)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    base.strip_suffix(".exe").unwrap_or(&base).to_owned()
}

/// 入口：`tfp-<plugin> ...` 直接运行插件；`tfp ...` 先看子命令
pub fn main(argv: &[String]) -> i32 {
    let program = program_name(argv.first().map(String::as_str).unwrap_or("tfp"));
    let rest = argv.get(1..).unwrap_or_default();
    let catalog = Catalog::open();
    if let Some(plugin) = program.strip_prefix("tfp-") {
        return plugin_command(&catalog, true, &program, plugin, rest);
    }

    let (global, positional) = match args::split_global(rest) {
        Ok(split) => split,
        Err(err) => return fail(&err, &Messages::new(detect_lang(None))),
    };
    let messages = Messages::new(detect_lang(global.lang.as_deref()));
    match positional.first().map(String::as_str) {
        None => {
            println!(
                "{}",
                help::global(&catalog.manifests(messages.lang(), Channel::Cli), &messages)
            );
            EXIT_OK
        }
        Some("help") if positional.len() > 1 => {
            let plugin = positional[1].as_str();
            let mut forwarded: Vec<String> = rest
                .iter()
                .filter(|a| *a != "help" && *a != plugin)
                .cloned()
                .collect();
            forwarded.push("--help".to_owned());
            plugin_command(&catalog, true, &format!("tfp {plugin}"), plugin, &forwarded)
        }
        Some("help") => {
            println!(
                "{}",
                help::global(&catalog.manifests(messages.lang(), Channel::Cli), &messages)
            );
            EXIT_OK
        }
        Some("list") => {
            println!(
                "{}",
                help::list(&catalog.manifests(messages.lang(), Channel::Cli), &messages)
            );
            EXIT_OK
        }
        Some("mcp") => crate::mcp::serve(messages.lang()),
        Some("version") => {
            println!("tfp {}", env!("CARGO_PKG_VERSION"));
            EXIT_OK
        }
        Some(plugin) => {
            // 去掉插件名本身，其余参数（含通用选项）原样交给插件命令
            let index = rest.iter().position(|a| a == plugin).unwrap_or(0);
            let mut forwarded = rest.to_vec();
            forwarded.remove(index);
            plugin_command(&catalog, true, &format!("tfp {plugin}"), plugin, &forwarded)
        }
    }
}

/// history 为 true 时把成功的调用记入应用的历史记录（来源 cli）；测试中关闭
fn plugin_command(
    catalog: &Catalog,
    history: bool,
    command: &str,
    plugin: &str,
    argv: &[String],
) -> i32 {
    let (global, rest) = match args::split_global(argv) {
        Ok(split) => split,
        Err(err) => return fail(&err, &Messages::new(detect_lang(None))),
    };
    let lang = detect_lang(global.lang.as_deref());
    let loaded = match catalog.load(plugin, lang) {
        Ok(loaded) => loaded,
        Err(err) => return fail(&err, &Messages::new(lang)),
    };
    let messages = Messages::new(lang).with_plugin(loaded.locale);
    let manifest = loaded.plugin.manifest().clone();
    let functions = cli_functions(&manifest);
    if functions.is_empty() {
        let err = AppError::new("cli.unknown_plugin").with("name", plugin);
        return fail(&err, &messages);
    }

    // 只有一个函数时可以省略函数名
    let (function, args_rest) = match rest.first() {
        Some(first) if functions.iter().any(|(name, _, _)| *name == first) => {
            (first.clone(), &rest[1..])
        }
        _ if functions.len() == 1 && !(global.help && rest.is_empty()) => {
            (functions[0].0.clone(), &rest[..])
        }
        _ if global.help || rest.is_empty() => {
            println!("{}", help::plugin(command, &manifest, &messages));
            return if global.help { EXIT_OK } else { EXIT_USAGE };
        }
        Some(other) => {
            let err = AppError::new("cli.unknown_function")
                .with("name", plugin)
                .with("function", other.as_str());
            return fail(&err, &messages);
        }
        None => unreachable!("empty arguments are handled above"),
    };
    let (_, spec, interface) = functions
        .iter()
        .find(|(name, _, _)| **name == function)
        .expect("function chosen from the list");

    if global.help {
        println!(
            "{}",
            help::function(command, &function, interface, &messages)
        );
        return EXIT_OK;
    }
    let call = Call {
        plugin: &loaded.plugin,
        function: &function,
        task: spec.task,
        interface,
        history,
    };
    run_function(&call, &global, args_rest, &messages)
}

struct Call<'a> {
    plugin: &'a std::sync::Arc<dyn tf_plugin_api::ToolPlugin>,
    function: &'a str,
    task: bool,
    interface: &'a tf_plugin_api::interface::Interface,
    history: bool,
}

fn run_function(call: &Call<'_>, global: &Global, argv: &[String], messages: &Messages) -> i32 {
    let Call {
        plugin,
        function,
        task,
        interface,
        ..
    } = *call;
    let cwd = std::env::current_dir().unwrap_or_default();
    let parsed = match args::parse(
        &interface.params,
        argv,
        global.args.as_deref(),
        &cwd,
        read_stdin,
    ) {
        Ok(parsed) => parsed,
        Err(err) => return fail(&err, messages),
    };

    // 一次进程只运行一个函数；Ctrl+C 处理函数需要 'static 的标志
    let cancelled: &'static AtomicBool = Box::leak(Box::new(AtomicBool::new(false)));
    let result = if task {
        let context = CliContext::new(messages, &plugin.manifest().id, global.verbose, cancelled);
        let _ = ctrlc::set_handler(move || {
            if cancelled.swap(true, Ordering::Relaxed) {
                std::process::exit(EXIT_CANCELLED);
            }
        });
        let result = plugin.run_task(function, parsed.args.clone(), &context);
        context.clear();
        result
    } else {
        plugin.call(function, parsed.args.clone())
    };

    match result {
        Ok(value) => {
            if call.history
                && let Some(store) = tf_host::app_data_dir()
                    .and_then(|dir| tf_core::Store::open(&dir.join("toolforge.db")).ok())
            {
                let masked = tf_host::agent::mask_sensitive(&interface.params, &parsed.args);
                tf_host::history::record(
                    &store,
                    "cli",
                    plugin.manifest(),
                    function,
                    &masked,
                    &value,
                );
            }
            let text = output_text(&value, interface, global.json, messages);
            if !text.is_empty() {
                println!("{text}");
            }
            EXIT_OK
        }
        Err(_) if cancelled.load(Ordering::Relaxed) => {
            eprintln!("{}", messages.text("cancelled"));
            EXIT_CANCELLED
        }
        Err(err) => fail(&AppError::from(err), messages),
    }
}

/// 结果的输出文本：`--json` 时为原始 JSON，否则按接口声明渲染
pub fn output_text(
    value: &Value,
    interface: &tf_plugin_api::interface::Interface,
    json: bool,
    messages: &Messages,
) -> String {
    if json {
        serde_json::to_string_pretty(value).unwrap_or_default()
    } else {
        render(value, interface.output.as_ref(), messages)
    }
}

#[cfg(test)]
#[path = "run_test.rs"]
mod tests;
