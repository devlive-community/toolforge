//! 右键菜单项的读取与启用 / 禁用。
//!
//! - 静态命令（shell\<动词>）：写入空的 LegacyDisable 值即可隐藏，删除该值恢复。
//! - 外壳扩展（shellex\ContextMenuHandlers\<名称>）：把 CLSID 加入
//!   HKLM\…\Shell Extensions\Blocked 即被资源管理器忽略，删除恢复。
//!
//! 两种方式都不删除原有的键，可随时恢复。

use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

use crate::reg::{Hive, Registry, Value};

const CLASSES: &str = "Software\\Classes";
pub const BLOCKED: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Shell Extensions\\Blocked";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Scope {
    /// 所有文件（* 与 AllFilesystemObjects）
    Files,
    /// 文件夹（Directory 与 Folder）
    Folders,
    /// 文件夹空白处
    Background,
    /// 桌面空白处
    Desktop,
    /// 驱动器
    Drives,
}

impl Scope {
    /// 对应的类名（HKCR 下的键）
    pub fn classes(self) -> &'static [&'static str] {
        match self {
            Scope::Files => &["*", "AllFilesystemObjects"],
            Scope::Folders => &["Directory", "Folder"],
            Scope::Background => &["Directory\\Background"],
            Scope::Desktop => &["DesktopBackground"],
            Scope::Drives => &["Drive"],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    /// shell 下的静态命令
    Command,
    /// shellex\ContextMenuHandlers 下的外壳扩展
    Handler,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    /// 用于启用 / 禁用的定位信息
    pub hive: Hive,
    /// 相对 hive 的完整路径（Software\Classes\…）
    pub path: String,
    pub scope: Scope,
    pub kind: Kind,
    /// 注册表中的键名
    pub key: String,
    /// 菜单上显示的名称（尽量解析出文字）
    pub name: String,
    /// 命令行（静态命令）或 DLL 路径（外壳扩展）
    pub target: Option<String>,
    pub clsid: Option<String>,
    pub enabled: bool,
    /// 只在按住 Shift 时显示
    pub extended: bool,
}

fn is_guid(text: &str) -> bool {
    let t = text.trim();
    t.len() == 38 && t.starts_with('{') && t.ends_with('}')
}

/// 去掉菜单名中的加速键标记（&Open → Open）
fn clean_name(name: &str) -> String {
    name.replace("&&", "\u{0}")
        .replace('&', "")
        .replace('\u{0}', "&")
}

/// 已被阻止的外壳扩展（当前用户与本机两处都算）
fn blocked(reg: &dyn Registry, clsid: &str) -> bool {
    [Hive::Machine, Hive::User]
        .iter()
        .any(|h| reg.value(*h, BLOCKED, clsid).is_some())
}

fn handler_dll(reg: &dyn Registry, clsid: &str) -> Option<String> {
    [Hive::User, Hive::Machine].iter().find_map(|h| {
        reg.value(
            *h,
            &format!("{CLASSES}\\CLSID\\{clsid}\\InprocServer32"),
            "",
        )
        .and_then(|v| v.text().map(str::to_owned))
        .filter(|s| !s.is_empty())
    })
}

fn handler_name(reg: &dyn Registry, clsid: &str, fallback: &str) -> String {
    [Hive::User, Hive::Machine]
        .iter()
        .find_map(|h| {
            reg.value(*h, &format!("{CLASSES}\\CLSID\\{clsid}"), "")
                .and_then(|v| v.text().map(str::to_owned))
                .filter(|s| !s.trim().is_empty())
        })
        .unwrap_or_else(|| fallback.to_owned())
}

/// 列出某个范围内的菜单项；同一键在当前用户与本机都有时以当前用户为准
pub fn list(reg: &dyn Registry, scope: Scope) -> Vec<Entry> {
    let mut out: Vec<Entry> = Vec::new();
    for class in scope.classes() {
        for hive in [Hive::User, Hive::Machine] {
            let base = format!("{CLASSES}\\{class}");
            let shell = format!("{base}\\shell");
            for key in reg.subkeys(hive, &shell) {
                let path = format!("{shell}\\{key}");
                if out.iter().any(|e| {
                    e.kind == Kind::Command
                        && e.key.eq_ignore_ascii_case(&key)
                        && e.path.eq_ignore_ascii_case(&path)
                }) {
                    continue;
                }
                let label = reg
                    .value(hive, &path, "MUIVerb")
                    .or_else(|| reg.value(hive, &path, ""))
                    .and_then(|v| v.text().map(str::to_owned))
                    .filter(|s| !s.trim().is_empty())
                    .map(|s| reg.resolve(&s))
                    .unwrap_or_else(|| key.clone());
                let target = reg
                    .value(hive, &format!("{path}\\command"), "")
                    .and_then(|v| v.text().map(str::to_owned));
                out.push(Entry {
                    hive,
                    path: path.clone(),
                    scope,
                    kind: Kind::Command,
                    key: key.clone(),
                    name: clean_name(&label),
                    target,
                    clsid: None,
                    enabled: reg.value(hive, &path, "LegacyDisable").is_none(),
                    extended: reg.value(hive, &path, "Extended").is_some(),
                });
            }
            let handlers = format!("{base}\\shellex\\ContextMenuHandlers");
            for key in reg.subkeys(hive, &handlers) {
                let path = format!("{handlers}\\{key}");
                let default = reg
                    .value(hive, &path, "")
                    .and_then(|v| v.text().map(str::to_owned))
                    .unwrap_or_default();
                // 有的键名本身就是 CLSID，有的在默认值里；默认值以 - 开头表示被其他工具禁用
                let raw = if is_guid(&key) {
                    key.clone()
                } else {
                    default.clone()
                };
                let disabled_by_dash = raw.starts_with('-');
                let clsid = raw.trim_start_matches('-').trim().to_owned();
                if !is_guid(&clsid) {
                    continue;
                }
                if out
                    .iter()
                    .any(|e| e.clsid.as_deref() == Some(&clsid) && e.scope == scope)
                {
                    continue;
                }
                out.push(Entry {
                    hive,
                    path,
                    scope,
                    kind: Kind::Handler,
                    key: key.clone(),
                    name: handler_name(reg, &clsid, &key),
                    target: handler_dll(reg, &clsid),
                    enabled: !disabled_by_dash && !blocked(reg, &clsid),
                    clsid: Some(clsid),
                    extended: false,
                });
            }
        }
    }
    out
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Target {
    pub hive: Hive,
    pub path: String,
    pub kind: Kind,
    #[serde(default)]
    pub clsid: Option<String>,
}

/// 启用或禁用一个菜单项
pub fn set_enabled(reg: &dyn Registry, target: &Target, enabled: bool) -> PluginResult<()> {
    match target.kind {
        Kind::Command => {
            if !reg.exists(target.hive, &target.path) {
                return Err(PluginError::new("ctxmenu.not_found"));
            }
            if enabled {
                reg.delete_value(target.hive, &target.path, "LegacyDisable")
            } else {
                reg.set(
                    target.hive,
                    &target.path,
                    "LegacyDisable",
                    &Value::Str(String::new()),
                )
            }
        }
        Kind::Handler => {
            let clsid = target
                .clsid
                .as_deref()
                .filter(|c| is_guid(c))
                .ok_or_else(|| PluginError::new("ctxmenu.not_found"))?;
            if enabled {
                // 两处的阻止记录都要删
                for hive in [Hive::User, Hive::Machine] {
                    if reg.value(hive, BLOCKED, clsid).is_some() {
                        reg.delete_value(hive, BLOCKED, clsid)?;
                    }
                }
                Ok(())
            } else {
                reg.set(Hive::Machine, BLOCKED, clsid, &Value::Str(String::new()))
            }
        }
    }
}

/// Windows 11 的经典右键菜单：存在这个空的 InprocServer32 时直接显示旧菜单
pub const CLASSIC: &str = "Software\\Classes\\CLSID\\{86ca1aa0-34aa-4e8b-a509-50c905bae2a2}";

pub fn classic_enabled(reg: &dyn Registry) -> bool {
    reg.exists(Hive::User, &format!("{CLASSIC}\\InprocServer32"))
}

pub fn set_classic(reg: &dyn Registry, enabled: bool) -> PluginResult<()> {
    if enabled {
        reg.set(
            Hive::User,
            &format!("{CLASSIC}\\InprocServer32"),
            "",
            &Value::Str(String::new()),
        )
    } else {
        reg.delete_tree(Hive::User, CLASSIC)
    }
}

#[cfg(test)]
#[path = "menu_test.rs"]
mod tests;
