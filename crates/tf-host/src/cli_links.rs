//! 命令行工具的入口链接：在 `~/.toolforge/bin` 下生成 `tfp` 与 `tfp-<插件>`，都指向安装包里的 tfp。
//!
//! macOS / Linux 用软链接（tfp 按程序名分派）；Windows 用 `.cmd`，把插件名作为第一个参数传给 tfp.exe。
//! 目录只放这些链接，重新生成时删掉不再需要的旧链接。

use std::io;
use std::path::{Path, PathBuf};

/// 链接目录：`~/.toolforge/bin`
pub fn bin_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".toolforge").join("bin"))
}

/// 安装包里的 tfp：Tauri sidecar 与主程序在同一目录
pub fn sidecar_beside(exe: &Path) -> PathBuf {
    let name = if cfg!(windows) { "tfp.exe" } else { "tfp" };
    exe.with_file_name(name)
}

fn entry_name(command: &str) -> String {
    if cfg!(windows) {
        format!("{command}.cmd")
    } else {
        command.to_owned()
    }
}

/// 由本模块生成的条目
fn is_ours(name: &str) -> bool {
    let name = name.strip_suffix(".cmd").unwrap_or(name);
    name == "tfp" || name.starts_with("tfp-")
}

/// 已生成 tfp 入口
pub fn installed(dir: &Path) -> bool {
    dir.join(entry_name("tfp")).exists() || dir.join(entry_name("tfp")).is_symlink()
}

/// 已生成的插件命令名（tfp-<插件>），排序
pub fn commands(dir: &Path) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let name = entry.ok()?.file_name().to_string_lossy().into_owned();
            let name = name.strip_suffix(".cmd").unwrap_or(&name).to_owned();
            name.starts_with("tfp-").then_some(name)
        })
        .collect();
    out.sort();
    out
}

fn create(dir: &Path, sidecar: &Path, command: &str, plugin: Option<&str>) -> io::Result<()> {
    let path = dir.join(entry_name(command));
    if path.exists() || path.is_symlink() {
        std::fs::remove_file(&path)?;
    }
    if cfg!(windows) {
        let args = plugin.map(|p| format!(" {p}")).unwrap_or_default();
        std::fs::write(&path, format!("@\"{}\"{args} %*\r\n", sidecar.display()))
    } else {
        link(sidecar, &path)
    }
}

#[cfg(unix)]
fn link(target: &Path, path: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, path)
}

#[cfg(not(unix))]
fn link(target: &Path, path: &Path) -> io::Result<()> {
    std::fs::copy(target, path).map(|_| ())
}

/// 生成 tfp 与各插件的入口，并删除不再需要的旧入口
pub fn write(dir: &Path, sidecar: &Path, plugins: &[String]) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let wanted: Vec<String> = std::iter::once("tfp".to_owned())
        .chain(plugins.iter().map(|p| format!("tfp-{p}")))
        .map(|c| entry_name(&c))
        .collect();
    for entry in std::fs::read_dir(dir)?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if is_ours(&name) && !wanted.contains(&name) {
            std::fs::remove_file(entry.path())?;
        }
    }
    create(dir, sidecar, "tfp", None)?;
    for plugin in plugins {
        create(dir, sidecar, &format!("tfp-{plugin}"), Some(plugin))?;
    }
    Ok(())
}

/// 删除本模块生成的全部入口；目录空了就一并删除
pub fn remove(dir: &Path) -> io::Result<()> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        if is_ours(&entry.file_name().to_string_lossy()) {
            std::fs::remove_file(entry.path())?;
        }
    }
    if std::fs::read_dir(dir)?.next().is_none() {
        std::fs::remove_dir(dir)?;
    }
    Ok(())
}

/// 把链接目录加入 PATH 的命令，供界面展示与复制
pub fn path_hint(dir: &Path) -> String {
    if cfg!(windows) {
        format!(
            "[Environment]::SetEnvironmentVariable(\"Path\", [Environment]::GetEnvironmentVariable(\"Path\", \"User\") + \";{}\", \"User\")",
            dir.display()
        )
    } else {
        "export PATH=\"$HOME/.toolforge/bin:$PATH\"".to_owned()
    }
}

#[cfg(test)]
#[path = "cli_links_test.rs"]
mod tests;
