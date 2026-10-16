use super::*;

/// 用规则补全校验位，得到合法代码
fn complete(first17: &str) -> String {
    format!("{first17}{}", check_char(first17).unwrap())
}

#[test]
fn computes_known_check_digits() {
    // 公开示例：91350100M000100Y43（福州某企业，见 GB 32100 示例格式）
    assert_eq!(check_char("91350100M000100Y4"), Some('3'));
    let code = complete("91110000600037341");
    let report = check(&code);
    assert_eq!(report.status, Status::Valid, "{code}");
    let fields = report.fields.unwrap();
    assert_eq!(fields.department.name, Some("industry_commerce"));
    assert_eq!(fields.category.name, Some("enterprise"));
    assert_eq!(fields.region.code, "110000");
    assert_eq!(fields.region.name, Some("beijing"));
    assert_eq!(fields.subject, "600037341");
}

#[test]
fn org_code_check_uses_gb_11714() {
    // GB 11714 示例：D2143569-X
    assert_eq!(org_check_char("D2143569"), Some('X'));
    assert_eq!(org_check_char("12345678"), Some('8'));
}

#[test]
fn reports_single_character_errors_and_expected_check() {
    let code = complete("91350100M000100Y4");
    let mut broken: Vec<char> = code.chars().collect();
    broken[17] = if broken[17] == '0' { '1' } else { '0' };
    let report = check(&broken.iter().collect::<String>());
    assert_eq!(report.status, Status::Invalid);
    assert_eq!(
        report.problems,
        vec![Problem::Check {
            expected: code[17..].to_owned(),
            found: broken[17].to_string()
        }]
    );
    // 改动前 17 位中的任意一位也会被校验位发现
    let mut changed: Vec<char> = code.chars().collect();
    changed[5] = if changed[5] == '1' { '2' } else { '1' };
    assert_eq!(
        check(&changed.iter().collect::<String>()).status,
        Status::Invalid
    );
}

#[test]
fn rejects_length_and_forbidden_characters() {
    assert_eq!(
        check("9135010").problems,
        vec![Problem::Length { found: 7 }]
    );
    let report = check("91350100M00010OY43");
    assert_eq!(
        report.problems,
        vec![Problem::Char {
            position: 15,
            found: "O".into()
        }]
    );
    for c in ['I', 'O', 'Z', 'S', 'V'] {
        assert!(!CHARSET.contains(c));
    }
}

#[test]
fn unknown_tables_are_unconfirmed_not_invalid() {
    let unknown_department = complete("B1350100MA0001000");
    assert_eq!(check(&unknown_department).status, Status::Unconfirmed);
    let unknown_category = complete("9535010000000000X");
    let report = check(&unknown_category);
    assert_eq!(report.status, Status::Unconfirmed);
    assert_eq!(report.fields.unwrap().category.name, None);
    // 不认识的地区不影响结论
    let region = complete("91990000MA0001000");
    let report = check(&region);
    assert_eq!(report.status, Status::Valid);
    assert_eq!(report.fields.unwrap().region.name, None);
}

#[test]
fn normalizes_visibly() {
    let n = normalize("  9135 0100ｍ000100y43 ");
    assert_eq!(n.value, "91350100M000100Y43");
    assert!(n.trimmed && n.removed_spaces && n.uppercased && n.fullwidth);
    let clean = normalize("91350100M000100Y43");
    assert!(!clean.trimmed && !clean.removed_spaces && !clean.uppercased && !clean.fullwidth);
}
