//! 可从命令行调用的插件：编进 tfp 的内置插件，加上在线安装到应用数据目录的插件。
//! 插件按名字（id 的最后一段）查找；与内置插件同名的在线插件被忽略，和桌面端一致。

use std::sync::Arc;

use serde_json::Value;
use tf_core::{AppError, AppResult};
use tf_host::installed::Installed;
use tf_plugin_api::interface::{Channel, Interface};
use tf_plugin_api::{FunctionSpec, Manifest, ToolPlugin};
use tf_wasm::WasmPlugin;

use crate::builtin::{self, Builtin};

/// 插件名：id 的最后一段（org.devlive.toolforge.hash → hash）
pub fn slug(id: &str) -> &str {
    id.rsplit('.').next().unwrap_or(id)
}

/// 对某个通道开放的函数
pub fn functions_for(
    manifest: &Manifest,
    channel: Channel,
) -> Vec<(&String, &FunctionSpec, Interface)> {
    manifest
        .functions
        .iter()
        .filter_map(|(name, spec)| {
            let declaration = spec.declaration();
            let interface = declaration.exposed(channel)?.clone();
            Some((name, spec, interface))
        })
        .collect()
}

/// 对命令行开放的函数
pub fn cli_functions(manifest: &Manifest) -> Vec<(&String, &FunctionSpec, Interface)> {
    functions_for(manifest, Channel::Cli)
}

/// 选语言包：当前语言 → en-US → 第一个
fn pick(locales: Vec<(&str, Value)>, lang: &str) -> Value {
    locales
        .iter()
        .find(|(l, _)| *l == lang)
        .or_else(|| locales.iter().find(|(l, _)| *l == "en-US"))
        .or_else(|| locales.first())
        .map(|(_, v)| v.clone())
        .unwrap_or_default()
}

pub struct Catalog {
    builtins: Vec<Builtin>,
    installed: Option<Installed>,
}

pub struct Loaded {
    pub plugin: Arc<dyn ToolPlugin>,
    /// 在线插件的 WASM 实例，用于限制文件访问范围；内置插件为 None
    pub wasm: Option<Arc<WasmPlugin>>,
    /// 当前语言的语言包
    pub locale: Value,
}

impl Catalog {
    pub fn open() -> Self {
        let installed = tf_host::app_data_dir()
            .map(|dir| Installed::new(dir.join("installed"), env!("CARGO_PKG_VERSION")));
        Self::new(builtin::all(), installed)
    }

    pub fn new(builtins: Vec<Builtin>, installed: Option<Installed>) -> Self {
        Self {
            builtins,
            installed,
        }
    }

    fn builtin(&self, name: &str) -> Option<(&Builtin, Arc<dyn ToolPlugin>)> {
        self.builtins.iter().find_map(|b| {
            let plugin = (b.create)();
            (slug(&plugin.manifest().id) == name).then_some((b, plugin))
        })
    }

    fn builtin_locale(builtin: &Builtin, lang: &str) -> Value {
        pick(
            builtin
                .locales
                .iter()
                .map(|(l, raw)| (*l, serde_json::from_str(raw).unwrap_or_default()))
                .collect(),
            lang,
        )
    }

    fn installed_locale(installed: &Installed, id: &str, lang: &str) -> Value {
        let Ok(assets) = installed.assets(id) else {
            return Value::Null;
        };
        pick(
            assets
                .locales
                .iter()
                .map(|(l, v)| (l.as_str(), Value::Object(v.clone())))
                .collect(),
            lang,
        )
    }

    /// 是否为编进 tfp 的内置插件（原生代码，不在 WASM 沙箱中）
    pub fn is_builtin(&self, name: &str) -> bool {
        self.builtin(name).is_some()
    }

    /// 对某个通道开放了函数的插件 manifest 与当前语言的语言包，按名字排序；不编译 WASM
    pub fn manifests(&self, lang: &str, channel: Channel) -> Vec<(Manifest, Value)> {
        let mut out: Vec<(Manifest, Value)> = self
            .builtins
            .iter()
            .map(|b| {
                (
                    (b.create)().manifest().clone(),
                    Self::builtin_locale(b, lang),
                )
            })
            .collect();
        if let Some(installed) = &self.installed {
            for info in installed.list() {
                if out.iter().any(|(m, _)| slug(&m.id) == slug(&info.id)) {
                    continue;
                }
                if let Ok(manifest) = installed.manifest(&info.id) {
                    out.push((manifest, Self::installed_locale(installed, &info.id, lang)));
                }
            }
        }
        out.retain(|(m, _)| !functions_for(m, channel).is_empty());
        out.sort_by(|(a, _), (b, _)| slug(&a.id).cmp(slug(&b.id)));
        out
    }

    /// 按名字加载插件与语言包；在线插件在这里才编译 WASM 组件
    pub fn load(&self, name: &str, lang: &str) -> AppResult<Loaded> {
        if let Some((builtin, plugin)) = self.builtin(name) {
            return Ok(Loaded {
                plugin,
                wasm: None,
                locale: Self::builtin_locale(builtin, lang),
            });
        }
        let unknown = || AppError::new("cli.unknown_plugin").with("name", name);
        let installed = self.installed.as_ref().ok_or_else(unknown)?;
        let info = installed
            .list()
            .into_iter()
            .find(|info| slug(&info.id) == name)
            .ok_or_else(unknown)?;
        let wasm = installed.load(&info.id)?;
        Ok(Loaded {
            plugin: wasm.clone(),
            wasm: Some(wasm),
            locale: Self::installed_locale(installed, &info.id, lang),
        })
    }
}

#[cfg(test)]
#[path = "catalog_test.rs"]
mod tests;
