//! 注册表访问的抽象：Windows 上用真实注册表，测试中用内存实现。

#[cfg(test)]
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Hive {
    /// HKEY_CURRENT_USER
    User,
    /// HKEY_LOCAL_MACHINE（写入需要管理员权限）
    Machine,
}

impl Hive {
    pub fn root(self) -> &'static str {
        match self {
            Hive::User => "HKEY_CURRENT_USER",
            Hive::Machine => "HKEY_LOCAL_MACHINE",
        }
    }
}

// 部分类型只在 Windows 的真实注册表中出现
#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Str(String),
    /// REG_EXPAND_SZ
    Expand(String),
    Dword(u32),
    /// 其他类型（只用于备份）：类型编号与原始字节
    Other(u32, Vec<u8>),
}

impl Value {
    pub fn text(&self) -> Option<&str> {
        match self {
            Value::Str(s) | Value::Expand(s) => Some(s),
            _ => None,
        }
    }
}

/// 名称为空字符串表示默认值
pub trait Registry {
    fn exists(&self, hive: Hive, path: &str) -> bool;
    fn subkeys(&self, hive: Hive, path: &str) -> Vec<String>;
    fn values(&self, hive: Hive, path: &str) -> Vec<(String, Value)>;
    fn set(&self, hive: Hive, path: &str, name: &str, value: &Value) -> PluginResult<()>;
    fn delete_value(&self, hive: Hive, path: &str, name: &str) -> PluginResult<()>;
    fn delete_tree(&self, hive: Hive, path: &str) -> PluginResult<()>;
    /// 把 "@shell32.dll,-8506" 这类间接字符串解析成文字
    fn resolve(&self, text: &str) -> String {
        text.to_owned()
    }

    fn value(&self, hive: Hive, path: &str, name: &str) -> Option<Value> {
        self.values(hive, path)
            .into_iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v)
    }
}

#[cfg_attr(not(any(windows, test)), allow(dead_code))]
pub fn access_denied() -> PluginError {
    PluginError::new("ctxmenu.needs_admin")
}

/// 内存中的注册表（测试用）
#[cfg(test)]
#[derive(Default)]
pub struct Memory {
    pub keys: std::sync::Mutex<BTreeMap<(Hive, String), BTreeMap<String, Value>>>,
    /// 模拟没有管理员权限
    pub read_only_machine: bool,
}

#[cfg(test)]
impl Memory {
    fn norm(path: &str) -> String {
        path.trim_matches('\\').to_ascii_lowercase()
    }

    /// 创建键（含所有父键），不写值
    pub fn key(&self, hive: Hive, path: &str) {
        let mut keys = self.keys.lock().unwrap();
        let parts: Vec<&str> = path.split('\\').collect();
        for i in 1..=parts.len() {
            keys.entry((hive, Self::norm(&parts[..i].join("\\"))))
                .or_default();
        }
    }

    pub fn put(&self, hive: Hive, path: &str, name: &str, value: Value) {
        self.key(hive, path);
        self.keys
            .lock()
            .unwrap()
            .get_mut(&(hive, Self::norm(path)))
            .unwrap()
            .insert(name.to_owned(), value);
    }

    fn check(&self, hive: Hive) -> PluginResult<()> {
        if self.read_only_machine && hive == Hive::Machine {
            return Err(access_denied());
        }
        Ok(())
    }
}

#[cfg(test)]
impl Registry for Memory {
    fn exists(&self, hive: Hive, path: &str) -> bool {
        self.keys
            .lock()
            .unwrap()
            .contains_key(&(hive, Self::norm(path)))
    }

    fn subkeys(&self, hive: Hive, path: &str) -> Vec<String> {
        let prefix = format!("{}\\", Self::norm(path));
        let keys = self.keys.lock().unwrap();
        let mut out: Vec<String> = keys
            .keys()
            .filter(|(h, k)| {
                *h == hive && k.starts_with(&prefix) && !k[prefix.len()..].contains('\\')
            })
            .map(|(_, k)| k[prefix.len()..].to_owned())
            .collect();
        out.dedup();
        out
    }

    fn values(&self, hive: Hive, path: &str) -> Vec<(String, Value)> {
        self.keys
            .lock()
            .unwrap()
            .get(&(hive, Self::norm(path)))
            .map(|v| v.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default()
    }

    fn set(&self, hive: Hive, path: &str, name: &str, value: &Value) -> PluginResult<()> {
        self.check(hive)?;
        self.put(hive, path, name, value.clone());
        Ok(())
    }

    fn delete_value(&self, hive: Hive, path: &str, name: &str) -> PluginResult<()> {
        self.check(hive)?;
        if let Some(values) = self.keys.lock().unwrap().get_mut(&(hive, Self::norm(path))) {
            values.retain(|k, _| !k.eq_ignore_ascii_case(name));
        }
        Ok(())
    }

    fn delete_tree(&self, hive: Hive, path: &str) -> PluginResult<()> {
        self.check(hive)?;
        let norm = Self::norm(path);
        self.keys.lock().unwrap().retain(|(h, k), _| {
            !(*h == hive && (*k == norm || k.starts_with(&format!("{norm}\\"))))
        });
        Ok(())
    }
}
