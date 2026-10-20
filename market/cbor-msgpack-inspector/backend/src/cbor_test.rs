use super::*;
use crate::model::Ctx;

fn parse_ok(bytes: &[u8]) -> Node {
    let mut ctx = Ctx::new(bytes);
    let (node, end) = match parse(&mut ctx, 0, 0) {
        Ok(result) => result,
        Err(Stop) => panic!(
            "parse failed {:02x?}: {}",
            bytes,
            ctx.diags
                .iter()
                .map(|d| d.code.as_str())
                .collect::<Vec<_>>()
                .join(",")
        ),
    };
    assert_eq!(end, bytes.len(), "no trailing data expected");
    node
}

fn parse_err(bytes: &[u8]) -> Vec<String> {
    let mut ctx = Ctx::new(bytes);
    let _ = parse(&mut ctx, 0, 0);
    ctx.diags.iter().map(|d| d.code.clone()).collect()
}

#[test]
fn rfc_8949_appendix_a_vectors() {
    // major 0：1, 10, 23, 24, 25, 100, 1000, 1000000
    assert_eq!(parse_ok(&[0x00]).value.unwrap(), "0");
    assert_eq!(parse_ok(&[0x0a]).value.unwrap(), "10");
    assert_eq!(parse_ok(&[0x17]).value.unwrap(), "23");
    assert_eq!(parse_ok(&[0x18, 0x18]).value.unwrap(), "24");
    assert_eq!(parse_ok(&[0x18, 0x19]).value.unwrap(), "25");
    assert_eq!(parse_ok(&[0x18, 0x64]).value.unwrap(), "100");
    assert_eq!(parse_ok(&[0x19, 0x03, 0xe8]).value.unwrap(), "1000");
    assert_eq!(
        parse_ok(&[0x1a, 0x00, 0x0f, 0x42, 0x40]).value.unwrap(),
        "1000000"
    );
    assert_eq!(
        parse_ok(&[0x1b, 0x00, 0x00, 0x00, 0xe8, 0xd4, 0xa5, 0x10, 0x00])
            .value
            .unwrap(),
        "1000000000000"
    );
    // major 1：-1 - n
    assert_eq!(parse_ok(&[0x20]).value.unwrap(), "-1");
    assert_eq!(parse_ok(&[0x29]).value.unwrap(), "-10");
    assert_eq!(parse_ok(&[0x38, 0x63]).value.unwrap(), "-100");
    // 64 位边界：-18446744073709551616
    assert_eq!(
        parse_ok(&[0x3b, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff])
            .value
            .unwrap(),
        "-18446744073709551616"
    );
    // 0x1b 最大无符号
    assert_eq!(
        parse_ok(&[0x1b, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff])
            .value
            .unwrap(),
        "18446744073709551615"
    );
}

#[test]
fn floats_keep_negative_zero_and_non_finite() {
    // 半精度 -0.0（RFC 向量 0xf9 0x80 0x00）
    assert_eq!(parse_ok(&[0xf9, 0x80, 0x00]).value.unwrap(), "-0.0");
    // 半精度 1.5（0xf9 0x3e 0x00）
    assert_eq!(parse_ok(&[0xf9, 0x3e, 0x00]).value.unwrap(), "1.5");
    // 单精度 NaN / inf / -inf
    assert_eq!(
        parse_ok(&[0xfa, 0x7f, 0xc0, 0x00, 0x00]).value.unwrap(),
        "NaN"
    );
    assert_eq!(
        parse_ok(&[0xfa, 0x7f, 0x80, 0x00, 0x00]).value.unwrap(),
        "inf"
    );
    assert_eq!(
        parse_ok(&[0xfa, 0xff, 0x80, 0x00, 0x00]).value.unwrap(),
        "-inf"
    );
    // 双精度 1.1（major 7，AI 27 → 0xfb）
    assert_eq!(
        parse_ok(&[0xfb, 0x3f, 0xf1, 0x99, 0x99, 0x99, 0x99, 0x99, 0x9a])
            .value
            .unwrap(),
        "1.1"
    );
}

#[test]
fn text_bytes_and_indefinite_chunking() {
    // "IETF"
    let node = parse_ok(&[0x64, b'I', b'E', b'T', b'F']);
    assert_eq!(node.kind, "str");
    assert_eq!(node.value.unwrap(), "IETF");
    // 不定长文本 "strea" + "ming"（RFC 8949 附录 A 向量）
    let node = parse_ok(&[
        0x7f, 0x65, b's', b't', b'r', b'e', b'a', 0x64, b'm', b'i', b'n', b'g', 0xff,
    ]);
    assert_eq!(node.value.unwrap(), "streaming");
    assert_eq!(node.indefinite, Some(true));
    // 不定长字节串
    let node = parse_ok(&[0x5f, 0x42, 0x01, 0x02, 0x43, 0x03, 0x04, 0x05, 0xff]);
    assert_eq!(node.kind, "bytes");
    assert_eq!(node.value.unwrap(), "0102030405");
    // 分块类型混用（bytes 块后跟 text 块）→ 报错
    let codes = parse_err(&[0x7f, 0x41, 0x01, 0x61, 0x62, 0xff]);
    assert!(codes.contains(&"cbor.chunk_type".to_owned()));
}

#[test]
fn maps_keep_order_and_report_duplicates() {
    // {"a": 1, "b": [2, 3]}
    let node = parse_ok(&[0xa2, 0x61, b'a', 0x01, 0x61, b'b', 0x82, 0x02, 0x03]);
    let entries = node.entries.unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].key.value.as_ref().unwrap(), "a");
    // 非字符串键保留原类型：{1: 2, 3: 4} → 键是 uint
    let node = parse_ok(&[0xa2, 0x01, 0x02, 0x03, 0x04]);
    let entries = node.entries.unwrap();
    assert_eq!(entries[0].key.kind, "uint");
    assert_eq!(entries[0].key.value.as_ref().unwrap(), "1");
    // 重复键：不覆盖，逐条保留并告警
    let mut ctx = Ctx::new(&[0xa2, 0x61, b'a', 0x01, 0x61, b'a', 0x02]);
    let (node, _) = parse(&mut ctx, 0, 0).unwrap();
    assert_eq!(node.entries.unwrap().len(), 2);
    assert!(
        ctx.diags
            .iter()
            .any(|d| d.code == "bin.duplicate_key" && d.level == WARNING)
    );
}

#[test]
fn tags_and_bignums() {
    // tag 1 包裹 1（0xc1 0x01）：保留类型与原值，不解释成日期
    let node = parse_ok(&[0xc1, 0x01]);
    assert_eq!(node.kind, "tag");
    assert_eq!(node.number, Some(1));
    // tag 2：0x01 → 1
    assert_eq!(parse_ok(&[0xc2, 0x41, 0x01]).value.unwrap(), "1");
    // tag 3：0x00 → -1；0x0003 → -4
    assert_eq!(parse_ok(&[0xc3, 0x41, 0x00]).value.unwrap(), "-1");
    assert_eq!(parse_ok(&[0xc3, 0x42, 0x00, 0x03]).value.unwrap(), "-4");
    // 2^64 大整数
    assert_eq!(
        parse_ok(&[
            0xc2, 0x49, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00
        ])
        .value
        .unwrap(),
        "18446744073709551616"
    );
    // tag 2 的内容不是字节串 → 报错
    let codes = parse_err(&[0xc2, 0x01]);
    assert!(codes.contains(&"cbor.tag_bytes".to_owned()));
}

#[test]
fn simple_values_and_errors() {
    assert_eq!(parse_ok(&[0xf4]).kind, "bool");
    assert_eq!(parse_ok(&[0xf5]).kind, "bool");
    assert_eq!(parse_ok(&[0xf6]).kind, "null");
    // undefined 与 null 是不同类型
    assert_eq!(parse_ok(&[0xf7]).kind, "undefined");
    // simple value 1 字节形式必须 ≥32
    assert!(parse_err(&[0xf8, 0x1f]).contains(&"cbor.simple_invalid".to_owned()));
    // 保留附加信息 28-30
    assert!(parse_err(&[0x1c]).contains(&"cbor.reserved".to_owned()));
    // 裸 break
    assert!(parse_err(&[0xff]).contains(&"cbor.break".to_owned()));
    // 截断
    assert!(!parse_err(&[0x19, 0x03]).is_empty());
    // 非法 UTF-8
    assert!(parse_err(&[0x62, 0xc3, 0x28]).contains(&"cbor.utf8".to_owned()));
}

#[test]
fn trailing_data_is_reported() {
    let mut ctx = Ctx::new(&[0x01, 0x02, 0x03]);
    let (root, trailing) = run(&mut ctx);
    let root = root.unwrap();
    assert_eq!(root.value.unwrap(), "1");
    let trailing = trailing.unwrap();
    assert_eq!(trailing.offset, 1);
    assert_eq!(trailing.length, 2);
}

#[test]
fn indefinite_array_and_map() {
    // [_ 1, 2]
    let node = parse_ok(&[0x9f, 0x01, 0x02, 0xff]);
    assert_eq!(node.children.unwrap().len(), 2);
    assert_eq!(node.indefinite, Some(true));
    // {_ "a": 1}
    let node = parse_ok(&[0xbf, 0x61, b'a', 0x01, 0xff]);
    assert_eq!(node.entries.unwrap().len(), 1);
}
