use super::*;
use crate::model::Ctx;

fn parse_ok(bytes: &[u8]) -> Node {
    let mut ctx = Ctx::new(bytes);
    let (node, end) = parse(&mut ctx, 0, 0).unwrap();
    assert_eq!(end, bytes.len());
    node
}

fn parse_err(bytes: &[u8]) -> Vec<String> {
    let mut ctx = Ctx::new(bytes);
    let _ = parse(&mut ctx, 0, 0);
    ctx.diags.iter().map(|d| d.code.clone()).collect()
}

#[test]
fn fixint_and_int_range_boundaries() {
    assert_eq!(parse_ok(&[0x00]).value.unwrap(), "0");
    assert_eq!(parse_ok(&[0x7f]).value.unwrap(), "127");
    assert_eq!(parse_ok(&[0xe0]).value.unwrap(), "-32");
    assert_eq!(parse_ok(&[0xff]).value.unwrap(), "-1");
    // uint8/16/32/64 全范围
    assert_eq!(parse_ok(&[0xcc, 0xff]).value.unwrap(), "255");
    assert_eq!(parse_ok(&[0xcd, 0xff, 0xff]).value.unwrap(), "65535");
    assert_eq!(
        parse_ok(&[0xce, 0xff, 0xff, 0xff, 0xff]).value.unwrap(),
        "4294967295"
    );
    assert_eq!(
        parse_ok(&[0xcf, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff])
            .value
            .unwrap(),
        "18446744073709551615"
    );
    // int8/16/32/64 负边界
    assert_eq!(parse_ok(&[0xd0, 0x80]).value.unwrap(), "-128");
    assert_eq!(
        parse_ok(&[0xd3, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00])
            .value
            .unwrap(),
        "-9223372036854775808"
    );
}

#[test]
fn str_bin_distinction_and_invalid_utf8() {
    // "a"
    let node = parse_ok(&[0xa1, b'a']);
    assert_eq!(node.kind, "str");
    assert_eq!(node.value.unwrap(), "a");
    // str16 "abc"
    let node = parse_ok(&[0xda, 0x00, 0x03, b'a', b'b', b'c']);
    assert_eq!(node.value.unwrap(), "abc");
    // bin8 [0x01, 0x02]：bytes 与 str 是不同类型
    let node = parse_ok(&[0xc4, 0x02, 0x01, 0x02]);
    assert_eq!(node.kind, "bytes");
    assert_eq!(node.value.unwrap(), "0102");
    // 非法 UTF-8 的 str
    assert!(parse_err(&[0xa2, 0xc3, 0x28]).contains(&"mp.utf8".to_owned()));
}

#[test]
fn nil_bool_float_and_reserved() {
    assert_eq!(parse_ok(&[0xc0]).kind, "null");
    assert_eq!(parse_ok(&[0xc2]).value.unwrap(), "false");
    assert_eq!(parse_ok(&[0xc3]).value.unwrap(), "true");
    // -0.0（float64 符号位为 1，其余为 0）
    assert_eq!(
        parse_ok(&[0xcb, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00])
            .value
            .unwrap(),
        "-0.0"
    );
    assert_eq!(
        parse_ok(&[0xca, 0x7f, 0x80, 0x00, 0x00]).value.unwrap(),
        "inf"
    );
    assert_eq!(
        parse_ok(&[0xcb, 0x7f, 0xf8, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00])
            .value
            .unwrap(),
        "NaN"
    );
    // 0xc1 是保留码
    assert!(parse_err(&[0xc1]).contains(&"mp.reserved".to_owned()));
}

#[test]
fn arrays_maps_and_duplicate_keys() {
    // ["a", "b"]
    let node = parse_ok(&[0x92, 0xa1, b'a', 0xa1, b'b']);
    assert_eq!(node.children.unwrap().len(), 2);
    // array16 空数组
    let node = parse_ok(&[0xdc, 0x00, 0x00]);
    assert_eq!(node.children.unwrap().len(), 0);
    // {"a": 1}
    let node = parse_ok(&[0x81, 0xa1, b'a', 0x01]);
    assert_eq!(node.entries.unwrap().len(), 1);
    // 非字符串键保留类型：{1: "a"}
    let node = parse_ok(&[0x81, 0x01, 0xa1, b'a']);
    assert_eq!(node.entries.unwrap()[0].key.kind, "uint");
    // 重复键告警且不覆盖
    let mut ctx = Ctx::new(&[0x82, 0x01, 0x01, 0x01, 0x02]);
    let (node, _) = parse(&mut ctx, 0, 0).unwrap();
    assert_eq!(node.entries.unwrap().len(), 2);
    assert!(ctx.diags.iter().any(|d| d.code == "bin.duplicate_key"));
}

#[test]
fn ext_and_timestamp() {
    // fixext1 type 0x05 payload 0x10：未知扩展保留类型与原值
    let node = parse_ok(&[0xd4, 0x05, 0x10]);
    assert_eq!(node.kind, "ext");
    assert_eq!(node.number, Some(5));
    assert_eq!(node.value.unwrap(), "10");
    // ext8：长度 3，载荷 de ad be
    let node = parse_ok(&[0xc7, 0x03, 0x30, 0xde, 0xad, 0xbe]);
    assert_eq!(node.number, Some(0x30));
    assert_eq!(node.value.unwrap(), "deadbe");
    // timestamp 32 位（fixext4, type -1）
    let node = parse_ok(&[0xd6, 0xff, 0x00, 0x00, 0x00, 0x00]);
    assert_eq!(node.kind, "timestamp");
    assert_eq!(node.value.unwrap(), "seconds=0, nanoseconds=0");
    // timestamp 64 位：ns 与秒拼装
    let node = parse_ok(&[0xd7, 0xff, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
    assert_eq!(node.value.unwrap(), "seconds=0, nanoseconds=0");
}

#[test]
fn truncation_and_trailing() {
    // str 声明 2 字节只有 1 字节
    assert!(!parse_err(&[0xa2, b'a']).is_empty());
    // array 声明 2 个元素只有 1 个
    assert!(!parse_err(&[0x92, 0x01]).is_empty());
    // 尾随数据单独报告
    let mut ctx = Ctx::new(&[0x01, 0x02]);
    let (root, trailing) = run(&mut ctx);
    assert_eq!(root.unwrap().value.unwrap(), "1");
    let trailing = trailing.unwrap();
    assert_eq!(trailing.offset, 1);
    assert_eq!(trailing.length, 1);
}
