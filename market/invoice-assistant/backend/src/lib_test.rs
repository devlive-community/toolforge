use super::*;
use serde_json::json;

#[test]
fn splits_tax_both_directions() {
    // 113 元含税 13%：净额 100、税额 13
    let out = tax_split(SplitArgs {
        amount: 113.0,
        rate: 13.0,
        tax_included: Some(true),
    })
    .unwrap();
    assert_eq!(out.gross, 113.0);
    assert!((out.net - 100.0).abs() < 1e-9);
    assert!((out.tax - 13.0).abs() < 1e-9);
    // 100 元不含税 13%：含税 113
    let out = tax_split(SplitArgs {
        amount: 100.0,
        rate: 13.0,
        tax_included: Some(false),
    })
    .unwrap();
    assert_eq!(out.gross, 113.0);
    // 0% 税率合法
    let out = tax_split(SplitArgs {
        amount: 100.0,
        rate: 0.0,
        tax_included: None,
    })
    .unwrap();
    assert_eq!(out.tax, 0.0);
}

#[test]
fn rejects_bad_split_inputs() {
    assert_eq!(
        tax_split(SplitArgs {
            amount: -1.0,
            rate: 13.0,
            tax_included: None
        })
        .unwrap_err()
        .code,
        "invoice.bad_amount"
    );
    assert_eq!(
        tax_split(SplitArgs {
            amount: 100.0,
            rate: 12.0,
            tax_included: None
        })
        .unwrap_err()
        .code,
        "invoice.bad_rate"
    );
}

#[test]
fn uppercase_covers_typical_amounts() {
    let cases: Vec<(f64, &str)> = vec![
        (0.0, "零元整"),
        (1.0, "壹元整"),
        (10.0, "壹拾元整"),
        (110.0, "壹佰壹拾元整"),
        (1234.56, "壹仟贰佰叁拾肆元伍角陆分"),
        (100.5, "壹佰元伍角整"),
        (0.05, "伍分"),
        (10000.0, "壹万元整"),
        (100_010_000.0, "壹亿零壹万元整"),
        (1234567.89, "壹佰贰拾叁万肆仟伍佰陆拾柒元捌角玖分"),
    ];
    for (amount, expected) in cases {
        let out = uppercase(amount).unwrap();
        assert!(out.valid, "{amount}");
        assert_eq!(out.text, expected, "amount {amount}");
    }
    let out = uppercase(123.456).unwrap();
    assert!(!out.valid);
}

#[test]
fn reimbursement_builds_markdown() {
    let out = reimbursement(ReimbursementArgs {
        title: "差旅报销".into(),
        person: "张三".into(),
        items: vec![
            ItemArgs {
                name: "火车票".into(),
                amount: 113.0,
                rate: 9.0,
                category: Some("交通".into()),
            },
            ItemArgs {
                name: "住宿".into(),
                amount: 226.0,
                rate: 6.0,
                category: None,
            },
        ],
    })
    .unwrap();
    assert_eq!(out.count, 2);
    assert!((out.total - 339.0).abs() < 1e-9);
    assert!(out.markdown.contains("# 差旅报销"));
    assert!(out.markdown.contains("报销人：张三"));
    assert!(out.markdown.contains("大写：叁佰叁拾玖元整"));
    assert!(out.markdown.contains("| 1 | 火车票 | 交通 |"));
}

#[test]
fn reimbursement_rejects_empty() {
    let err = reimbursement(ReimbursementArgs {
        title: "t".into(),
        person: "p".into(),
        items: vec![],
    })
    .unwrap_err();
    assert_eq!(err.code, "invoice.empty_items");
}

#[test]
fn dispatches_via_plugin() {
    let tool = InvoiceAssistant::default();
    let out = tool.call("uppercase", json!({"amount": 1234.56})).unwrap();
    assert_eq!(out["text"], "壹仟贰佰叁拾肆元伍角陆分");
    assert_eq!(
        tool.call("nope", json!({})).unwrap_err().code,
        "plugin.function_not_found"
    );
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&InvoiceAssistant::default()).unwrap();
    assert_eq!(checked, 3);
}
