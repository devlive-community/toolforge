use super::*;

fn never() -> bool {
    false
}

fn fixture(name: &str, bytes: &[u8]) -> String {
    let path = std::env::temp_dir().join(format!("tfp-hex-search-{name}-{}", std::process::id()));
    std::fs::write(&path, bytes).unwrap();
    path.to_string_lossy().into_owned()
}

#[test]
fn parses_hex_and_text_patterns() {
    assert_eq!(
        pattern("de ad,BE 0xef", Mode::Hex).unwrap(),
        [0xde, 0xad, 0xbe, 0xef]
    );
    assert_eq!(pattern("0x4F50", Mode::Hex).unwrap(), [0x4f, 0x50]);
    assert_eq!(
        pattern("abc", Mode::Hex).unwrap_err().code,
        "hex.invalid_pattern"
    );
    assert_eq!(
        pattern("zz", Mode::Hex).unwrap_err().code,
        "hex.invalid_pattern"
    );
    assert_eq!(
        pattern("  ", Mode::Hex).unwrap_err().code,
        "hex.empty_pattern"
    );
    assert_eq!(pattern("中", Mode::Text).unwrap(), "中".as_bytes());
}

#[test]
fn finds_forward_and_backward() {
    let path = fixture("basic", b"abcXYZabcXYZabc");
    let f = |from| forward(&path, b"XYZ", from, false, &never).unwrap();
    assert_eq!(f(0), Some(3));
    assert_eq!(f(3), Some(3));
    assert_eq!(f(4), Some(9));
    assert_eq!(f(10), None);
    let b = |before| backward(&path, b"XYZ", before, false, &never).unwrap();
    assert_eq!(b(15), Some(9));
    assert_eq!(b(9), Some(3));
    assert_eq!(b(3), None);
    assert_eq!(forward(&path, b"xyz", 0, true, &never).unwrap(), Some(3));
    assert_eq!(forward(&path, b"xyz", 0, false, &never).unwrap(), None);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn finds_matches_across_chunk_boundaries() {
    let mut bytes = vec![0u8; CHUNK * 2 + 100];
    let at = CHUNK - 2;
    bytes[at..at + 4].copy_from_slice(b"NEED");
    let late = CHUNK * 2 + 10;
    bytes[late..late + 4].copy_from_slice(b"NEED");
    let path = fixture("chunks", &bytes);
    assert_eq!(
        forward(&path, b"NEED", 0, false, &never).unwrap(),
        Some(at as u64)
    );
    assert_eq!(
        forward(&path, b"NEED", at as u64 + 1, false, &never).unwrap(),
        Some(late as u64)
    );
    assert_eq!(
        backward(&path, b"NEED", bytes.len() as u64, false, &never).unwrap(),
        Some(late as u64)
    );
    assert_eq!(
        backward(&path, b"NEED", late as u64, false, &never).unwrap(),
        Some(at as u64)
    );
    std::fs::remove_file(path).unwrap();
}
