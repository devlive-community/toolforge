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
