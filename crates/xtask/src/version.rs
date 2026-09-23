//! 应用版本号管理：root package.json、desktop package.json 与 Cargo workspace 版本保持一致。

use std::path::Path;
use std::process::Command;

use regex::Regex;

/// 需要同步版本号的文件（插件有独立版本，不在此列）
const VERSION_FILES: &[&str] = &["package.json", "apps/desktop/package.json", "Cargo.toml"];

pub fn is_semver(version: &str) -> bool {
    Regex::new(r"^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$")
        .expect("valid regex")
        .is_match(version)
}

/// 替换 package.json 顶层的 version 字段（第一个出现的即顶层字段）
pub fn set_package_version(content: &str, version: &str) -> Option<String> {
    let re = Regex::new(r#"(?m)^(\s*"version"\s*:\s*")[^"]*(")"#).expect("valid regex");
    re.is_match(content).then(|| {
        re.replacen(content, 1, format!("${{1}}{version}${{2}}"))
            .into_owned()
    })
}

/// 替换 Cargo.toml 中 [workspace.package] 段的 version
pub fn set_workspace_version(content: &str, version: &str) -> Option<String> {
    let start = content.find("[workspace.package]")?;
    let section_end = content[start + 1..]
        .find("\n[")
        .map(|i| start + 1 + i)
        .unwrap_or(content.len());
    let section = &content[start..section_end];
    let re = Regex::new(r#"(?m)^(version\s*=\s*")[^"]*(")"#).expect("valid regex");
    if !re.is_match(section) {
        return None;
    }
    let replaced = re.replacen(section, 1, format!("${{1}}{version}${{2}}"));
    Some(format!(
        "{}{}{}",
        &content[..start],
        replaced,
        &content[section_end..]
    ))
}

pub fn current_version(root: &Path) -> Result<String, String> {
    let content = std::fs::read_to_string(root.join("apps/desktop/package.json"))
        .map_err(|e| format!("cannot read desktop package.json: {e}"))?;
    let re = Regex::new(r#"(?m)^\s*"version"\s*:\s*"([^"]*)""#).expect("valid regex");
    re.captures(&content)
        .map(|c| c[1].to_owned())
        .ok_or_else(|| "version field not found in desktop package.json".into())
}

pub fn bump(root: &Path, version: &str) -> Result<(), String> {
    if !is_semver(version) {
        return Err(format!("`{version}` is not a valid semver version"));
    }
    for file in VERSION_FILES {
        let path = root.join(file);
        let content =
            std::fs::read_to_string(&path).map_err(|e| format!("cannot read {file}: {e}"))?;
        let updated = if file.ends_with(".json") {
            set_package_version(&content, version)
        } else {
            set_workspace_version(&content, version)
        }
        .ok_or_else(|| format!("version field not found in {file}"))?;
        std::fs::write(&path, updated).map_err(|e| format!("cannot write {file}: {e}"))?;
        println!("\x1b[32m✓\x1b[0m {file} → {version}");
    }
    // 刷新 Cargo.lock 中 workspace 成员的版本
    let status = Command::new("cargo")
        .args(["update", "--workspace", "--offline", "--quiet"])
        .current_dir(root)
        .status()
        .map_err(|e| format!("failed to run cargo update: {e}"))?;
    if !status.success() {
        return Err("cargo update --workspace failed".into());
    }
    println!("\x1b[32m✓\x1b[0m Cargo.lock");
    Ok(())
}

#[cfg(test)]
#[path = "version_test.rs"]
mod tests;
