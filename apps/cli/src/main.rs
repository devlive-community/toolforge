//! tfp：从命令行运行 ToolForge 插件。
//!
//! 以 `tfp-<插件>` 的名字运行（软链接或 Windows 的 .cmd）时直接调用该插件，
//! 以 `tfp` 运行时第一个参数是插件名或子命令（list / help / version / mcp）。
//! 可调用的函数、参数与帮助全部来自插件 manifest 的接口声明。

mod args;
mod builtin;
mod catalog;
mod context;
mod help;
mod i18n;
mod mcp;
mod mcp_tools;
mod render;
mod run;

fn main() {
    let argv: Vec<String> = std::env::args_os()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    std::process::exit(run::main(&argv));
}
