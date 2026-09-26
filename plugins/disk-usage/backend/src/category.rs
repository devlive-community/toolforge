//! 按扩展名把文件归类，用于类型统计与树图着色。

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
#[repr(u8)]
pub enum Category {
    Images,
    Videos,
    Audio,
    Documents,
    Archives,
    Code,
    Other,
}

pub const ALL: [Category; 7] = [
    Category::Images,
    Category::Videos,
    Category::Audio,
    Category::Documents,
    Category::Archives,
    Category::Code,
    Category::Other,
];

pub fn of(name: &str) -> Category {
    let Some((_, ext)) = name.rsplit_once('.') else {
        return Category::Other;
    };
    match ext.to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "heic" | "heif" | "bmp" | "tif" | "tiff"
        | "svg" | "raw" | "cr2" | "cr3" | "nef" | "arw" | "dng" | "psd" | "ico" | "avif" => {
            Category::Images
        }
        "mp4" | "mov" | "mkv" | "avi" | "wmv" | "m4v" | "webm" | "flv" | "3gp" | "mts" => {
            Category::Videos
        }
        "mp3" | "flac" | "wav" | "aac" | "m4a" | "ogg" | "wma" | "aiff" | "opus" => Category::Audio,
        "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "txt" | "md" | "rtf" | "csv"
        | "pages" | "numbers" | "key" | "epub" | "odt" => Category::Documents,
        "zip" | "rar" | "7z" | "tar" | "gz" | "tgz" | "bz2" | "xz" | "zst" | "dmg" | "iso"
        | "pkg" | "deb" | "rpm" | "msi" | "jar" => Category::Archives,
        "rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "go" | "java" | "kt" | "swift" | "c" | "h"
        | "cpp" | "hpp" | "cs" | "rb" | "php" | "vue" | "json" | "toml" | "yaml" | "yml"
        | "html" | "css" | "scss" | "sql" | "sh" | "o" | "a" | "rlib" | "rmeta" | "class"
        | "wasm" | "map" => Category::Code,
        _ => Category::Other,
    }
}
