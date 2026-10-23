//! AI Agent 授权判断。插件作者在 manifest 的接口声明里给出上限（开放给 agent 的函数、副作用、
//! 用到的权限、是否每次确认），用户在 ToolForge 设置里授权，结果不会超出声明。
//!
//! - 总开关关闭（默认）、插件或函数被用户关闭 → 拒绝；插件默认开启
//! - 函数没有单独设置时：只读函数允许，写入与对外操作每次确认
//! - 声明了 confirm、声明的权限比授权时多、或原生内置插件用到文件权限（不在沙箱里，
//!   无法限制它直接读写的路径）→ 最多到每次确认
//! - 路径参数必须位于授权目录内（解析符号链接与 `..` 之后判断），写入需要可写目录

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_core::store::PLUGIN_SCOPE;
use tf_core::{AgentPath, AppError, AppResult, InterfaceGrant, Store};
use tf_plugin_api::interface::{Effect, Interface};

/// 总开关在 kv 中的键
pub const ENABLED_KEY: &str = "agent.enabled";
/// 授权表中的通道名
pub const CHANNEL: &str = "agent";
const FS_PERMISSION: &str = "fs:user-selected";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Off,
    Ask,
    Allow,
}

impl Mode {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "off" => Some(Self::Off),
            "ask" => Some(Self::Ask),
            "allow" => Some(Self::Allow),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Ask => "ask",
            Self::Allow => "allow",
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum Decision {
    Allow,
    /// 需要用户确认后才能调用
    Ask,
    Deny(AppError),
}

/// 授权目录（已规范化）
#[derive(Debug, Clone, PartialEq)]
pub struct Root {
    pub path: PathBuf,
    pub write: bool,
}

pub struct Policy {
    enabled: bool,
    grants: Vec<InterfaceGrant>,
    roots: Vec<Root>,
}

pub use tf_wasm::normalize_path as normalize;

impl Policy {
    pub fn new(enabled: bool, grants: Vec<InterfaceGrant>, paths: &[AgentPath]) -> Self {
        let roots = paths
            .iter()
            .filter_map(|p| {
                Some(Root {
                    path: Path::new(&p.path).canonicalize().ok()?,
                    write: p.access == "write",
                })
            })
            .collect();
        Self {
            enabled,
            grants,
            roots,
        }
    }

    pub fn load(store: &Store) -> AppResult<Self> {
        let enabled = store
            .kv_get(ENABLED_KEY)?
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        Ok(Self::new(
            enabled,
            store.interface_grants(CHANNEL)?,
            &store.agent_paths()?,
        ))
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    fn grant(&self, plugin_id: &str, function: &str) -> Option<&InterfaceGrant> {
        self.grants
            .iter()
            .find(|g| g.plugin_id == plugin_id && g.function == function)
    }

    /// 插件默认开启，用户关闭后为 false
    pub fn plugin_enabled(&self, plugin_id: &str) -> bool {
        self.grant(plugin_id, PLUGIN_SCOPE)
            .is_none_or(|g| Mode::parse(&g.mode) != Some(Mode::Off))
    }

    /// 函数的有效模式；native 表示原生内置插件（不在 WASM 沙箱中）
    pub fn function_mode(
        &self,
        plugin_id: &str,
        function: &str,
        interface: &Interface,
        native: bool,
    ) -> Mode {
        let mode = match self.grant(plugin_id, function) {
            Some(grant) => {
                let mode = Mode::parse(&grant.mode).unwrap_or(Mode::Off);
                let granted = grant.uses.as_deref().unwrap_or_default();
                let widened = interface.uses.iter().any(|u| !granted.contains(u));
                if mode == Mode::Allow && widened {
                    Mode::Ask
                } else {
                    mode
                }
            }
            None if interface.effect == Effect::Read => Mode::Allow,
            None => Mode::Ask,
        };
        let capped =
            interface.confirm || (native && interface.uses.iter().any(|u| u == FS_PERMISSION));
        if mode == Mode::Allow && capped {
            Mode::Ask
        } else {
            mode
        }
    }

    /// 路径是否在授权目录内；写入需要可写目录
    pub fn path_allowed(&self, path: &Path, write: bool) -> bool {
        let Some(target) = normalize(path) else {
            return false;
        };
        self.roots
            .iter()
            .any(|root| target.starts_with(&root.path) && (root.write || !write))
    }

    pub fn roots(&self) -> &[Root] {
        &self.roots
    }

    /// WASM 插件的文件访问范围：只允许授权目录
    pub fn file_scope(&self) -> tf_wasm::FileScope {
        let pick = |write: bool| {
            self.roots
                .iter()
                .filter(|r| r.write == write)
                .map(|r| r.path.clone())
                .collect()
        };
        tf_wasm::FileScope::Roots {
            read: pick(false),
            write: pick(true),
        }
    }

    /// 判断一次调用；paths 为声明为路径的参数（路径，是否写入）
    pub fn decide(
        &self,
        plugin_id: &str,
        function: &str,
        interface: &Interface,
        native: bool,
        paths: &[(PathBuf, bool)],
    ) -> Decision {
        if !self.enabled {
            return Decision::Deny(AppError::new("agent.disabled"));
        }
        if !self.plugin_enabled(plugin_id) {
            return Decision::Deny(AppError::new("agent.plugin_off").with("plugin", plugin_id));
        }
        let mode = self.function_mode(plugin_id, function, interface, native);
        if mode == Mode::Off {
            return Decision::Deny(
                AppError::new("agent.function_off")
                    .with("plugin", plugin_id)
                    .with("function", function),
            );
        }
        if let Some((path, write)) = paths.iter().find(|(p, w)| !self.path_allowed(p, *w)) {
            return Decision::Deny(
                AppError::new("agent.path_denied")
                    .with("path", path.to_string_lossy().into_owned())
                    .with("access", if *write { "write" } else { "read" }),
            );
        }
        match mode {
            Mode::Allow => Decision::Allow,
            _ => Decision::Ask,
        }
    }
}

/// 把标记为 x-sensitive 的参数替换为 `***`，用于历史记录与日志
pub fn mask_sensitive(params: &Value, args: &Value) -> Value {
    let mut masked = args.clone();
    if let (Some(props), Some(map)) = (params["properties"].as_object(), masked.as_object_mut()) {
        for (name, schema) in props {
            if schema["x-sensitive"] == true
                && let Some(value) = map.get_mut(name)
            {
                *value = Value::String("***".to_owned());
            }
        }
    }
    masked
}

#[cfg(test)]
#[path = "agent_test.rs"]
mod tests;
