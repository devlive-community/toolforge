//! 编译命令行工具 tfp，并按 Tauri sidecar 的命名放到 `apps/desktop/src-tauri/binaries/`，
//! 发布时由 `tauri.release.conf.json` 的 `bundle.externalBin` 打进安装包。

use std::path::{Path, PathBuf};
use std::process::Command;

pub const BINARIES_DIR: &str = "apps/desktop/src-tauri/binaries";

fn exe_suffix(target: &str) -> &'static str {
    if target.contains("windows") {
        ".exe"
    } else {
        ""
    }
}

/// Tauri 要求的 sidecar 文件名：`tfp-<目标三元组>[.exe]`
pub fn sidecar_name(target: &str) -> String {
    format!("tfp-{target}{}", exe_suffix(target))
}

/// 从 `rustc -vV` 的输出中取出本机目标三元组
pub fn parse_host(rustc_version: &str) -> Option<String> {
    rustc_version
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .map(|host| host.trim().to_owned())
}

fn host_target() -> Result<String, String> {
    let output = Command::new("rustc")
        .arg("-vV")
        .output()
        .map_err(|e| format!("failed to run rustc: {e}"))?;
    parse_host(&String::from_utf8_lossy(&output.stdout))
        .ok_or_else(|| "cannot determine the host target from rustc -vV".to_owned())
}

/// 编译 tfp 并复制为 sidecar；未指定目标时使用本机目标
pub fn build(root: &Path, target: Option<&str>) -> Result<PathBuf, String> {
    let target = match target {
        Some(target) => target.to_owned(),
        None => host_target()?,
    };
    let status = Command::new("cargo")
        .args([
            "build",
            "--release",
            "--locked",
            "-p",
            "toolforge-cli",
            "--target",
            &target,
        ])
        .current_dir(root)
        .status()
        .map_err(|e| format!("failed to spawn cargo: {e}"))?;
    if !status.success() {
        return Err("building tfp failed".to_owned());
    }
    let built = root
        .join("target")
        .join(&target)
        .join("release")
        .join(format!("tfp{}", exe_suffix(&target)));
    let dir = root.join(BINARIES_DIR);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let out = dir.join(sidecar_name(&target));
    std::fs::copy(&built, &out).map_err(|e| format!("{}: {e}", built.display()))?;
    println!("{}", out.display());
    Ok(out)
}

#[cfg(test)]
#[path = "sidecar_test.rs"]
mod tests;
