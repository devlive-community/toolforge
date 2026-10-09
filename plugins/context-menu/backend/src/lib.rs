//! Windows 右键菜单管理插件后端：列出文件、文件夹、空白处、桌面与驱动器的右键菜单项，
//! 用 Windows 自带的机制禁用 / 启用（LegacyDisable 与 Shell Extensions\Blocked，不删除任何键），
//! 每次修改前写一个可双击恢复的 .reg 撤销文件；在 Windows 11 上可切换经典右键菜单。

mod menu;
mod reg;
mod undo;
#[cfg(windows)]
mod win;

use std::path::PathBuf;

use serde::Deserialize;
use serde_json::{Value as Json, json};
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

use menu::{Kind, Scope, Target};
use reg::{Hive, Registry};

const MANIFEST: &str = include_str!("../../manifest.json");

#[derive(Deserialize)]
struct ListArgs {
    scope: Scope,
}

#[derive(Deserialize)]
struct SetArgs {
    #[serde(flatten)]
    target: Target,
    enabled: bool,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Deserialize)]
struct ClassicArgs {
    enabled: bool,
}

/// 撤销文件所在的文件夹
fn undo_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("ToolForge")
        .join("context-menu")
}

#[cfg(windows)]
fn registry() -> PluginResult<win::Windows> {
    Ok(win::Windows)
}

#[cfg(not(windows))]
fn registry() -> PluginResult<NoRegistry> {
    Err(PluginError::new("ctxmenu.windows_only"))
}

/// 非 Windows 平台的占位类型（插件在这些平台上不会被注册）
#[cfg(not(windows))]
struct NoRegistry;

#[cfg(not(windows))]
impl Registry for NoRegistry {
    fn exists(&self, _: Hive, _: &str) -> bool {
        false
    }
    fn subkeys(&self, _: Hive, _: &str) -> Vec<String> {
        Vec::new()
    }
    fn values(&self, _: Hive, _: &str) -> Vec<(String, reg::Value)> {
        Vec::new()
    }
    fn set(&self, _: Hive, _: &str, _: &str, _: &reg::Value) -> PluginResult<()> {
        Err(PluginError::new("ctxmenu.windows_only"))
    }
    fn delete_value(&self, _: Hive, _: &str, _: &str) -> PluginResult<()> {
        Err(PluginError::new("ctxmenu.windows_only"))
    }
    fn delete_tree(&self, _: Hive, _: &str) -> PluginResult<()> {
        Err(PluginError::new("ctxmenu.windows_only"))
    }
}

/// 启用 / 禁用一项：先写撤销文件，再修改
fn set_entry(reg: &dyn Registry, args: &SetArgs, dir: &std::path::Path) -> PluginResult<PathBuf> {
    let (hive, path, name) = match args.target.kind {
        Kind::Command => (
            args.target.hive,
            args.target.path.clone(),
            "LegacyDisable".to_owned(),
        ),
        Kind::Handler => {
            let clsid = args
                .target
                .clsid
                .clone()
                .ok_or_else(|| PluginError::new("ctxmenu.not_found"))?;
            (Hive::Machine, menu::BLOCKED.to_owned(), clsid)
        }
    };
    let mut changes = vec![undo::Change {
        hive,
        path: &path,
        name: &name,
        before: reg.value(hive, &path, &name),
    }];
    // 启用外壳扩展时，当前用户下的阻止记录也会被删除
    let user_block = (args.target.kind == Kind::Handler)
        .then(|| reg.value(Hive::User, menu::BLOCKED, &name))
        .flatten();
    if args.enabled && user_block.is_some() {
        changes.push(undo::Change {
            hive: Hive::User,
            path: menu::BLOCKED,
            name: &name,
            before: user_block,
        });
    }
    let script = undo::script(&changes, &[]);
    let label = args.name.clone().unwrap_or_else(|| name.clone());
    let file = undo::save(dir, &label, &script)?;
    if let Err(e) = menu::set_enabled(reg, &args.target, args.enabled) {
        let _ = std::fs::remove_file(&file);
        return Err(e);
    }
    Ok(file)
}

fn set_classic(reg: &dyn Registry, enabled: bool, dir: &std::path::Path) -> PluginResult<PathBuf> {
    let server = format!("{}\\InprocServer32", menu::CLASSIC);
    let script = if enabled {
        undo::script(&[], &[(Hive::User, menu::CLASSIC)])
    } else {
        undo::script(
            &[undo::Change {
                hive: Hive::User,
                path: &server,
                name: "",
                before: Some(reg::Value::Str(String::new())),
            }],
            &[],
        )
    };
    let file = undo::save(dir, "windows11-classic-menu", &script)?;
    if let Err(e) = menu::set_classic(reg, enabled) {
        let _ = std::fs::remove_file(&file);
        return Err(e);
    }
    Ok(file)
}

pub struct ContextMenu {
    manifest: Manifest,
}

impl Default for ContextMenu {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for ContextMenu {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Json) -> PluginResult<Json> {
        let reg = registry()?;
        match function {
            "list" => {
                let args: ListArgs = parse_args(args)?;
                to_value(menu::list(&reg, args.scope))
            }
            "set" => {
                let args: SetArgs = parse_args(args)?;
                let file = set_entry(&reg, &args, &undo_dir())?;
                Ok(json!({ "undo": file.to_string_lossy() }))
            }
            "classic" => {
                #[cfg(windows)]
                let supported = win::build_number().is_some_and(|b| b >= 22000);
                #[cfg(not(windows))]
                let supported = false;
                Ok(json!({ "supported": supported, "enabled": menu::classic_enabled(&reg) }))
            }
            "set_classic" => {
                let args: ClassicArgs = parse_args(args)?;
                let file = set_classic(&reg, args.enabled, &undo_dir())?;
                Ok(json!({ "undo": file.to_string_lossy() }))
            }
            "restart_explorer" => {
                #[cfg(windows)]
                win::restart_explorer()?;
                Ok(json!({}))
            }
            "undo_dir" => Ok(json!({ "path": undo_dir().to_string_lossy() })),
            other => Err(unknown_function(other)),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
