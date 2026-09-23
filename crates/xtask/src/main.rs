//! 仓库自动化入口：`cargo xtask <command>`。
//!
//! 本地与 CI 执行完全相同的检查，保证「本地通过 = CI 通过」。

mod check;
mod notes;
mod release;
mod rules;
mod version;

use std::process::ExitCode;

const HELP: &str = "\
Usage: cargo xtask <command>

Commands:
  check [part] [--fast]   Run the checks CI runs; part = rules | rust | web (default: all)
                          --fast skips the web build
  rules                   Scan the repository for convention violations only
  bump <version>          Set the app version in package.json files and Cargo.toml
  notes [ref]             Print release notes for ref (default HEAD) since the previous tag
  release <version>       Check, bump, commit and tag a release (push is left to you)
  help                    Show this message
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("check") => {
            let fast = args.iter().any(|a| a == "--fast");
            match args
                .iter()
                .skip(1)
                .find(|a| !a.starts_with("--"))
                .map(String::as_str)
            {
                None => check::run(check::Part::All, fast),
                Some("rules") => check::run(check::Part::Rules, fast),
                Some("rust") => check::run(check::Part::Rust, fast),
                Some("web") => check::run(check::Part::Web, fast),
                Some(other) => Err(format!("unknown check part `{other}`\n\n{HELP}")),
            }
        }
        Some("rules") => rules::run(&check::workspace_root()),
        Some("bump") => match args.get(1) {
            Some(v) => version::bump(&check::workspace_root(), v),
            None => Err(format!("missing version\n\n{HELP}")),
        },
        Some("notes") => notes::run(&check::workspace_root(), args.get(1).map(String::as_str))
            .map(|notes| print!("{notes}")),
        Some("release") => match args.get(1) {
            Some(v) => release::run(&check::workspace_root(), v),
            None => Err(format!("missing version\n\n{HELP}")),
        },
        Some("help") | None => {
            print!("{HELP}");
            Ok(())
        }
        Some(other) => Err(format!("unknown command `{other}`\n\n{HELP}")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("\n\x1b[31merror:\x1b[0m {message}");
            ExitCode::FAILURE
        }
    }
}
