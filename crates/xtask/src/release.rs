//! 本地发布：检查 → 同步版本号 → 提交 → 打标签。推送由人工确认后执行，
//! 推送标签会触发 GitHub Actions 的 release 工作流完成构建与发布。

use std::path::Path;
use std::process::Command;

use crate::{check, version};

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|e| format!("failed to run git: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

pub fn run(root: &Path, next: &str) -> Result<(), String> {
    if !git(root, &["status", "--porcelain"])?.is_empty() {
        return Err("working tree is not clean; commit or stash changes first".into());
    }
    let tag = format!("v{next}");
    if !git(root, &["tag", "--list", &tag])?.is_empty() {
        return Err(format!("tag {tag} already exists"));
    }
    let current = version::current_version(root)?;
    println!("Releasing {current} → {next}");

    check::run(check::Part::All, true)?;
    version::bump(root, next)?;

    git(
        root,
        &[
            "add",
            "package.json",
            "apps/desktop/package.json",
            "Cargo.toml",
            "Cargo.lock",
        ],
    )?;
    git(root, &["commit", "-m", &format!("chore(release): {tag}")])?;
    git(root, &["tag", "-a", &tag, "-m", &tag])?;

    let branch = git(root, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    println!("\n\x1b[32mCreated commit and tag {tag}.\x1b[0m Publish it with:\n");
    println!("  git push origin {branch} && git push origin {tag}\n");
    Ok(())
}
