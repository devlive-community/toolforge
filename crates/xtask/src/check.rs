use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use crate::rules;

pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("xtask lives in crates/xtask")
        .to_path_buf()
}

struct Step {
    name: &'static str,
    program: &'static str,
    args: &'static [&'static str],
}

const RUST_STEPS: &[Step] = &[
    Step {
        name: "rustfmt",
        program: "cargo",
        args: &["fmt", "--all", "--", "--check"],
    },
    Step {
        name: "clippy",
        program: "cargo",
        args: &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--all-features",
            "--",
            "-D",
            "warnings",
        ],
    },
    Step {
        name: "rust tests",
        program: "cargo",
        args: &["test", "--workspace", "--all-features"],
    },
];

const WEB_STEPS: &[Step] = &[Step {
    name: "typecheck",
    program: "pnpm",
    args: &["typecheck"],
}];

const BUILD_STEPS: &[Step] = &[Step {
    name: "web build",
    program: "pnpm",
    args: &["web:build"],
}];

#[derive(Clone, Copy, PartialEq)]
pub enum Part {
    All,
    Rules,
    Rust,
    Web,
}

/// 逐步执行检查；子进程继承终端输出，耗时步骤的日志实时可见。
pub fn run(part: Part, fast: bool) -> Result<(), String> {
    let root = workspace_root();
    let started = Instant::now();

    if matches!(part, Part::All | Part::Rules) {
        stage("convention rules");
        rules::run(&root)?;
    }

    let mut steps: Vec<&Step> = Vec::new();
    if matches!(part, Part::All | Part::Rust) {
        steps.extend(RUST_STEPS);
    }
    if matches!(part, Part::All | Part::Web) {
        steps.extend(WEB_STEPS);
        if !fast {
            steps.extend(BUILD_STEPS);
        }
    }
    for step in steps {
        stage(step.name);
        let begin = Instant::now();
        let status = Command::new(program(step.program))
            .args(step.args)
            .current_dir(&root)
            .status()
            .map_err(|e| format!("failed to spawn `{}`: {e}", step.program))?;
        if !status.success() {
            return Err(format!("`{}` failed", step.name));
        }
        println!(
            "\x1b[32m✓\x1b[0m {} ({:.1}s)",
            step.name,
            begin.elapsed().as_secs_f32()
        );
    }

    println!(
        "\n\x1b[32mAll checks passed\x1b[0m in {:.1}s",
        started.elapsed().as_secs_f32()
    );
    Ok(())
}

fn stage(name: &str) {
    println!("\n\x1b[1;36m==> {name}\x1b[0m");
}

/// Windows 上 pnpm 是 .cmd 脚本
fn program(name: &str) -> String {
    if cfg!(windows) && name == "pnpm" {
        "pnpm.cmd".into()
    } else {
        name.into()
    }
}
