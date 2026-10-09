use super::*;

fn gbk(text: &str) -> Vec<u8> {
    GB18030.encode(text).0.into_owned()
}

#[test]
fn decodes_names() {
    assert_eq!(
        name("报告/说明.txt".as_bytes(), Some("ignored")),
        "报告/说明.txt"
    );
    // 没有 UTF-8 标记：解压库按 CP437 逐字节解码，字符数等于字节数
    let raw = gbk("报告/说明.txt");
    let cp437_like: String = raw
        .iter()
        .map(|&b| if b < 0x80 { b as char } else { '·' })
        .collect();
    assert_eq!(name(&raw, Some(&cp437_like)), "报告/说明.txt");
    // Unicode 路径扩展字段给出的名称与字节数不同，以它为准
    assert_eq!(name(&raw, Some("扩展字段.txt")), "扩展字段.txt");
    assert_eq!(name(&gbk("资料.tar"), None), "资料.tar");
    assert_eq!(name(b"plain.txt", Some("plain.txt")), "plain.txt");
}

#[test]
fn decodes_text_contents() {
    assert_eq!(
        content("中文 UTF-8".as_bytes(), false).as_deref(),
        Some("中文 UTF-8")
    );
    assert_eq!(content(b"\xef\xbb\xbfbom", false).as_deref(), Some("bom"));
    assert_eq!(
        content(&gbk("数据库=本地\n"), false).as_deref(),
        Some("数据库=本地\n")
    );
    let mut le = vec![0xff, 0xfe];
    le.extend("宽字符".encode_utf16().flat_map(u16::to_le_bytes));
    assert_eq!(content(&le, false).as_deref(), Some("宽字符"));
    // 截断在双字节字符中间
    let mut cut = gbk("中文内容");
    cut.pop();
    assert_eq!(content(&cut, true).as_deref(), Some("中文内"));
    assert_eq!(content(b"\x89PNG\r\n\x1a\n\0\0", false), None);
}
