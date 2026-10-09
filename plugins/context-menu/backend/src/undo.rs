//! 每次修改前写一个“撤销”用的 .reg 文件：双击即可把这一处改回原样。

use std::path::{Path, PathBuf};

use tf_plugin_api::{PluginError, PluginResult};

use crate::reg::{Hive, Value};

/// 一处修改：在 hive\path 下把 name 从 before 改成 after（None 表示不存在）
pub struct Change<'a> {
    pub hive: Hive,
    pub path: &'a str,
    pub name: &'a str,
    pub before: Option<Value>,
}

fn quote(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn utf16(text: &str) -> Vec<u8> {
    text.encode_utf16()
        .chain(std::iter::once(0))
        .flat_map(u16::to_le_bytes)
        .collect()
}

fn line(name: &str, value: Option<&Value>) -> String {
    let key = if name.is_empty() {
        "@".to_owned()
    } else {
        quote(name)
    };
    let data = match value {
        None => "-".to_owned(),
        Some(Value::Str(s)) => quote(s),
        Some(Value::Expand(s)) => format!("hex(2):{}", hex(&utf16(s))),
        Some(Value::Dword(d)) => format!("dword:{d:08x}"),
        Some(Value::Other(kind, bytes)) => format!("hex({kind:x}):{}", hex(bytes)),
    };
    format!("{key}={data}")
}

/// 生成恢复到修改前状态的 .reg 内容；键原本不存在时删除整个键
pub fn script(changes: &[Change], removed_keys: &[(Hive, &str)]) -> String {
    let mut out = vec![
        "Windows Registry Editor Version 5.00".to_owned(),
        String::new(),
    ];
    for (hive, path) in removed_keys {
        out.push(format!("[-{}\\{}]", hive.root(), path));
        out.push(String::new());
    }
    for change in changes {
        out.push(format!("[{}\\{}]", change.hive.root(), change.path));
        out.push(line(change.name, change.before.as_ref()));
        out.push(String::new());
    }
    out.join("\r\n")
}

/// 写入撤销文件，返回其路径；文件名带时间与说明
pub fn save(dir: &Path, label: &str, content: &str) -> PluginResult<PathBuf> {
    std::fs::create_dir_all(dir)
        .map_err(|e| PluginError::new("ctxmenu.backup_failed").with("detail", e.to_string()))?;
    let stamp = jiff::Zoned::now().strftime("%Y%m%d-%H%M%S").to_string();
    let safe: String = label
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(40)
        .collect();
    let path = dir.join(format!("undo-{stamp}-{safe}.reg"));
    // .reg 文件用 UTF-16 LE 加 BOM，regedit 才能正确读取中文
    let mut bytes = vec![0xff, 0xfe];
    bytes.extend(content.encode_utf16().flat_map(u16::to_le_bytes));
    std::fs::write(&path, bytes)
        .map_err(|e| PluginError::new("ctxmenu.backup_failed").with("detail", e.to_string()))?;
    Ok(path)
}

#[cfg(test)]
#[path = "undo_test.rs"]
mod tests;
