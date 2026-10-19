use super::*;

#[test]
fn bigint_to_decimal_converts_256_be_bytes() {
    // RFC 8949 附录 A 向量：tag 2 的 0x01 → 1，0x0003 → 3
    assert_eq!(bigint_to_decimal(&[0x01], false), "1");
    assert_eq!(bigint_to_decimal(&[0x00, 0x03], false), "3");
    // 18446744073709551616 = 2^64（超出 u64）
    assert_eq!(
        bigint_to_decimal(
            &[0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
            false
        ),
        "18446744073709551616"
    );
    // 负大整数：tag 3 的 0x00 → -1
    assert_eq!(bigint_to_decimal(&[0x00], true), "-1");
    assert_eq!(bigint_to_decimal(&[0x00, 0x64], true), "-101");
    // 前导零被去除
    assert_eq!(bigint_to_decimal(&[0x00, 0x00, 0x2a], false), "42");
}

#[test]
fn hex_encodes_bytes() {
    assert_eq!(hex(&[0x00, 0xff, 0x1a]), "00ff1a");
    assert_eq!(hex(&[]), "");
}

#[test]
fn report_diag_updates_stats() {
    let mut report = Report {
        format: "cbor",
        overall: "issues",
        incomplete: false,
        stats: Stats::default(),
        root: None,
        trailing: None,
        diagnostics: Vec::new(),
    };
    push_report_diag(
        &mut report,
        ERROR,
        "transport.hex",
        serde_json::Map::new(),
        None,
    );
    push_report_diag(
        &mut report,
        WARNING,
        "bin.duplicate_key",
        serde_json::Map::new(),
        Some(3),
    );
    assert_eq!(report.stats.errors, 1);
    assert_eq!(report.stats.warnings, 1);
    assert_eq!(report.diagnostics.len(), 2);
}
