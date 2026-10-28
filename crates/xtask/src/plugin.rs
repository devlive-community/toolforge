//! 在线插件的构建、独立签名与 COS 发布目录。

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

/// 与 xtask 同一工具链的 cargo。显式指定同目录的 rustc：cargo 默认经 PATH 中的 rustup 代理
/// 调用 rustc，代理会读取依赖包自带的 rust-toolchain.toml（如 pic-scale 指定 nightly），
/// 在没有设置 RUSTUP_TOOLCHAIN 的环境（CI）里换成别的工具链
fn cargo() -> Command {
    let cargo = Path::new(env!("CARGO"));
    let mut command = Command::new(cargo);
    let rustc = cargo.with_file_name(if cfg!(windows) { "rustc.exe" } else { "rustc" });
    if rustc.is_file() {
        command.env("RUSTC", rustc);
    }
    command
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
    run(cargo().env_remove(KEY_ENV).current_dir(root).args([
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
    .env_remove(KEY_ENV)
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

/// 独立签名已经构建的未签名候选包；此命令不启动 Cargo、Vite 或其他子进程。
pub fn sign(input: &Path, out: &Path, key_id: &str) -> Result<PathBuf, String> {
    let bytes = std::fs::read(input).map_err(|e| e.to_string())?;
    let package = tf_package::open(&bytes).map_err(|e| e.to_string())?;
    if !matches!(package.trust, tf_package::Trust::Unsigned) {
        return Err("refusing to re-sign an already signed package".into());
    }
    let key = read_key(&std::env::var(KEY_ENV).map_err(|_| format!("set {KEY_ENV} to sign"))?)?;
    let public = BASE64.encode(key.verifying_key().as_bytes());
    if !tf_package::OFFICIAL_KEYS
        .iter()
        .any(|(id, value)| *id == key_id && *value == public)
    {
        return Err("signing key does not match the application's trusted official key".into());
    }
    let bytes =
        tf_package::build(&package.files, Some((key_id, &key))).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let target = out.join(input.file_name().ok_or("missing package filename")?);
    if target.exists() {
        return Err(format!(
            "{} already exists; refusing to overwrite",
            target.display()
        ));
    }
    std::fs::write(&target, bytes).map_err(|e| e.to_string())?;
    println!("{}", target.display());
    Ok(target)
}

/// 发布及历史搬迁必须校验所有制品为已知官方签名，不重新打包历史字节。
pub fn verify(dir: &Path) -> Result<(), String> {
    let mut paths = Vec::new();
    collect_tfp(dir, &mut paths)?;
    for path in paths {
        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        let package = tf_package::open(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        if !matches!(package.trust, tf_package::Trust::Official { .. }) {
            return Err(format!("{} is not officially signed", path.display()));
        }
    }
    Ok(())
}

/// manifest 中 `i18n:key` 形式的文案，从包内语言包中取出各语言的值
fn localized(
    package: &tf_package::Package,
    value: &str,
) -> serde_json::Map<String, serde_json::Value> {
    let mut out = serde_json::Map::new();
    let Some(key) = value.strip_prefix("i18n:") else {
        out.insert("en-US".into(), value.into());
        return out;
    };
    for (path, data) in &package.files {
        let Some(locale) = path
            .strip_prefix("locales/")
            .and_then(|p| p.strip_suffix(".json"))
        else {
            continue;
        };
        if let Some(text) = serde_json::from_slice::<serde_json::Value>(data)
            .ok()
            .and_then(|v| v.get(key).and_then(|t| t.as_str()).map(str::to_owned))
        {
            out.insert(locale.to_owned(), text.into());
        }
    }
    out
}

/// 递归收集目录下的 .tfp 文件（CI 会把历史版本下载到 previous/ 子目录供全量重建）
/// 插件对外开放的调用通道（cli / agent），汇总各函数的接口声明，供市场展示
pub fn channels(manifest: &serde_json::Value) -> Vec<String> {
    let mut out: Vec<String> = manifest
        .get("functions")
        .and_then(serde_json::Value::as_object)
        .into_iter()
        .flat_map(|functions| functions.values())
        .filter_map(|spec| spec.pointer("/interface/expose")?.as_array())
        .flatten()
        .filter_map(|c| c.as_str().filter(|c| matches!(*c, "cli" | "agent")))
        .map(str::to_owned)
        .collect();
    out.sort();
    out.dedup();
    out
}

fn collect_tfp(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.is_dir() {
            collect_tfp(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "tfp") {
            out.push(path);
        }
    }
    Ok(())
}

/// 版本比较键：按数字段比较，0.1.10 > 0.1.9（文件名排序对这类版本不可靠）
fn version_key(version: &str) -> Vec<u64> {
    version
        .split(['.', '-'])
        .map(|part| part.parse().unwrap_or(0))
        .collect()
}

/// 为不可变制品生成全版本目录，并生成只包含最高 active 版本的兼容市场索引。
/// 旧目录保留 withdrawn/revoked 状态；同版本的任何字节变化都会拒绝发布。
pub fn index(
    dir: &Path,
    stats_base: &str,
    merge: Option<&Path>,
    merge_catalog: Option<&Path>,
    source_commit: &str,
    generation: u64,
    dates: &crate::release_dates::ReleaseDates,
) -> Result<PathBuf, String> {
    if !stats_base.starts_with("https://") || stats_base.contains(['?', '#']) {
        return Err("stats base URL must use HTTPS without a query or fragment".into());
    }
    if generation == 0
        || source_commit.len() != 40
        || !source_commit.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(
            "source commit must be a 40-digit commit SHA and generation must be positive".into(),
        );
    }
    let read_json = |path: &Path| -> Result<serde_json::Value, String> {
        serde_json::from_slice(
            &std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?,
        )
        .map_err(|e| format!("{}: {e}", path.display()))
    };
    let mut plugins = match merge {
        Some(path) => read_json(path)?["plugins"]
            .as_array()
            .cloned()
            .ok_or("invalid old registry")?,
        None => Vec::new(),
    };
    let mut prior = BTreeMap::new();
    let previous_catalog = merge_catalog.map(read_json).transpose()?;
    if let Some(old) = &previous_catalog {
        if old["schemaVersion"] != 1 || old["generation"].as_u64().is_none_or(|g| g > generation) {
            return Err("catalog generation must not go backwards".into());
        }
        for item in old["plugins"]
            .as_array()
            .ok_or("invalid old release catalog")?
        {
            let key = (
                item["id"].as_str().ok_or("missing plugin id")?.to_owned(),
                item["version"]
                    .as_str()
                    .ok_or("missing plugin version")?
                    .to_owned(),
            );
            if prior.insert(key, item.clone()).is_some() {
                return Err("duplicate version in old catalog".into());
            }
        }
    }
    let mut paths = Vec::new();
    collect_tfp(dir, &mut paths)?;
    paths.sort();
    let mut catalog = BTreeMap::new();
    let mut best: BTreeMap<String, (Vec<u64>, serde_json::Value)> = BTreeMap::new();
    for path in paths {
        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        let package = tf_package::open(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        let manifest = &package.manifest;
        if manifest.version.is_empty()
            || manifest.version.len() > 64
            || !manifest
                .version
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b".+-".contains(&b))
        {
            return Err("plugin version is not a safe URL path segment".into());
        }
        let digest = HEXLOWER.encode(&Sha256::digest(&bytes));
        let object_key = format!(
            "packages/{}/{}/{}.tfp",
            manifest.id, manifest.version, digest
        );
        let key = (manifest.id.clone(), manifest.version.clone());
        let mut item = serde_json::json!({
            "id": manifest.id, "version": manifest.version, "sha256": digest,
            "size": bytes.len(), "objectKey": object_key, "status": "active",
        });
        if let Some(old) = prior.get(&key) {
            for field in ["sha256", "size", "objectKey"] {
                if item[field] != old[field] {
                    return Err(format!(
                        "immutable published version changed: {} {}",
                        manifest.id, manifest.version
                    ));
                }
            }
            if !matches!(
                old["status"].as_str(),
                Some("active" | "withdrawn" | "revoked")
            ) {
                return Err("invalid catalog status".into());
            }
            item["status"] = old["status"].clone();
            if let Some(published) = old.get("publishedAt") {
                item["publishedAt"] = published.clone();
            }
        }
        // 旧目录没有记录发布时间时，按 git 历史补上；本次新增的版本用发布源提交的时间
        if item.get("publishedAt").is_none_or(|v| v.is_null())
            && let Some(at) = dates.versions.get(&key).or(dates.now.as_ref())
        {
            item["publishedAt"] = at.clone().into();
        }
        if let Some(seen) = catalog.insert(key, item.clone())
            && seen != item
        {
            return Err(format!(
                "conflicting packages for {} {}",
                manifest.id, manifest.version
            ));
        }
        if item["status"] != "active" {
            continue;
        }
        let raw: serde_json::Value =
            serde_json::from_slice(&package.files["manifest.json"]).map_err(|e| e.to_string())?;
        let entry = serde_json::json!({
            "minAppVersion": raw.get("minAppVersion"), "accent": raw.get("accent"),
            "channels": channels(&raw),
            "id": manifest.id, "version": manifest.version, "category": manifest.category,
            "names": localized(&package, &manifest.name), "descriptions": localized(&package, &manifest.description),
            "icon": package.files.get("icon.svg").map(|svg| String::from_utf8_lossy(svg).into_owned()),
            "url": format!("{}/v1/plugins/{}/versions/{}/download", stats_base.trim_end_matches('/'), manifest.id, manifest.version),
            "sha256": digest, "size": bytes.len(),
            "signed": matches!(package.trust, tf_package::Trust::Official { .. }),
        });
        let version = version_key(&manifest.version);
        if best
            .get(&manifest.id)
            .is_none_or(|(seen, _)| version > *seen)
        {
            best.insert(manifest.id.clone(), (version, entry));
        }
    }
    for key in prior.keys() {
        if !catalog.contains_key(key) {
            return Err(format!(
                "published package missing from complete inventory: {} {}",
                key.0, key.1
            ));
        }
    }
    // 发布时间：首次发布取该插件所有版本中最早的时间，更新时间取市场当前版本的时间
    let mut first: BTreeMap<&str, &str> = BTreeMap::new();
    let catalog_dates = catalog
        .iter()
        .filter_map(|((id, _), item)| Some((id.as_str(), item["publishedAt"].as_str()?)));
    // git 历史里更早的版本（安装包已不在本次清单中）也算首次发布
    let history_dates = dates
        .versions
        .iter()
        .map(|((id, _), at)| (id.as_str(), at.as_str()));
    for (id, at) in catalog_dates.chain(history_dates) {
        let slot = first.entry(id).or_insert(at);
        if at < *slot {
            *slot = at;
        }
    }
    for (_, entry) in best.values_mut() {
        let id = entry["id"].as_str().unwrap_or_default().to_owned();
        let version = entry["version"].as_str().unwrap_or_default().to_owned();
        if let Some(at) = first.get(id.as_str()) {
            entry["publishedAt"] = (*at).into();
        }
        if let Some(at) = catalog
            .get(&(id, version))
            .and_then(|i| i["publishedAt"].as_str())
        {
            entry["updatedAt"] = at.into();
        }
    }
    // 即使使用旧索引合并，也统一改写到统计 API，绝不保留旧的文件直链。
    plugins.retain(|p| !catalog.keys().any(|(id, _)| p["id"] == id.as_str()));
    plugins.extend(best.into_values().map(|(_, entry)| entry));
    for entry in &mut plugins {
        let id = entry["id"].as_str().ok_or("missing merged plugin id")?;
        let version = entry["version"]
            .as_str()
            .ok_or("missing merged plugin version")?;
        entry["url"] = format!(
            "{}/v1/plugins/{id}/versions/{version}/download",
            stats_base.trim_end_matches('/')
        )
        .into();
    }
    plugins.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    let out = dir.join("registry.json");
    let registry = serde_json::json!({"version": 1, "sourceCommit": source_commit, "generation": generation, "plugins": plugins});
    let full = serde_json::json!({"schemaVersion": 1, "sourceCommit": source_commit, "generation": generation, "plugins": catalog.into_values().collect::<Vec<_>>()});
    if let Some(old) = previous_catalog
        && old["generation"] == generation
        && old != full
    {
        return Err("same catalog generation has different content".into());
    }
    for (path, value) in [
        (&out, &registry),
        (&dir.join("release-catalog.json"), &full),
    ] {
        std::fs::write(
            path,
            serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
    }
    println!("{} ({} plugins)", out.display(), plugins.len());
    Ok(out)
}

#[cfg(test)]
#[path = "plugin_test.rs"]
mod tests;
