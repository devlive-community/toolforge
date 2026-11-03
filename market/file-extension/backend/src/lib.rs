//! 文件后缀百科插件后端：常见后缀的格式说明、MIME 与安全提示；
//! 按魔数识别用户选择文件的真实格式（改名的可执行文件一眼识破）。
//!
//! 文件读取走宿主 `fs:user-selected`（路径必须来自用户选择），只读头部 64 字节。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};
use tf_plugin_guest::net;

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct FileExtension {
    manifest: Manifest,
}

impl Default for FileExtension {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for FileExtension {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "list" => to_value(list(parse_args(args)?)?),
            "identify" => to_value(identify(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(FileExtension);

/// 后缀条目：extension、类别键、MIME、风险键（safe/office/script/executable/…）。
/// 描述键由前端按 `descriptions.desc_<extension 点号换下划线>` 拼出，数据不重复存储
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub extension: &'static str,
    /// categories.<key>
    pub category: &'static str,
    pub mime: &'static str,
    /// risks.<key>
    pub risk: &'static str,
}

macro_rules! entries {
    ($(($ext:literal, $cat:ident, $mime:literal, $risk:ident)),+ $(,)?) => {
        &[ $( Entry {
            extension: $ext,
            category: stringify!($cat),
            mime: $mime,
            risk: stringify!($risk),
        } ),+ ]
    };
}

static ENTRIES: &[Entry] = entries![
    (
        ".docx",
        document,
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        office
    ),
    (".doc", document, "application/msword", office),
    (
        ".xlsx",
        document,
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        office
    ),
    (".xls", document, "application/vnd.ms-excel", office),
    (
        ".pptx",
        document,
        "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        office
    ),
    (".pdf", document, "application/pdf", safe),
    (".md", document, "text/markdown", safe),
    (".txt", document, "text/plain", safe),
    (".csv", document, "text/csv", safe),
    (".rtf", document, "application/rtf", safe),
    (
        ".exe",
        executable,
        "application/vnd.microsoft.portable-executable",
        executable
    ),
    (".msi", executable, "application/x-msi", executable),
    (
        ".dll",
        executable,
        "application/vnd.microsoft.portable-executable",
        executable
    ),
    (
        ".apk",
        executable,
        "application/vnd.android.package-archive",
        executable
    ),
    (".dmg", disk, "application/x-apple-diskimage", safe),
    (
        ".deb",
        archive,
        "application/vnd.debian.binary-package",
        safe
    ),
    (".rpm", archive, "application/x-rpm", safe),
    (
        ".appimage",
        executable,
        "application/x-executable",
        executable
    ),
    (".bat", script, "application/x-bat", script),
    (".ps1", script, "text/x-powershell", script),
    (".sh", script, "application/x-sh", script),
    (".js", script, "text/javascript", script),
    (".vbs", script, "text/x-vbscript", script),
    (".scr", executable, "application/octet-stream", executable),
    (".zip", archive, "application/zip", safe),
    (".7z", archive, "application/x-7z-compressed", safe),
    (".rar", archive, "application/vnd.rar", safe),
    (".gz", archive, "application/gzip", safe),
    (".tar", archive, "application/x-tar", safe),
    (".xz", archive, "application/x-xz", safe),
    (".png", image, "image/png", safe),
    (".jpg", image, "image/jpeg", safe),
    (".jpeg", image, "image/jpeg", safe),
    (".gif", image, "image/gif", safe),
    (".webp", image, "image/webp", safe),
    (".svg", image, "image/svg+xml", script),
    (".ico", image, "image/x-icon", safe),
    (".bmp", image, "image/bmp", safe),
    (".mp3", audio, "audio/mpeg", safe),
    (".wav", audio, "audio/wav", safe),
    (".flac", audio, "audio/flac", safe),
    (".ogg", audio, "audio/ogg", safe),
    (".mp4", video, "video/mp4", safe),
    (".mkv", video, "video/x-matroska", safe),
    (".mov", video, "video/quicktime", safe),
    (".avi", video, "video/x-msvideo", safe),
    (".html", web, "text/html", script),
    (".css", web, "text/css", safe),
    (".json", web, "application/json", safe),
    (".xml", web, "application/xml", safe),
    (".iso", disk, "application/x-iso9660-image", safe),
    (".db", data, "application/x-sqlite3", safe),
    (".sqlite", data, "application/x-sqlite3", safe),
    (".so", executable, "application/x-sharedlib", executable),
    (
        ".dylib",
        executable,
        "application/x-mach-binary",
        executable
    ),
    (".jar", executable, "application/java-archive", script),
    (
        ".class",
        executable,
        "application/x-java-applet",
        executable
    ),
    (".wasm", data, "application/wasm", safe),
    (".lnk", executable, "application/x-ms-shortcut", executable),
];

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ListArgs {
    /// 按后缀或类别过滤（包含匹配，大小写不敏感）
    #[serde(default)]
    pub query: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListOutput {
    pub entries: Vec<Entry>,
    pub total: usize,
}

pub fn list(args: ListArgs) -> PluginResult<ListOutput> {
    let needle = args.query.trim().trim_start_matches('.').to_lowercase();
    let entries: Vec<Entry> = ENTRIES
        .iter()
        .filter(|e| {
            needle.is_empty()
                || e.extension.to_lowercase().contains(&needle)
                || e.category.contains(&needle)
        })
        .cloned()
        .collect();
    if entries.is_empty() && !needle.is_empty() {
        return Err(PluginError::new("extension.not_found").with("query", args.query.trim()));
    }
    let total = entries.len();
    Ok(ListOutput { entries, total })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentifyArgs {
    /// 用户通过系统对话框选择的文件路径
    pub path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentifyOutput {
    /// 名义后缀（小写，含点；无后缀为空串）
    pub nominal: String,
    /// 识别出的真实格式（magic 键），未识别为 None
    pub real: Option<&'static str>,
    /// magic.<real> 的中文/英文说明由前端翻译
    pub magic_hex: String,
    /// 名义后缀与真实格式是否一致（无名义后缀或未识别时不做判断）
    pub matches_nominal: Option<bool>,
    /// 风险提示键（识别结果对应后缀的 risk；未识别为 None）
    pub risk: Option<&'static str>,
}

/// (名称, 偏移, 魔数字节)
const MAGICS: &[(&str, usize, &[u8])] = &[
    ("elf", 0, b"\x7fELF"),
    ("pe_exe", 0, b"MZ"),
    ("zip", 0, b"PK\x03\x04"),
    ("gzip", 0, b"\x1f\x8b"),
    ("bzip2", 0, b"BZh"),
    ("xz", 0, b"\xfd7zXZ\x00"),
    ("sevenzip", 0, b"7z\xbc\xaf\x27\x1c"),
    ("rar", 0, b"Rar!"),
    ("png", 0, b"\x89PNG\r\n\x1a\n"),
    ("jpeg", 0, b"\xff\xd8\xff"),
    ("gif", 0, b"GIF87a"),
    ("bmp", 0, b"BM"),
    ("pdf", 0, b"%PDF"),
    ("sqlite", 0, b"SQLite format 3\x00"),
    ("java_class", 0, b"\xca\xfe\xba\xbe"),
    ("wasm", 0, b"\x00asm"),
    ("ogg", 0, b"OggS"),
    ("flac", 0, b"fLaC"),
    ("mp3_id3", 0, b"ID3"),
    ("ole2_doc", 0, b"\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1"),
    ("windows_lnk", 0, b"\x4c\x00\x00\x00"),
    ("webp_riff", 0, b"RIFF"),
    ("macho_64", 0, b"\xcf\xfa\xed\xfe"),
    ("macho", 0, b"\xce\xfa\xed\xfe"),
    ("dmg", 0, b"koly"),
    ("iso", 32769, b"CD001"),
    ("tar", 257, b"ustar"),
];

pub fn identify(args: IdentifyArgs) -> PluginResult<IdentifyOutput> {
    // 只读头部 64 字节（tar 的魔数在偏移 257，单独补读一次）
    let mut head = match net::read_file(&args.path, 0, 64) {
        Ok(bytes) => bytes,
        Err(err) if err.code == "fs.not_found" => return Err(err),
        Err(err) => return Err(err),
    };
    let nominal = std::path::Path::new(&args.path)
        .extension()
        .map(|ext| format!(".{}", ext.to_string_lossy().to_lowercase()))
        .unwrap_or_default();
    let mut real: Option<&'static str> = None;
    for (name, offset, magic) in MAGICS {
        let bytes = if *offset == 0 {
            head.as_slice()
        } else {
            // 偏移魔数（tar/iso）：补读到偏移处
            head = net::read_file(&args.path, *offset as u64, 64).unwrap_or_default();
            head.as_slice()
        };
        if !bytes.starts_with(magic) {
            continue;
        }
        // RIFF 容器（WEBP/WAV/AVI）需要子标记确认，首版只认 WEBP
        if *name == "webp_riff" && head.get(8..12) != Some(b"WEBP") {
            continue;
        }
        real = Some(name);
        break;
    }
    let magic_hex = head
        .iter()
        .take(16)
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ");
    let matches_nominal = real.map(|real_key| {
        let real_ext = match real_key {
            "pe_exe" => ".exe",
            "zip" => ".zip",
            "png" => ".png",
            "jpeg" => ".jpg",
            "pdf" => ".pdf",
            "sqlite" => ".db",
            "elf" => ".so",
            _ => "",
        };
        if real_ext.is_empty() {
            return true; // 无对应扩展名可比较，不误报
        }
        nominal == real_ext
            || (real_key == "pe_exe"
                && [".exe", ".dll", ".scr", ".msi"].contains(&nominal.as_str()))
            || (real_key == "zip"
                && [".zip", ".docx", ".xlsx", ".pptx", ".jar", ".apk"].contains(&nominal.as_str()))
            || (real_key == "jpeg" && [".jpg", ".jpeg"].contains(&nominal.as_str()))
            || (real_key == "sqlite" && [".db", ".sqlite"].contains(&nominal.as_str()))
    });
    let risk = real.and_then(|key| {
        let ext = match key {
            "pe_exe" => ".exe",
            "zip" => ".zip",
            "png" => ".png",
            "jpeg" => ".jpg",
            "pdf" => ".pdf",
            "sqlite" => ".db",
            "elf" => ".so",
            "gzip" => ".gz",
            "bzip2" => ".bz2",
            "xz" => ".xz",
            "sevenzip" => ".7z",
            "rar" => ".rar",
            "gif" => ".gif",
            "bmp" => ".bmp",
            "java_class" => ".class",
            "wasm" => ".wasm",
            "ogg" => ".ogg",
            "flac" => ".flac",
            "mp3_id3" => ".mp3",
            "ole2_doc" => ".doc",
            "windows_lnk" => ".lnk",
            "macho_64" | "macho" => ".dylib",
            "dmg" => ".dmg",
            "iso" => ".iso",
            "tar" => ".tar",
            _ => "",
        };
        ENTRIES.iter().find(|e| e.extension == ext).map(|e| e.risk)
    });
    Ok(IdentifyOutput {
        nominal,
        real,
        magic_hex,
        matches_nominal,
        risk,
    })
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
