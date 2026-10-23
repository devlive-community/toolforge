//! 按文件头识别常见格式。

/// (偏移, 特征字节, 名称)
const SIGNATURES: &[(usize, &[u8], &str)] = &[
    (0, b"\x89PNG\r\n\x1a\n", "PNG image"),
    (0, b"\xff\xd8\xff", "JPEG image"),
    (0, b"GIF87a", "GIF image"),
    (0, b"GIF89a", "GIF image"),
    (0, b"BM", "BMP image"),
    (0, b"II*\0", "TIFF image"),
    (0, b"MM\0*", "TIFF image"),
    (0, b"\0\0\x01\0", "ICO icon"),
    (0, b"icns", "ICNS icon"),
    (0, b"%PDF-", "PDF document"),
    (0, b"PK\x03\x04", "ZIP archive (also docx, xlsx, jar, apk)"),
    (0, b"PK\x05\x06", "ZIP archive (empty)"),
    (0, b"7z\xbc\xaf\x27\x1c", "7-Zip archive"),
    (0, b"Rar!\x1a\x07", "RAR archive"),
    (0, b"\x1f\x8b", "gzip"),
    (0, b"BZh", "bzip2"),
    (0, b"\xfd7zXZ\0", "xz"),
    (0, b"\x28\xb5\x2f\xfd", "Zstandard"),
    (257, b"ustar", "tar archive"),
    (0, b"\x7fELF", "ELF executable"),
    (0, b"MZ", "Windows executable (PE)"),
    (0, b"\xcf\xfa\xed\xfe", "Mach-O 64-bit"),
    (0, b"\xce\xfa\xed\xfe", "Mach-O 32-bit"),
    (0, b"\xca\xfe\xba\xbe", "Mach-O universal / Java class"),
    (0, b"\0asm", "WebAssembly"),
    (0, b"SQLite format 3\0", "SQLite database"),
    (0, b"OggS", "Ogg"),
    (0, b"fLaC", "FLAC audio"),
    (0, b"ID3", "MP3 audio"),
    (0, b"\x1aE\xdf\xa3", "Matroska / WebM"),
    (4, b"ftyp", "MP4 / QuickTime"),
    (0, b"wOFF", "WOFF font"),
    (0, b"wOF2", "WOFF2 font"),
    (0, b"\0\x01\0\0", "TrueType font"),
    (0, b"OTTO", "OpenType font"),
    (0, b"\xef\xbb\xbf", "UTF-8 text with BOM"),
    (0, b"\xff\xfe", "UTF-16 LE text"),
    (0, b"\xfe\xff", "UTF-16 BE text"),
];

/// RIFF 容器的具体类型
fn riff(head: &[u8]) -> Option<&'static str> {
    if !head.starts_with(b"RIFF") || head.len() < 12 {
        return None;
    }
    Some(match &head[8..12] {
        b"WEBP" => "WebP image",
        b"WAVE" => "WAV audio",
        b"AVI " => "AVI video",
        _ => "RIFF container",
    })
}

pub fn detect(head: &[u8]) -> Option<&'static str> {
    riff(head).or_else(|| {
        SIGNATURES
            .iter()
            .find(|(at, sig, _)| head.get(*at..at + sig.len()) == Some(*sig))
            .map(|(_, _, name)| *name)
    })
}

#[cfg(test)]
#[path = "magic_test.rs"]
mod tests;
