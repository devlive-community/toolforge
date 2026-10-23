//! 在线安装的插件：校验 .tfp 包后解压到数据目录，启动时加载为 WASM 插件并注册。
//!
//! 目录结构：`<数据目录>/installed/<插件 id>/` 存放包内文件，另有 `install.json` 记录版本、
//! 签名状态与来源。安装先写入临时目录再整体替换，失败不会留下半个插件。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tf_core::{AppError, AppResult, store::now_millis};
use tf_package::{PackageError, Trust};
use tf_plugin_api::Manifest;
use tf_wasm::WasmPlugin;

pub const INSTALL_FILE: &str = "install.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InstallInfo {
    pub id: String,
    pub version: String,
    /// official / unsigned / unknownKey
    pub trust: String,
    pub key_id: Option<String>,
    pub digest: String,
    pub installed_at: i64,
    /// 安装来源：网址，或本地文件时为 None
    pub url: Option<String>,
    /// 插件名称：语言 → 名称（"*" 为未翻译的名称）；旧版本安装记录中没有
    #[serde(default)]
    pub names: BTreeMap<String, String>,
}

/// 包内声明的插件名称。manifest.name 为 `i18n:键` 时从 locales/<语言>.json 读取，否则原样作为 "*"
pub fn package_names(package: &tf_package::Package) -> BTreeMap<String, String> {
    let mut names = BTreeMap::new();
    let Some(key) = package.manifest.name.strip_prefix("i18n:") else {
        names.insert("*".to_owned(), package.manifest.name.clone());
        return names;
    };
    for (path, bytes) in &package.files {
        let Some(locale) = path
            .strip_prefix("locales/")
            .and_then(|p| p.strip_suffix(".json"))
        else {
            continue;
        };
        let Ok(value) = serde_json::from_slice::<Value>(bytes) else {
            continue;
        };
        // 键可以是 a.b 形式的嵌套路径
        let found = key
            .split('.')
            .try_fold(&value, |node, part| node.get(part))
            .and_then(Value::as_str);
        if let Some(name) = found {
            names.insert(locale.to_owned(), name.to_owned());
        }
    }
    names
}

pub fn package_error(err: PackageError) -> AppError {
    let mut out = AppError::new(err.code);
    for (key, value) in err.params {
        out = out.with(key, value);
    }
    out
}

fn trust_parts(trust: &Trust) -> (String, Option<String>) {
    match trust {
        Trust::Official { key_id } => ("official".into(), Some(key_id.clone())),
        Trust::Unsigned => ("unsigned".into(), None),
        Trust::UnknownKey { key_id } => ("unknownKey".into(), Some(key_id.clone())),
    }
}

/// 允许通过 tfplugin:// 读取的文件
fn servable(path: &str) -> bool {
    matches!(path, "ui/index.js" | "ui/index.css" | "icon.svg")
}

pub struct Installed {
    root: PathBuf,
    /// 当前应用版本，用于校验插件声明的 minAppVersion
    app_version: String,
}

impl Installed {
    pub fn new(root: PathBuf, app_version: impl Into<String>) -> Self {
        Self {
            root,
            app_version: app_version.into(),
        }
    }

    /// 插件要求的应用版本高于当前版本时拒绝；未声明的旧插件视为兼容
    fn check_app_version(&self, manifest: &Manifest) -> AppResult<()> {
        match &manifest.min_app_version {
            Some(min) if !crate::version::at_least(&self.app_version, min) => {
                Err(AppError::new("market.needs_app")
                    .with("id", manifest.id.as_str())
                    .with("version", min.as_str()))
            }
            _ => Ok(()),
        }
    }

    fn dir(&self, id: &str) -> AppResult<PathBuf> {
        if !tf_package::valid_id(id) {
            return Err(AppError::new("package.bad_id").with("id", id));
        }
        Ok(self.root.join(id))
    }

    /// 校验并安装；非官方签名的包只有在 allow_unsigned 时安装。is_builtin 用于拒绝与内置插件重名
    pub fn install(
        &self,
        bytes: &[u8],
        url: Option<&str>,
        allow_unsigned: bool,
        is_builtin: impl Fn(&str) -> bool,
    ) -> AppResult<(InstallInfo, Manifest)> {
        let package = tf_package::open(bytes).map_err(package_error)?;
        let id = package.manifest.id.clone();
        let names = package_names(&package);
        let names_value = || {
            Value::Object(
                names
                    .iter()
                    .map(|(k, v)| (k.clone(), Value::String(v.clone())))
                    .collect(),
            )
        };
        if is_builtin(&id) {
            return Err(AppError::new("package.builtin_conflict")
                .with("id", id)
                .with("names", names_value()));
        }
        self.check_app_version(&package.manifest)?;
        let (trust, key_id) = trust_parts(&package.trust);
        if trust != "official" && !allow_unsigned {
            let mut err = AppError::new("package.untrusted")
                .with("id", id.as_str())
                .with("names", names_value())
                .with("trust", trust.as_str())
                .with("version", package.manifest.version.as_str());
            if let Some(key) = &key_id {
                err = err.with("keyId", key.as_str());
            }
            return Err(err);
        }
        let info = InstallInfo {
            id: id.clone(),
            version: package.manifest.version.clone(),
            trust,
            key_id,
            digest: package.digest.clone(),
            installed_at: now_millis(),
            url: url.map(str::to_owned),
            names,
        };
        std::fs::create_dir_all(&self.root)?;
        let target = self.dir(&id)?;
        let staging = self.root.join(format!(".staging-{id}-{}", now_millis()));
        let result = (|| -> AppResult<()> {
            for (path, data) in &package.files {
                let file = staging.join(path);
                if let Some(parent) = file.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(file, data)?;
            }
            std::fs::write(
                staging.join(INSTALL_FILE),
                serde_json::to_vec_pretty(&info)
                    .map_err(|e| AppError::new("fs.io").with("detail", e.to_string()))?,
            )?;
            if target.exists() {
                std::fs::remove_dir_all(&target)?;
            }
            std::fs::rename(&staging, &target)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_dir_all(&staging);
        }
        result?;
        Ok((info, package.manifest))
    }

    pub fn uninstall(&self, id: &str) -> AppResult<()> {
        let dir = self.dir(id)?;
        if !dir.join(INSTALL_FILE).exists() {
            return Err(AppError::new("package.not_installed").with("id", id));
        }
        std::fs::remove_dir_all(dir)?;
        Ok(())
    }

    /// 已安装插件；损坏的目录被跳过
    pub fn list(&self) -> Vec<InstallInfo> {
        let Ok(entries) = std::fs::read_dir(&self.root) else {
            return Vec::new();
        };
        let mut out: Vec<InstallInfo> = entries
            .filter_map(Result::ok)
            .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
            .filter_map(|e| {
                let raw = std::fs::read(e.path().join(INSTALL_FILE)).ok()?;
                serde_json::from_slice(&raw).ok()
            })
            .collect();
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out
    }

    pub fn manifest(&self, id: &str) -> AppResult<Manifest> {
        let raw = std::fs::read(self.dir(id)?.join("manifest.json"))?;
        serde_json::from_slice(&raw)
            .map_err(|e| AppError::new("package.invalid").with("detail", e.to_string()))
    }

    /// 编译 WASM 组件
    pub fn load(&self, id: &str) -> AppResult<Arc<WasmPlugin>> {
        let manifest = self.manifest(id)?;
        self.check_app_version(&manifest)?;
        let wasm = std::fs::read(self.dir(id)?.join("plugin.wasm"))?;
        Ok(Arc::new(WasmPlugin::load(manifest, &wasm)?))
    }

    /// 供 tfplugin:// 协议读取的界面文件
    pub fn file(&self, id: &str, path: &str) -> AppResult<Vec<u8>> {
        if !servable(path) {
            return Err(AppError::new("fs.not_found").with("path", path));
        }
        Ok(std::fs::read(self.dir(id)?.join(path))?)
    }

    /// 语言包与图标，前端注册到 i18n 与图标表
    pub fn assets(&self, id: &str) -> AppResult<Assets> {
        let dir = self.dir(id)?;
        let mut locales = BTreeMap::new();
        if let Ok(entries) = std::fs::read_dir(dir.join("locales")) {
            for entry in entries.filter_map(Result::ok) {
                let path = entry.path();
                let Some(locale) = path.file_stem().map(|s| s.to_string_lossy().into_owned())
                else {
                    continue;
                };
                if let Some(Value::Object(map)) = std::fs::read(&path)
                    .ok()
                    .and_then(|raw| serde_json::from_slice(&raw).ok())
                {
                    locales.insert(locale, map);
                }
            }
        }
        Ok(Assets {
            locales,
            icon: std::fs::read_to_string(dir.join("icon.svg")).ok(),
            css: dir.join("ui/index.css").exists(),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

#[derive(Debug, Serialize)]
pub struct Assets {
    pub locales: BTreeMap<String, Map<String, Value>>,
    pub icon: Option<String>,
    pub css: bool,
}

#[cfg(test)]
#[path = "installed_test.rs"]
mod tests;
