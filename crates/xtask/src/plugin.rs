//! 在线插件的打包与签名：`cargo xtask plugin keygen | pack`。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use data_encoding::{BASE64, HEXLOWER};
use ed25519_dalek::SigningKey;
use sha2::{Digest, Sha256};

/// 签名私钥的环境变量（base64 编码的 32 字节种子），CI 中来自仓库机密
pub const KEY_ENV: &str = "TOOLFORGE_PLUGIN_KEY";
pub const DEFAULT_KEY_ID: &str = "toolforge-2026";

fn read_key(text: &str) -> Result<SigningKey, String> {
    let seed: [u8; 32] = BASE64
        .decode(text.trim().as_bytes())
        .ok()
        .and_then(|b| b.try_into().ok())
        .ok_or("the signing key must be 32 bytes in base64")?;
    Ok(SigningKey::from_bytes(&seed))
}

/// 生成密钥对：私钥写入文件（仅当前用户可读），公钥打印到标准输出
pub fn keygen(out: &Path) -> Result<(), String> {
    if out.exists() {
        return Err(format!(
            "{} already exists; refusing to overwrite a signing key",
            out.display()
        ));
    }
    let mut seed = [0u8; 32];
    getrandom::fill(&mut seed).map_err(|e| e.to_string())?;
    let key = SigningKey::from_bytes(&seed);
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(out, BASE64.encode(&seed)).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(out, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| e.to_string())?;
    }
    println!("private key: {}", out.display());
    println!(
        "public key:  {}",
        BASE64.encode(key.verifying_key().as_bytes())
    );
    Ok(())
}

fn run(command: &mut Command) -> Result<(), String> {
    let status = command.status().map_err(|e| format!("{command:?}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{command:?} failed with {status}"))
    }
}

fn crate_name(backend: &Path) -> Result<String, String> {
    let toml = std::fs::read_to_string(backend.join("Cargo.toml")).map_err(|e| e.to_string())?;
    toml.lines()
        .find_map(|l| {
            l.strip_prefix("name = \"")?
                .strip_suffix('"')
                .map(str::to_owned)
        })
        .ok_or_else(|| "backend/Cargo.toml has no package name".to_owned())
}

/// 收集插件目录中要打进包的文件
fn collect(root: &Path, dir: &Path, wasm: Vec<u8>) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let read = |path: &Path| std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()));
    let mut files = BTreeMap::new();
    files.insert(
        "manifest.json".to_owned(),
        read(&dir.join("manifest.json"))?,
    );
    files.insert("plugin.wasm".to_owned(), wasm);
    let ui = dir.join("ui/dist");
    files.insert("ui/index.js".to_owned(), read(&ui.join("index.js"))?);
    if ui.join("index.css").exists() {
        files.insert("ui/index.css".to_owned(), read(&ui.join("index.css"))?);
    }
    if dir.join("icon.svg").exists() {
        files.insert("icon.svg".to_owned(), read(&dir.join("icon.svg"))?);
    }
    for entry in std::fs::read_dir(dir.join("locales")).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.extension().is_some_and(|e| e == "json") {
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            files.insert(format!("locales/{name}"), read(&path)?);
        }
    }
    let _ = root;
    Ok(files)
}

pub struct PackOptions {
    pub slug: String,
    pub out: PathBuf,
    pub key_id: String,
    /// 未提供私钥时生成未签名的包
    pub unsigned: bool,
}

/// 编译 WASM 组件与界面模块，打包为 dist/plugins/<id>-<version>.tfp
pub fn pack(root: &Path, options: &PackOptions) -> Result<PathBuf, String> {
    let dir = root.join("market").join(&options.slug);
    let backend = dir.join("backend");
    let name = crate_name(&backend)?;
    run(Command::new(env!("CARGO")).current_dir(root).args([
        "build",
        "--release",
        "--target",
        "wasm32-wasip2",
        "-p",
        &name,
    ]))?;
    let wasm_path = root
        .join("target/wasm32-wasip2/release")
        .join(format!("{}.wasm", name.replace('-', "_")));
    let wasm = std::fs::read(&wasm_path).map_err(|e| format!("{}: {e}", wasm_path.display()))?;
    run(Command::new(if cfg!(windows) {
        "corepack.cmd"
    } else {
        "corepack"
    })
    .current_dir(dir.join("ui"))
    .args(["pnpm", "exec", "vite", "build", "--config"])
    .arg(root.join("packages/plugin-build/vite.config.ts")))?;

    let files = collect(root, &dir, wasm)?;
    let manifest: serde_json::Value =
        serde_json::from_slice(&files["manifest.json"]).map_err(|e| e.to_string())?;
    let key = match std::env::var(KEY_ENV) {
        Ok(text) if !options.unsigned => Some(read_key(&text)?),
        _ if options.unsigned => None,
        _ => return Err(format!("set {KEY_ENV} to sign, or pass --unsigned")),
    };
    let bytes = tf_package::build(&files, key.as_ref().map(|k| (options.key_id.as_str(), k)))
        .map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&options.out).map_err(|e| e.to_string())?;
    let out = options.out.join(format!(
        "{}-{}.tfp",
        manifest["id"].as_str().unwrap_or("plugin"),
        manifest["version"].as_str().unwrap_or("0.0.0")
    ));
    std::fs::write(&out, &bytes).map_err(|e| e.to_string())?;
    println!("{}", out.display());
    println!("sha256 {}", HEXLOWER.encode(&Sha256::digest(&bytes)));
    println!("{}", if key.is_some() { "signed" } else { "unsigned" });
    Ok(out)
}
