use super::*;

#[test]
fn formats_rows_with_labels_hex_and_ascii() {
    let rows = rows(b"Hello\x00\x7f\xffWorld!!!!!!+tail", 0x20, 8);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].offset, 0x20);
    assert_eq!(rows[0].label, "00000020");
    assert_eq!(rows[0].hex[..6], ["48", "65", "6C", "6C", "6F", "00"]);
    assert_eq!(rows[0].ascii[..8].concat(), "Hello···");
    assert_eq!(rows[1].label, "00000030");
    assert_eq!(rows[1].hex.len(), 8);
}

#[test]
fn widens_offsets_for_large_files() {
    assert_eq!(offset_width(0), 8);
    assert_eq!(offset_width(0x1_0000_0000), 8);
    assert_eq!(offset_width(0x1_0000_0001), 9);
}

#[test]
fn reads_ranges_and_stops_at_the_end() {
    let path = std::env::temp_dir().join(format!("tfp-hex-file-{}", std::process::id()));
    std::fs::write(&path, b"0123456789").unwrap();
    let p = path.to_string_lossy();
    assert_eq!(read_at(&p, 2, 3).unwrap(), b"234");
    assert_eq!(read_at(&p, 8, 100).unwrap(), b"89");
    assert!(read_at(&p, 10, 4).unwrap().is_empty());
    assert_eq!(
        read_at("/no/such/file", 0, 1).unwrap_err().code,
        "fs.not_found"
    );
    let dir = std::env::temp_dir();
    assert_eq!(
        read_at(&dir.to_string_lossy(), 0, 1).unwrap_err().code,
        "hex.not_a_file"
    );
    std::fs::remove_file(path).unwrap();
}
