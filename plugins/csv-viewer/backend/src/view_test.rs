use super::*;
use crate::load::{finish, parse};

fn table() -> Table {
    let text = "name,age,city\nAda,36,London\nbob,,Paris\nÉmile,9,paris\nCleo,120,Cairo\n";
    let mut t = parse(text, b',', |_| {}, || false).unwrap();
    finish(&mut t, None);
    t
}

fn names(t: &Table, spec: Spec) -> Vec<String> {
    let view = build(t, &spec).unwrap();
    page(t, view.as_deref(), 0, 100)
        .rows
        .into_iter()
        .map(|r| r.cells[0].clone())
        .collect()
}

#[test]
fn sorts_numbers_and_text_with_empty_last() {
    let t = table();
    let by = |column, desc| Spec {
        sort: vec![SortKey { column, desc }],
        ..Default::default()
    };
    assert_eq!(names(&t, by(1, false)), vec!["Émile", "Ada", "Cleo", "bob"]);
    assert_eq!(names(&t, by(1, true)), vec!["Cleo", "Ada", "Émile", "bob"]);
    assert_eq!(names(&t, by(0, false)), vec!["Ada", "bob", "Cleo", "Émile"]);
    assert!(build(&t, &Spec::default()).unwrap().is_none());
}

#[test]
fn filters_rows() {
    let t = table();
    let query = |q: &str| {
        names(
            &t,
            Spec {
                query: q.into(),
                ..Default::default()
            },
        )
    };
    assert_eq!(query("PARIS"), vec!["bob", "Émile"]);
    assert_eq!(query("émi"), vec!["Émile"]);
    let filter = |column, op, value: &str| {
        names(
            &t,
            Spec {
                filters: vec![Filter {
                    column,
                    op,
                    value: value.into(),
                }],
                ..Default::default()
            },
        )
    };
    assert_eq!(filter(1, Op::Greater, "30"), vec!["Ada", "Cleo"]);
    assert_eq!(filter(1, Op::Less, "100"), vec!["Ada", "Émile"]);
    assert_eq!(filter(1, Op::Empty, ""), vec!["bob"]);
    assert_eq!(filter(2, Op::Equals, "Paris"), vec!["bob"]);
    assert_eq!(
        filter(2, Op::NotEquals, "Paris"),
        vec!["Ada", "Émile", "Cleo"]
    );
    assert_eq!(filter(2, Op::Contains, "AR"), vec!["bob", "Émile"]);
    let bad = Spec {
        sort: vec![SortKey {
            column: 9,
            desc: false,
        }],
        ..Default::default()
    };
    assert_eq!(build(&t, &bad).unwrap_err().code, "csv.invalid_column");
}

#[test]
fn pages_and_clips() {
    let mut t = Table::new();
    t.push_row(["h"]);
    for i in 0..5 {
        t.push_row([i.to_string().as_str()]);
    }
    let long = "x".repeat(5000);
    t.push_row([long.as_str()]);
    finish(&mut t, Some(true));
    let p = page(&t, None, 4, 10);
    assert_eq!((p.total, p.rows.len(), p.rows[0].index), (6, 2, 4));
    assert_eq!(p.rows[1].cells[0].chars().count(), 4097);
    assert!(page(&t, None, 50, 10).rows.is_empty());
}

#[test]
fn computes_column_stats() {
    let t = table();
    let s = stats(&t, None, 1).unwrap();
    assert_eq!((s.count, s.empty, s.distinct), (4, 1, 3));
    assert_eq!((s.min, s.max, s.sum), (Some(9.0), Some(120.0), Some(165.0)));
    assert_eq!(s.mean, Some(55.0));
    let s = stats(&t, None, 2).unwrap();
    assert_eq!(s.mean, None);
    assert_eq!((s.min_length, s.max_length), (Some(5), Some(6)));
    assert_eq!(
        s.top[0],
        ValueCount {
            value: "Cairo".into(),
            count: 1
        }
    );
    let view = build(
        &t,
        &Spec {
            query: "paris".into(),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(stats(&t, view.as_deref(), 2).unwrap().count, 2);
    assert_eq!(stats(&t, None, 7).unwrap_err().code, "csv.invalid_column");
}
