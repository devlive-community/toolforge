use super::*;
use crate::descriptor::build as enc;

#[test]
fn varint_roundtrip() {
    for value in [0u64, 1, 127, 128, 300, 1 << 20, u64::MAX] {
        let mut buf = Vec::new();
        enc::encode_varint(value, &mut buf);
        let mut reader = Reader::new(&buf);
        assert_eq!(reader.read_varint().unwrap(), value);
        assert!(reader.at_end());
    }
}

#[test]
fn varint_rejects_overlong() {
    // 11 个连续 MSB 字节：超过 10 字节上限
    let buf = [0x80u8; 11];
    let mut reader = Reader::new(&buf);
    assert_eq!(reader.read_varint().unwrap_err().code, "pbd.bad_varint");
}

#[test]
fn field_roundtrip() {
    let mut buf = Vec::new();
    enc::encode_varint_field(1, 42, &mut buf);
    enc::encode_string_field(2, "hello", &mut buf);
    enc::encode_varint_field(3, u64::MAX, &mut buf);
    let mut reader = Reader::new(&buf);
    let first = reader.next_field().unwrap().unwrap();
    assert_eq!(
        (first.number, first.wire_type, first.varint),
        (1, WT_VARINT, 42)
    );
    let second = reader.next_field().unwrap().unwrap();
    assert_eq!((second.number, second.wire_type), (2, WT_LEN));
    assert_eq!(second.bytes, b"hello");
    let third = reader.next_field().unwrap().unwrap();
    assert_eq!(third.varint, u64::MAX);
    assert!(reader.next_field().unwrap().is_none());
}

#[test]
fn truncated_input_rejected() {
    let mut buf = Vec::new();
    enc::encode_string_field(1, "abcdef", &mut buf);
    // 截掉载荷尾部
    let mut reader = Reader::new(&buf[..buf.len() - 2]);
    let err = reader.next_field().unwrap_err();
    assert_eq!(err.code, "pbd.truncated");
}

#[test]
fn bad_tag_and_wire_type_rejected() {
    // 字段号 0
    let mut buf = Vec::new();
    enc::encode_varint(0, &mut buf);
    assert_eq!(
        Reader::new(&buf).next_field().unwrap_err().code,
        "pbd.bad_tag"
    );
    // wire type 3（start group）已废弃
    let mut buf2 = Vec::new();
    enc::encode_key(1, 3, &mut buf2);
    assert_eq!(
        Reader::new(&buf2).next_field().unwrap_err().code,
        "pbd.bad_wire_type"
    );
}

#[test]
fn zigzag_roundtrip() {
    for value in [0i64, -1, 1, -2, i64::MAX, i64::MIN] {
        assert_eq!(zigzag_decode(enc::zigzag_encode(value)), value);
    }
    // 标准用例
    assert_eq!(zigzag_decode(0), 0);
    assert_eq!(zigzag_decode(1), -1);
    assert_eq!(zigzag_decode(2), 1);
    assert_eq!(zigzag_decode(u64::MAX), i64::MIN);
}

#[test]
fn fixed_width_fields() {
    let mut buf = Vec::new();
    enc::encode_key(5, WT_32BIT, &mut buf);
    buf.extend_from_slice(&[1, 2, 3, 4]);
    enc::encode_key(6, WT_64BIT, &mut buf);
    buf.extend_from_slice(&[0u8; 8]);
    let mut reader = Reader::new(&buf);
    let a = reader.next_field().unwrap().unwrap();
    assert_eq!((a.number, a.wire_type, a.bytes.len()), (5, WT_32BIT, 4));
    let b = reader.next_field().unwrap().unwrap();
    assert_eq!((b.number, b.wire_type, b.bytes.len()), (6, WT_64BIT, 8));
    assert!(reader.next_field().unwrap().is_none());
}
