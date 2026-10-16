use super::*;

fn get<'a>(values: &'a [Value], kind: &str) -> Option<&'a str> {
    values
        .iter()
        .find(|v| v.kind == kind)
        .map(|v| v.value.as_str())
}

#[test]
fn reads_integers_in_both_byte_orders() {
    let bytes = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x88];
    let le = values(&bytes, true);
    assert_eq!(get(&le, "binary"), Some("00000001"));
    assert_eq!(get(&le, "u16"), Some("513"));
    assert_eq!(get(&le, "u32"), Some("67305985"));
    assert_eq!(
        get(&le, "i64"),
        Some(i64::from_le_bytes(bytes).to_string().as_str())
    );
    let be = values(&bytes, false);
    assert_eq!(get(&be, "u16"), Some("258"));
    assert_eq!(get(&be, "u32"), Some("16909060"));
    assert_eq!(get(&values(&[0xff], true), "i8"), Some("-1"));
}

#[test]
fn reads_floats_text_and_timestamps() {
    let le = values(&1.5f32.to_le_bytes(), true);
    assert_eq!(get(&le, "f32"), Some("1.5"));
    assert_eq!(get(&le, "f64"), None, "needs eight bytes");
    assert_eq!(
        get(&values(&0.1f32.to_le_bytes(), true), "f32"),
        Some("0.1")
    );
    assert_eq!(
        get(&values(&1.5e-20f64.to_le_bytes(), true), "f64"),
        Some("1.5e-20")
    );
    assert_eq!(
        get(&values(&2.5e20f64.to_le_bytes(), true), "f64"),
        Some("2.5e20")
    );
    assert_eq!(
        get(&values(&f64::NAN.to_le_bytes(), true), "f64"),
        Some("NaN")
    );
    assert_eq!(
        get(&values("中x".as_bytes(), true), "utf8"),
        Some("中 U+4E2D")
    );
    assert_eq!(get(&values(&[0xff, 0], true), "utf8"), None);
    let t = values(&1_700_000_000u32.to_be_bytes(), false);
    assert_eq!(get(&t, "unix32"), Some("2023-11-14 22:13:20 UTC"));
    let ms = values(&1_700_000_000_123i64.to_le_bytes(), true);
    assert_eq!(get(&ms, "unixMs"), Some("2023-11-14 22:13:20.123 UTC"));
}

#[test]
fn reads_varint_zigzag_bcd_and_fixed_point() {
    // protobuf 文档示例：150 = 96 01
    let v = values(&[0x96, 0x01, 0xff], true);
    assert_eq!(get(&v, "varint"), Some("150 (2 B)"));
    assert_eq!(get(&v, "zigzag"), Some("75"));
    assert_eq!(get(&values(&[0x03], true), "zigzag"), Some("-2"));
    assert_eq!(
        get(&values(&[0x12, 0x34, 0x5f], true), "bcd"),
        Some("12345")
    );
    assert_eq!(get(&values(&[0xa1], true), "bcd"), None);
    assert_eq!(
        get(&values(&[0x80, 0x01], false), "fixed8_8"),
        Some("-127.99609375")
    );
    assert_eq!(
        get(&values(&0x0001_8000u32.to_le_bytes(), true), "fixed16_16"),
        Some("1.5")
    );
}

#[test]
fn reads_uuid_in_rfc_and_guid_order() {
    let id = uuid::Uuid::parse_str("017f22e2-79b0-7cc3-98c4-dc0c0c07398f").unwrap();
    let be = values(id.as_bytes(), false);
    let text = get(&be, "uuid").unwrap();
    assert!(
        text.starts_with("017f22e2-79b0-7cc3-98c4-dc0c0c07398f · v7 · 2022-02-22"),
        "{text}"
    );
    let le = values(&id.to_bytes_le(), true);
    assert!(get(&le, "uuid").unwrap().starts_with("017f22e2-79b0-7cc3"));
}

#[test]
fn reads_colors_strings_and_encoded_text() {
    let v = values(&[0xff, 0x80, 0x00, 0x7f], true);
    assert_eq!(get(&v, "rgba"), Some("#FF80007F"));
    assert_eq!(
        get(&values(&0xf800u16.to_le_bytes(), true), "rgb565"),
        Some("#FF0000")
    );
    assert_eq!(
        get(&values(b"hello\0world", true), "cstring"),
        Some("\"hello\" (5 B)")
    );
    assert_eq!(get(&values(b"no terminator", true), "cstring"), None);
    assert_eq!(
        get(&values(b"SGVsbG8gV29ybGQ=!", true), "base64"),
        Some("SGVsbG8gV29ybGQ= → \"Hello World\" (11 B)")
    );
    assert_eq!(
        get(&values(b"JBSWY3DPEE======", true), "base32").map(|s| s.ends_with("\"Hello!\" (6 B)")),
        Some(true)
    );
    assert_eq!(get(&values(b"abc", true), "base64"), None);
}
