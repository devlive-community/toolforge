//! LibreOffice：查找安装位置，并以无界面方式转换文档（Word → PDF、PDF → Word、旧格式 → docx）。

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::Serialize;
use tf_plugin_api::{PluginError, PluginResult, TaskContext, cancelled};

/// 单个文件的转换超时
const TIMEOUT: Duration = Duration::from_secs(180);

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Engine {
    pub path: String,
    pub version: Option<String>,
}

fn candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            for name in ["soffice", "libreoffice", "soffice.exe"] {
                out.push(dir.join(name));
            }
        }
    }
    if cfg!(target_os = "macos") {
        out.push("/Applications/LibreOffice.app/Contents/MacOS/soffice".into());
        if let Some(home) = std::env::var_os("HOME") {
            out.push(Path::new(&home).join("Applications/LibreOffice.app/Contents/MacOS/soffice"));
        }
    } else if cfg!(windows) {
        for var in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
            if let Some(dir) = std::env::var_os(var) {
                out.push(
                    Path::new(&dir)
                        .join("LibreOffice")
                        .join("program")
                        .join("soffice.exe"),
                );
            }
        }
    } else {
        out.extend(
            [
                "/usr/bin/soffice",
                "/usr/lib/libreoffice/program/soffice",
                "/opt/libreoffice/program/soffice",
                "/snap/bin/libreoffice",
            ]
            .map(PathBuf::from),
        );
        if let Ok(entries) = std::fs::read_dir("/opt") {
            for entry in entries.flatten() {
                if entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("libreoffice")
                {
                    out.push(entry.path().join("program").join("soffice"));
                }
            }
        }
    }
    out
}

/// 查找 LibreOffice；返回第一个能运行并报出版本的程序
pub fn find() -> Option<Engine> {
    candidates()
        .into_iter()
        .filter(|p| p.is_file())
        .find_map(|path| {
            let output = Command::new(&path)
                .arg("--version")
                .stdin(Stdio::null())
                .output()
                .ok()?;
            let text = String::from_utf8_lossy(&output.stdout);
            let line = text.lines().find(|l| l.contains("LibreOffice"))?;
            let version = line.split_whitespace().nth(1).map(str::to_owned);
            Some(Engine {
                path: path.to_string_lossy().into_owned(),
                version,
            })
        })
}

/// 独立的配置目录：避免与用户正在使用的 LibreOffice 冲突
fn profile() -> String {
    let dir = std::env::temp_dir().join("toolforge-libreoffice-profile");
    let path = dir.to_string_lossy().replace('\\', "/");
    if path.starts_with('/') {
        format!("file://{path}")
    } else {
        format!("file:///{path}")
    }
}

/// 用 LibreOffice 转换，返回生成的文件（位于 out_dir）
pub fn convert(
    engine: &Engine,
    input: &Path,
    out_dir: &Path,
    filter: &str,
    import_filter: Option<&str>,
    ctx: &dyn TaskContext,
) -> PluginResult<PathBuf> {
    let mut command = Command::new(&engine.path);
    command
        .arg(format!("-env:UserInstallation={}", profile()))
        .args([
            "--headless",
            "--norestore",
            "--nolockcheck",
            "--nodefault",
            "--nologo",
        ]);
    if let Some(import) = import_filter {
        command.arg(format!("--infilter={import}"));
    }
    command
        .args(["--convert-to", filter, "--outdir"])
        .arg(out_dir)
        .arg(input)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|e| PluginError::new("doc.engine_failed").with("detail", e.to_string()))?;
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|e| PluginError::new("doc.engine_failed").with("detail", e.to_string()))?
        {
            break status;
        }
        if ctx.is_cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(cancelled());
        }
        if started.elapsed() > TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            return Err(PluginError::new("doc.engine_timeout").with("seconds", TIMEOUT.as_secs()));
        }
        std::thread::sleep(Duration::from_millis(200));
    };
    let ext = filter.split(':').next().unwrap_or(filter);
    let stem = input
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let produced = out_dir.join(format!("{stem}.{ext}"));
    if !status.success() || !produced.is_file() {
        let mut detail = String::new();
        if let Some(mut stderr) = child.stderr.take() {
            use std::io::Read;
            let _ = stderr.read_to_string(&mut detail);
        }
        return Err(PluginError::new("doc.engine_failed").with(
            "detail",
            detail.trim().chars().take(300).collect::<String>(),
        ));
    }
    Ok(produced)
}

#[cfg(test)]
#[path = "engine_test.rs"]
mod tests;
