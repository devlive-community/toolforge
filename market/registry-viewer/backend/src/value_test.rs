use crate::value::RegValue;

#[test]
fn decodes_all_supported_types() {
    // REG_SZ：UTF-16LE "hi" + 终止符
    let (value, issues) = RegValue::from_hex(1, vec![0x68, 0, 0x69, 0, 0, 0]);
    assert_eq!(
        value,
        RegValue::Sz {
            text: "hi".to_owned()
        }
    );
    assert!(issues.is_empty());

    // REG_DWORD
    let (value, _) = RegValue::from_hex(4, vec![0xff, 0, 0, 0]);
    assert_eq!(value, RegValue::Dword { value: 255 });

    // REG_QWORD：64 位边界
    let (value, _) = RegValue::from_hex(11, vec![0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]);
    match value {
        RegValue::Qword { value } => assert_eq!(value, u64::MAX),
        other => panic!("expected qword, got {other:?}"),
    }

    // REG_BINARY 原样
    let (value, _) = RegValue::from_hex(3, vec![0xde, 0xad]);
    assert_eq!(
        value,
        RegValue::Binary {
            bytes: vec![0xde, 0xad]
        }
    );

    // 未知类型保留类型号
    let (value, _) = RegValue::from_hex(9, vec![1, 2]);
    assert_eq!(
        value,
        RegValue::Unknown {
            type_id: 9,
            bytes: vec![1, 2]
        }
    );
}

#[test]
fn detects_corrupt_payloads() {
    // 奇数字节
    let (_, issues) = RegValue::from_hex(1, vec![0x68]);
    assert!(issues.contains(&"reg.odd_utf16_length".to_owned()));
    // 无终止符
    let (_, issues) = RegValue::from_hex(1, vec![0x68, 0]);
    assert!(issues.contains(&"reg.missing_terminator".to_owned()));
    // DWORD 长度错误
    let (value, issues) = RegValue::from_hex(4, vec![1, 2]);
    assert!(issues.contains(&"reg.bad_dword_length".to_owned()));
    assert!(matches!(value, RegValue::Unknown { type_id: 4, .. }));
}

#[test]
fn multi_sz_keeps_boundaries() {
    // "a\r\nb" 多字符串：a\0 \r \n b\0 \0（LE）
    let bytes = vec![0x61, 0, 0x0d, 0, 0x0a, 0, 0x62, 0, 0, 0, 0, 0];
    let (value, issues) = RegValue::from_hex(7, bytes);
    assert_eq!(
        value,
        RegValue::MultiSz {
            items: vec!["a\r\nb".to_owned()]
        }
    );
    assert!(issues.is_empty());

    // 两个条目：a\0 b\0 \0
    let bytes = vec![0x61, 0, 0, 0, 0x62, 0, 0, 0, 0, 0];
    let (value, _) = RegValue::from_hex(7, bytes);
    assert_eq!(
        value,
        RegValue::MultiSz {
            items: vec!["a".to_owned(), "b".to_owned()]
        }
    );
}

#[test]
fn same_requires_same_type() {
    let sz = RegValue::Sz {
        text: "1".to_owned(),
    };
    let dword = RegValue::Dword { value: 1 };
    // 同字节不同类型 → 不等
    assert!(!sz.same(&dword));
    assert!(sz.same(&RegValue::Sz {
        text: "1".to_owned()
    }));
}
