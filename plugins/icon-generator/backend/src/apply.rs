//! 把图标设置到文件夹或文件上，或恢复默认图标。
//! macOS 用 NSWorkspace（文件与文件夹）；Windows 写 desktop.ini（仅文件夹）；
//! Linux 用 gio 元数据（GNOME）并写 .directory（KDE），仅文件夹。

use std::path::Path;

use image::RgbaImage;
use tf_plugin_api::{PluginError, PluginResult};

#[cfg_attr(not(windows), allow(dead_code))]
const SECTION: &str = "[.ShellClassInfo]";
/// 写进文件夹里的图标文件名（隐藏）
#[cfg_attr(target_os = "macos", allow(dead_code))]
const ICON_NAME: &str = ".folder-icon";

fn failed(path: &Path, detail: impl ToString) -> PluginError {
    PluginError::new("icon.apply_failed")
        .with("path", path.to_string_lossy().as_ref())
        .with("detail", detail.to_string())
}

#[cfg_attr(target_os = "macos", allow(dead_code))]
fn folder_only(path: &Path) -> PluginResult<()> {
    if path.is_dir() {
        Ok(())
    } else {
        Err(PluginError::new("icon.apply_folder_only")
            .with("path", path.to_string_lossy().as_ref()))
    }
}

/// 在 desktop.ini 的 [.ShellClassInfo] 中设置或去掉 IconResource，保留其他内容
#[cfg_attr(not(windows), allow(dead_code))]
pub fn desktop_ini(existing: &str, icon: Option<&str>) -> String {
    let entry = icon.map(|icon| format!("IconResource={icon},0"));
    let mut lines: Vec<String> = Vec::new();
    let mut in_section = false;
    let mut seen_section = false;
    let mut inserted = false;
    for line in existing.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            // 离开 [.ShellClassInfo] 前补上图标
            if in_section
                && !inserted
                && let Some(entry) = &entry
            {
                lines.push(entry.clone());
                inserted = true;
            }
            in_section = trimmed.eq_ignore_ascii_case(SECTION);
            seen_section |= in_section;
            lines.push(line.to_owned());
            continue;
        }
        let key = trimmed.split('=').next().unwrap_or("").trim();
        let icon_key = ["IconResource", "IconFile", "IconIndex"]
            .iter()
            .any(|k| key.eq_ignore_ascii_case(k));
        if !(in_section && icon_key) {
            lines.push(line.to_owned());
        }
    }
    if let Some(entry) = entry.filter(|_| !inserted) {
        if !seen_section {
            lines.push(SECTION.to_owned());
        }
        lines.push(entry);
    }
    let mut out = lines.join("\r\n");
    out.push_str("\r\n");
    out
}

#[cfg(target_os = "macos")]
pub fn set(target: &Path, master: &RgbaImage) -> PluginResult<()> {
    use objc2::AllocAnyThread;
    use objc2_app_kit::{NSImage, NSWorkspace, NSWorkspaceIconCreationOptions};
    use objc2_foundation::{NSData, NSString};

    if !target.exists() {
        return Err(
            PluginError::new("fs.not_found").with("path", target.to_string_lossy().as_ref())
        );
    }
    let icns = crate::presets::icns(&mut |size| {
        crate::presets::png_bytes(&crate::compose::scaled(master, size))
    });
    let data = NSData::with_bytes(&icns);
    let image =
        NSImage::initWithData(NSImage::alloc(), &data).ok_or_else(|| failed(target, "NSImage"))?;
    let path = NSString::from_str(&target.to_string_lossy());
    let ok = NSWorkspace::sharedWorkspace().setIcon_forFile_options(
        Some(&image),
        &path,
        NSWorkspaceIconCreationOptions(0),
    );
    if ok {
        Ok(())
    } else {
        Err(failed(target, "NSWorkspace"))
    }
}

#[cfg(target_os = "macos")]
pub fn clear(target: &Path) -> PluginResult<()> {
    use objc2_app_kit::{NSWorkspace, NSWorkspaceIconCreationOptions};
    use objc2_foundation::NSString;

    let path = NSString::from_str(&target.to_string_lossy());
    let ok = NSWorkspace::sharedWorkspace().setIcon_forFile_options(
        None,
        &path,
        NSWorkspaceIconCreationOptions(0),
    );
    if ok {
        Ok(())
    } else {
        Err(failed(target, "NSWorkspace"))
    }
}

#[cfg(windows)]
mod win {
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    use windows_sys::Win32::Storage::FileSystem::{
        FILE_ATTRIBUTE_HIDDEN, FILE_ATTRIBUTE_NORMAL, FILE_ATTRIBUTE_READONLY,
        FILE_ATTRIBUTE_SYSTEM, GetFileAttributesW, INVALID_FILE_ATTRIBUTES, SetFileAttributesW,
    };
    use windows_sys::Win32::UI::Shell::{SHCNE_UPDATEITEM, SHCNF_PATHW, SHChangeNotify};

    pub fn wide(path: &Path) -> Vec<u16> {
        path.as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    pub fn attributes(path: &Path) -> u32 {
        unsafe { GetFileAttributesW(wide(path).as_ptr()) }
    }

    pub fn set_attributes(path: &Path, value: u32) -> bool {
        unsafe { SetFileAttributesW(wide(path).as_ptr(), value) != 0 }
    }

    /// 隐藏文件不能直接覆盖写入，先恢复为普通属性
    pub fn unlock(path: &Path) {
        if path.exists() {
            set_attributes(path, FILE_ATTRIBUTE_NORMAL);
        }
    }

    pub fn hide(path: &Path) {
        set_attributes(path, FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM);
    }

    /// 资源管理器只读取带只读或系统属性的文件夹里的 desktop.ini
    pub fn mark_folder(path: &Path, on: bool) {
        let current = attributes(path);
        if current == INVALID_FILE_ATTRIBUTES {
            return;
        }
        let next = if on {
            current | FILE_ATTRIBUTE_READONLY
        } else {
            current & !FILE_ATTRIBUTE_READONLY
        };
        set_attributes(path, next);
    }

    pub fn refresh(path: &Path) {
        let path = wide(path);
        unsafe {
            SHChangeNotify(
                SHCNE_UPDATEITEM as i32,
                SHCNF_PATHW,
                path.as_ptr().cast(),
                std::ptr::null(),
            )
        };
    }

    /// desktop.ini 可能是 UTF-16（带 BOM）或 ANSI
    pub fn read_ini(path: &Path) -> (String, bool) {
        let Ok(bytes) = std::fs::read(path) else {
            return (String::new(), false);
        };
        if let Some(rest) = bytes.strip_prefix(&[0xff, 0xfe]) {
            let (pairs, _) = rest.as_chunks::<2>();
            let units: Vec<u16> = pairs.iter().map(|c| u16::from_le_bytes(*c)).collect();
            (String::from_utf16_lossy(&units), true)
        } else {
            (String::from_utf8_lossy(&bytes).into_owned(), false)
        }
    }

    pub fn write_ini(path: &Path, text: &str, utf16: bool) -> std::io::Result<()> {
        let bytes = if utf16 {
            let mut out = vec![0xff, 0xfe];
            out.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
            out
        } else {
            text.as_bytes().to_vec()
        };
        std::fs::write(path, bytes)
    }
}

#[cfg(windows)]
pub fn set(target: &Path, master: &RgbaImage) -> PluginResult<()> {
    folder_only(target)?;
    let icon_name = format!("{ICON_NAME}.ico");
    let icon = target.join(&icon_name);
    let ini = target.join("desktop.ini");
    let sizes: Vec<RgbaImage> = crate::presets::WINDOWS_SIZES
        .iter()
        .map(|s| crate::compose::scaled(master, *s))
        .collect();
    let bytes = crate::presets::ico(&sizes)?;
    win::unlock(&icon);
    std::fs::write(&icon, bytes).map_err(|e| failed(&icon, e))?;
    win::hide(&icon);
    let (existing, utf16) = win::read_ini(&ini);
    win::unlock(&ini);
    win::write_ini(&ini, &desktop_ini(&existing, Some(&icon_name)), utf16)
        .map_err(|e| failed(&ini, e))?;
    win::hide(&ini);
    win::mark_folder(target, true);
    win::refresh(target);
    Ok(())
}

#[cfg(windows)]
pub fn clear(target: &Path) -> PluginResult<()> {
    folder_only(target)?;
    let icon = target.join(format!("{ICON_NAME}.ico"));
    let ini = target.join("desktop.ini");
    if icon.exists() {
        win::unlock(&icon);
        std::fs::remove_file(&icon).map_err(|e| failed(&icon, e))?;
    }
    if ini.exists() {
        let (existing, utf16) = win::read_ini(&ini);
        let rest = desktop_ini(&existing, None);
        win::unlock(&ini);
        // 只剩空的 [.ShellClassInfo] 时删掉整个文件
        if rest
            .lines()
            .all(|l| l.trim().is_empty() || l.trim().eq_ignore_ascii_case(SECTION))
        {
            std::fs::remove_file(&ini).map_err(|e| failed(&ini, e))?;
            win::mark_folder(target, false);
        } else {
            win::write_ini(&ini, &rest, utf16).map_err(|e| failed(&ini, e))?;
            win::hide(&ini);
        }
    }
    win::refresh(target);
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn gio(args: &[&str]) -> bool {
    std::process::Command::new("gio")
        .args(args)
        .output()
        .is_ok_and(|o| o.status.success())
}

#[cfg(all(unix, not(target_os = "macos")))]
pub fn set(target: &Path, master: &RgbaImage) -> PluginResult<()> {
    folder_only(target)?;
    let icon = target.join(format!("{ICON_NAME}.png"));
    std::fs::write(
        &icon,
        crate::presets::png_bytes(&crate::compose::scaled(master, 512)),
    )
    .map_err(|e| failed(&icon, e))?;
    let uri = format!("file://{}", icon.to_string_lossy());
    gio(&[
        "set",
        "-t",
        "string",
        &target.to_string_lossy(),
        "metadata::custom-icon",
        &uri,
    ]);
    let directory = target.join(".directory");
    let existing = std::fs::read_to_string(&directory).unwrap_or_default();
    if existing.is_empty() {
        let text = format!("[Desktop Entry]\nIcon=./{ICON_NAME}.png\n");
        std::fs::write(&directory, text).map_err(|e| failed(&directory, e))?;
    }
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
pub fn clear(target: &Path) -> PluginResult<()> {
    folder_only(target)?;
    gio(&[
        "set",
        "-t",
        "unset",
        &target.to_string_lossy(),
        "metadata::custom-icon",
    ]);
    let icon = target.join(format!("{ICON_NAME}.png"));
    if icon.exists() {
        std::fs::remove_file(&icon).map_err(|e| failed(&icon, e))?;
    }
    let directory = target.join(".directory");
    if std::fs::read_to_string(&directory).is_ok_and(|t| t.contains(ICON_NAME)) {
        std::fs::remove_file(&directory).map_err(|e| failed(&directory, e))?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "apply_test.rs"]
mod tests;
