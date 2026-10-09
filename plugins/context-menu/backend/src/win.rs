//! Windows 注册表的实现。

use std::io;

use tf_plugin_api::{PluginError, PluginResult};
use winreg::RegKey;
use winreg::enums::{
    HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_SET_VALUE, KEY_WRITE, REG_DWORD,
    REG_EXPAND_SZ, REG_SZ,
};
use winreg::types::FromRegValue;

use crate::reg::{Hive, Registry, Value, access_denied};

pub struct Windows;

fn root(hive: Hive) -> RegKey {
    RegKey::predef(match hive {
        Hive::User => HKEY_CURRENT_USER,
        Hive::Machine => HKEY_LOCAL_MACHINE,
    })
}

fn map_err(e: io::Error) -> PluginError {
    // ERROR_ACCESS_DENIED = 5
    if e.kind() == io::ErrorKind::PermissionDenied || e.raw_os_error() == Some(5) {
        access_denied()
    } else {
        PluginError::new("ctxmenu.registry_failed").with("detail", e.to_string())
    }
}

fn ignore_missing(result: io::Result<()>) -> PluginResult<()> {
    match result {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other.map_err(map_err),
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

impl Registry for Windows {
    fn exists(&self, hive: Hive, path: &str) -> bool {
        root(hive).open_subkey_with_flags(path, KEY_READ).is_ok()
    }

    fn subkeys(&self, hive: Hive, path: &str) -> Vec<String> {
        root(hive)
            .open_subkey_with_flags(path, KEY_READ)
            .map(|k| k.enum_keys().flatten().collect())
            .unwrap_or_default()
    }

    fn values(&self, hive: Hive, path: &str) -> Vec<(String, Value)> {
        let Ok(key) = root(hive).open_subkey_with_flags(path, KEY_READ) else {
            return Vec::new();
        };
        key.enum_values()
            .flatten()
            .map(|(name, raw)| {
                let value = match raw.vtype {
                    REG_SZ => String::from_reg_value(&raw)
                        .map(Value::Str)
                        .unwrap_or(Value::Str(String::new())),
                    REG_EXPAND_SZ => String::from_reg_value(&raw)
                        .map(Value::Expand)
                        .unwrap_or(Value::Expand(String::new())),
                    REG_DWORD => u32::from_reg_value(&raw)
                        .map(Value::Dword)
                        .unwrap_or(Value::Dword(0)),
                    other => Value::Other(other as u32, raw.bytes.to_vec()),
                };
                (name, value)
            })
            .collect()
    }

    fn set(&self, hive: Hive, path: &str, name: &str, value: &Value) -> PluginResult<()> {
        let (key, _) = root(hive)
            .create_subkey_with_flags(path, KEY_WRITE)
            .map_err(map_err)?;
        match value {
            Value::Str(s) => key.set_value(name, s),
            Value::Dword(d) => key.set_value(name, d),
            Value::Expand(s) => key.set_raw_value(
                name,
                &winreg::RegValue {
                    bytes: wide(s)
                        .iter()
                        .flat_map(|u| u.to_le_bytes())
                        .collect::<Vec<u8>>(),
                    vtype: REG_EXPAND_SZ,
                },
            ),
            Value::Other(..) => {
                return Err(PluginError::new("ctxmenu.registry_failed")
                    .with("detail", "unsupported value type"));
            }
        }
        .map_err(map_err)
    }

    fn delete_value(&self, hive: Hive, path: &str, name: &str) -> PluginResult<()> {
        match root(hive).open_subkey_with_flags(path, KEY_SET_VALUE) {
            Ok(key) => ignore_missing(key.delete_value(name)),
            Err(e) => ignore_missing(Err(e)),
        }
    }

    fn delete_tree(&self, hive: Hive, path: &str) -> PluginResult<()> {
        ignore_missing(root(hive).delete_subkey_all(path))
    }

    fn resolve(&self, text: &str) -> String {
        if !text.starts_with('@') {
            return text.to_owned();
        }
        let input = wide(text);
        let mut buffer = vec![0u16; 1024];
        // SAFETY: 输入以 0 结尾，输出缓冲区长度与传入的大小一致
        let hr = unsafe {
            windows_sys::Win32::UI::Shell::SHLoadIndirectString(
                input.as_ptr(),
                buffer.as_mut_ptr(),
                buffer.len() as u32,
                std::ptr::null(),
            )
        };
        if hr != 0 {
            return text.to_owned();
        }
        let len = buffer.iter().position(|c| *c == 0).unwrap_or(buffer.len());
        String::from_utf16_lossy(&buffer[..len])
    }
}

/// Windows 版本号（如 22631）
pub fn build_number() -> Option<u32> {
    root(Hive::Machine)
        .open_subkey_with_flags("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion", KEY_READ)
        .ok()?
        .get_value::<String, _>("CurrentBuildNumber")
        .ok()?
        .parse()
        .ok()
}

/// 重启资源管理器，让右键菜单的修改立即生效
pub fn restart_explorer() -> PluginResult<()> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let _ = std::process::Command::new("taskkill")
        .args(["/f", "/im", "explorer.exe"])
        .creation_flags(CREATE_NO_WINDOW)
        .status();
    std::process::Command::new("explorer.exe")
        .spawn()
        .map(|_| ())
        .map_err(|e| PluginError::new("ctxmenu.restart_failed").with("detail", e.to_string()))
}
