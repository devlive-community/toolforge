use super::*;
use crate::fixtures::{many, sample};

fn done(bytes: &[u8], limit: usize) -> Wat {
    match print(bytes, limit, &|| false) {
        Printed::Done(wat) => wat,
        _ => panic!("print failed"),
    }
}

#[test]
fn splits_functions() {
    let wat = done(&sample(), usize::MAX);
    assert!(!wat.truncated);
    assert!(wat.text.starts_with("(module"));
    assert_eq!(wat.functions.len(), 2);
    let add = wat.function(0).unwrap();
    assert!(add.starts_with("  (func $add"));
    assert!(add.contains("i32.add"));
    assert!(!add.contains("$run"));
    assert!(wat.function(1).unwrap().contains("call 0"));
    assert!(wat.function(2).is_none());
    // 导入的函数不是函数块
    assert!(!wat.function(0).unwrap().contains("import"));
}

#[test]
fn outline_folds_function_bodies() {
    let wat = done(&sample(), usize::MAX);
    let (outline, truncated) = wat.outline();
    assert!(!truncated);
    assert!(outline.contains("(func $add"));
    assert!(!outline.contains("i32.add"));
    assert!(outline.contains(";; … "));
    assert!(outline.contains("(export \"add\""));
}

#[test]
fn stops_at_the_budget() {
    let wat = done(&many(200), 2000);
    assert!(wat.truncated);
    assert!(wat.text.len() <= 2000);
    // 最后一个函数可能不完整，不计入
    assert!(wat.functions.len() < 200);
    for range in &wat.functions {
        assert!(wat.text[range.clone()].trim_end().ends_with(')'));
    }
}

#[test]
fn can_be_cancelled() {
    assert!(matches!(
        print(&many(2000), usize::MAX, &|| true),
        Printed::Cancelled
    ));
}

#[test]
fn reports_unprintable_modules() {
    let bytes = crate::fixtures::raw(&[1, 0x80]);
    assert!(matches!(
        print(&bytes, usize::MAX, &|| false),
        Printed::Failed(_)
    ));
}
