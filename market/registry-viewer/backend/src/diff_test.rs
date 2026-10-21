use super::*;
use crate::doc;

fn sample_a() -> String {
    r#"Windows Registry Editor Version 5.00

[HKEY_CURRENT_USER\Software\App]
"Keep"="same"
"Typed"="text"
"Repeated"="text"
"Repeated"=dword:00000001
"Changed"="old"
"OnlyA"="a"
"#
    .to_owned()
}

fn sample_b() -> String {
    r#"Windows Registry Editor Version 5.00

[HKEY_CURRENT_USER\Software\App]
"Keep"="same"
"Typed"=dword:00000002
"Repeated"=dword:00000002
"Changed"="new"
"OnlyB"="b"

[HKEY_CURRENT_USER\Software\New]
"Fresh"="yes"
"#
    .to_owned()
}

#[test]
fn diffs_types_data_and_presence() {
    let a = doc::parse(&sample_a()).unwrap();
    let b = doc::parse(&sample_b()).unwrap();
    let report = diff(&a, &b, None).unwrap();
    let kinds = |name: &str| -> Vec<&DiffKind> {
        report
            .entries
            .iter()
            .filter(|e| e.value_name.as_deref() == Some(name) || e.path.contains(name))
            .map(|e| &e.kind)
            .collect()
    };
    // 大小写不敏感匹配，不制造虚假增删
    assert!(!report.entries.iter().any(|e| {
        e.value_name
            .as_deref()
            .map(|n| n.eq_ignore_ascii_case("keep"))
            .unwrap_or(false)
    }));
    // 同名不同类型 → 类型变化
    let typed = kinds("Typed");
    assert!(typed.iter().any(|k| matches!(k, DiffKind::TypeChanged)));
    // 重复赋值只比较最后一条：DWORD 1 → DWORD 2，不能误报类型变化
    let repeated = kinds("Repeated");
    assert_eq!(repeated.len(), 1);
    assert!(matches!(repeated[0], DiffKind::DataChanged));
    // 类型一致数据不同 → 数据变化
    let changed = kinds("Changed");
    assert!(changed.iter().any(|k| matches!(k, DiffKind::DataChanged)));
    // 单侧存在
    assert!(
        kinds("OnlyA")
            .iter()
            .any(|k| matches!(k, DiffKind::ValueMissing))
    );
    assert!(
        kinds("OnlyB")
            .iter()
            .any(|k| matches!(k, DiffKind::ValueAdded))
    );
    // B 独有键
    assert!(
        report
            .entries
            .iter()
            .any(|e| matches!(e.kind, DiffKind::KeyAdded) && e.path.ends_with("New"))
    );
    assert!(!report.identical);
    assert!(!report.has_delete_statements);
}

#[test]
fn identical_files_and_value_name_case() {
    let text = r#"Windows Registry Editor Version 5.00

[HKEY_CURRENT_USER\Software\App]
"Keep"="same"
"#;
    let a = doc::parse(text).unwrap();
    let b = doc::parse(text).unwrap();
    let report = diff(&a, &b, None).unwrap();
    assert!(report.identical);

    // 值名大小写不同但数据相同：不制造虚假增删
    let case = r#"Windows Registry Editor Version 5.00

[HKEY_CURRENT_USER\Software\App]
"keep"="same"
"#;
    let c = doc::parse(case).unwrap();
    let report = diff(&a, &c, None).unwrap();
    assert!(
        report.identical,
        "value name case should not diff: {:?}",
        report.entries
    );
}

#[test]
fn flags_delete_statements() {
    let text = r#"Windows Registry Editor Version 5.00

[-HKEY_CURRENT_USER\Software\Old]
"#;
    let doc = doc::parse(text).unwrap();
    let other = doc::parse("Windows Registry Editor Version 5.00\n").unwrap();
    let report = diff(&doc, &other, None).unwrap();
    assert!(report.has_delete_statements);
}
