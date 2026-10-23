use super::*;

#[test]
fn detects_common_formats() {
    assert_eq!(detect(b"\x89PNG\r\n\x1a\n\0\0"), Some("PNG image"));
    assert_eq!(detect(b"%PDF-1.7\n"), Some("PDF document"));
    assert_eq!(detect(b"RIFF\0\0\0\0WEBPVP8 "), Some("WebP image"));
    assert_eq!(detect(b"RIFF\0\0\0\0WAVEfmt "), Some("WAV audio"));
    assert_eq!(detect(b"\0\0\0\x20ftypisom"), Some("MP4 / QuickTime"));
    assert_eq!(detect(b"SQLite format 3\0..."), Some("SQLite database"));
    let mut tar = vec![0u8; 300];
    tar[257..262].copy_from_slice(b"ustar");
    assert_eq!(detect(&tar), Some("tar archive"));
    assert_eq!(detect(b"plain text"), None);
    assert_eq!(detect(b""), None);
}
