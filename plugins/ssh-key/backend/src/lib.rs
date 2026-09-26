//! SSH 密钥插件后端：生成 Ed25519 / ECDSA / RSA 密钥，查看公钥与私钥信息（指纹、随机图），
//! 修改私钥口令，并以正确的权限保存到文件。

mod detect;
mod keys;

use std::path::Path;

use serde::Deserialize;
use serde_json::{Value, json};
use tf_plugin_api::{
    Detection, LogLevel, Manifest, PluginError, PluginResult, TaskContext, ToolPlugin, parse_args,
    to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

#[derive(Deserialize)]
struct GenerateArgs {
    kind: keys::Kind,
    #[serde(default)]
    comment: String,
    #[serde(default)]
    passphrase: Option<String>,
}

#[derive(Deserialize)]
struct InspectArgs {
    text: String,
    #[serde(default)]
    passphrase: Option<String>,
}

#[derive(Deserialize)]
struct PassphraseArgs {
    private: String,
    #[serde(default)]
    old: Option<String>,
    #[serde(default)]
    new: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveArgs {
    path: String,
    private: String,
    /// 为空时只保存私钥
    #[serde(default)]
    public: String,
    #[serde(default)]
    overwrite: bool,
}

fn io(path: &Path, err: std::io::Error) -> PluginError {
    PluginError::new("fs.io")
        .with("path", path.to_string_lossy().as_ref())
        .with("detail", err.to_string())
}

/// 私钥写为 0600、公钥 0644（Unix）；默认不覆盖已有文件
fn save(args: &SaveArgs) -> PluginResult<Value> {
    let private = Path::new(&args.path);
    let public = Path::new(&format!("{}.pub", args.path)).to_owned();
    let with_public = !args.public.trim().is_empty();
    if !args.overwrite {
        let targets: Vec<&Path> = if with_public {
            vec![private, public.as_path()]
        } else {
            vec![private]
        };
        for path in targets {
            if path.exists() {
                return Err(PluginError::new("ssh.file_exists")
                    .with("path", path.to_string_lossy().as_ref()));
            }
        }
    }
    if let Some(parent) = private.parent() {
        std::fs::create_dir_all(parent).map_err(|e| io(parent, e))?;
    }
    write_private(private, &args.private)?;
    if with_public {
        std::fs::write(&public, format!("{}\n", args.public.trim())).map_err(|e| io(&public, e))?;
    }
    Ok(
        json!({ "private": args.path, "public": with_public.then(|| public.to_string_lossy().into_owned()) }),
    )
}

#[cfg(unix)]
fn write_private(path: &Path, content: &str) -> PluginResult<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    use std::os::unix::fs::PermissionsExt;
    // 先以 0600 创建，避免私钥在写入过程中对其他用户可读
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .map_err(|e| io(path, e))?;
    file.write_all(content.as_bytes())
        .map_err(|e| io(path, e))?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).map_err(|e| io(path, e))
}

#[cfg(not(unix))]
fn write_private(path: &Path, content: &str) -> PluginResult<()> {
    std::fs::write(path, content).map_err(|e| io(path, e))
}

pub struct SshKey {
    manifest: Manifest,
}

impl Default for SshKey {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for SshKey {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn detect(&self, text: &str) -> Option<Detection> {
        detect::detect(text)
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "inspect" => {
                let args: InspectArgs = parse_args(args)?;
                to_value(keys::inspect(&args.text, args.passphrase.as_deref()))
            }
            "passphrase" => {
                let args: PassphraseArgs = parse_args(args)?;
                to_value(keys::change_passphrase(
                    &args.private,
                    args.old.as_deref(),
                    args.new.as_deref(),
                )?)
            }
            "save" => save(&parse_args(args)?),
            other => Err(unknown_function(other)),
        }
    }

    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        match function {
            // RSA 4096 可能需要几秒，作为任务运行
            "generate" => {
                let args: GenerateArgs = parse_args(args)?;
                ctx.stage("ssh.generate");
                let generated =
                    keys::generate(args.kind, &args.comment, args.passphrase.as_deref())?;
                ctx.log(
                    LogLevel::Info,
                    "ssh.generated",
                    json!({ "algorithm": generated.info.algorithm, "bits": generated.info.bits, "fingerprint": generated.info.sha256 }),
                );
                to_value(generated)
            }
            _ => self.call(function, args),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
