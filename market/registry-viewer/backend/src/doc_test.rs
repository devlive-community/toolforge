use super::*;

const SAMPLE: &str = r#"Windows Registry Editor Version 5.00

; 注释行
[HKEY_CURRENT_USER\Software\Test]
"String"="hello \"quoted\" back\\slash"
@="默认值"
"Expanded"=hex(2):25,00,55,00,53,00,45,00,52,00,25,00,00,00
"Multi"=hex(7):61,00,0d,00,0a,00,62,00,00,00,00,00
"Binary"=hex:de,ad,be,ef
"Qword"=hex(b):ff,ff,ff,ff,ff,ff,ff,ff
"Number"=dword:000000ff
"Deleted"=-
"Repeat"="first"
"Repeat"="second"

[-HKEY_LOCAL_MACHINE\Software\Old]
"#;

const CONTINUATION: &str = r#"Windows Registry Editor Version 5.00

[HKEY_CURRENT_USER\Software\Long]
"Big"=hex:01,02,03,04,05,06,07,08,09,0a,0b,0c,0d,0e,0f,\
  10,11,12
"#;

#[test]
fn parses_keys_values_and_delete_statements() {
    let doc = parse(SAMPLE).unwrap();
    assert_eq!(doc.keys.len(), 2);
    let key = &doc.keys[0];
    assert_eq!(key.path, r#"HKEY_CURRENT_USER\Software\Test"#);
    assert_eq!(key.line, 4);
    assert!(!key.delete);
    // 默认值
    let default = key.values.iter().find(|v| v.is_default).unwrap();
    assert_eq!(
        default.value,
        RegValue::Sz {
            text: "默认值".to_owned()
        }
    );
    // 转义
    let string = key.values.iter().find(|v| v.name == "String").unwrap();
    assert_eq!(
        string.value,
        RegValue::Sz {
            text: "hello \"quoted\" back\\slash".to_owned()
        }
    );
    // DWORD 不经浮点
    let number = key.values.iter().find(|v| v.name == "Number").unwrap();
    assert_eq!(number.value, RegValue::Dword { value: 255 });
    // QWORD 64 位边界
    let qword = key.values.iter().find(|v| v.name == "Qword").unwrap();
    match &qword.value {
        RegValue::Qword { value } => assert_eq!(*value, u64::MAX),
        other => panic!("expected qword: {other:?}"),
    }
    // MULTI_SZ
    let multi = key.values.iter().find(|v| v.name == "Multi").unwrap();
    match &multi.value {
        RegValue::MultiSz { items } => assert_eq!(items, &vec!["a\r\nb".to_owned()]),
        other => panic!("expected multi: {other:?}"),
    }
    // EXPAND_SZ 保留原文
    let expanded = key.values.iter().find(|v| v.name == "Expanded").unwrap();
    assert_eq!(
        expanded.value,
        RegValue::ExpandSz {
            text: "%USER%".to_owned()
        }
    );
    // 删值声明：保留为未知类型 + 诊断
    let deleted = key.values.iter().find(|v| v.name == "Deleted").unwrap();
    assert!(deleted.issues.contains(&"reg.delete_value".to_owned()));
    // 重复赋值保留原顺序并标记
    let repeats: Vec<&_> = key.values.iter().filter(|v| v.name == "Repeat").collect();
    assert_eq!(repeats.len(), 2);
    assert!(!repeats[0].duplicate);
    assert!(repeats[1].duplicate);
    // 删键
    assert!(doc.keys[1].delete);
    assert_eq!(doc.keys[1].path, r#"HKEY_LOCAL_MACHINE\Software\Old"#);
}

#[test]
fn parses_hex_continuation_lines() {
    let doc = parse(CONTINUATION).unwrap();
    let key = &doc.keys[0];
    let big = key.values.iter().find(|v| v.name == "Big").unwrap();
    match &big.value {
        RegValue::Binary { bytes } => {
            assert_eq!(bytes.len(), 18);
            assert_eq!(bytes[0], 0x01);
            assert_eq!(bytes[17], 0x12);
        }
        other => panic!("expected binary: {other:?}"),
    }
    // 续行起始行号是第一行
    assert_eq!(big.line, 4);
}

#[test]
fn rejects_bad_headers_and_versions() {
    let error = parse("REGEDIT4\n").unwrap_err();
    assert_eq!(error.code, "reg.unsupported_version");
    let error = parse("").unwrap_err();
    assert_eq!(error.code, "reg.bad_header");
    let error = parse("random text").unwrap_err();
    assert_eq!(error.code, "reg.bad_header");
}

#[test]
fn decodes_utf16le_bom() {
    let mut bytes = vec![0xFF, 0xFE];
    let text = "Windows Registry Editor Version 5.00\r\n\r\n[HKEY_USERS\\X]\r\n";
    for unit in text.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    let decoded = decode_text(&bytes).unwrap();
    assert!(decoded.starts_with("Windows Registry Editor Version 5.00"));
    let doc = parse(&decoded).unwrap();
    assert_eq!(doc.keys[0].path, r#"HKEY_USERS\X"#);
}

#[test]
fn decodes_utf8_with_and_without_bom() {
    let text = "Windows Registry Editor Version 5.00\n\n[HKEY_USERS\\X]\n";
    let bom = decode_text(
        &[0xEF, 0xBB, 0xBF]
            .iter()
            .chain(text.as_bytes())
            .cloned()
            .collect::<Vec<u8>>(),
    )
    .unwrap();
    let plain = decode_text(text.as_bytes()).unwrap();
    assert!(parse(&bom).is_ok());
    assert!(parse(&plain).is_ok());
}
