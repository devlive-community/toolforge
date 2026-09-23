use super::*;

#[test]
fn ascii_offsets_are_identical() {
    let offsets = Utf16Offsets::new("abc");
    assert_eq!((offsets.get(0), offsets.get(3)), (0, 3));
}

#[test]
fn cjk_and_emoji_convert_to_utf16_units() {
    // "中" 3 字节 / 1 单元；"😀" 4 字节 / 2 单元
    let text = "中😀a";
    let offsets = Utf16Offsets::new(text);
    assert_eq!(offsets.get(3), 1);
    assert_eq!(offsets.get(7), 3);
    assert_eq!(offsets.get(text.len()), 4);
    assert_eq!(text.encode_utf16().count(), 4);
}
