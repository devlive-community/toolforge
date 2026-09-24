use super::*;

#[test]
fn stores_ragged_rows() {
    let mut table = Table::new();
    table.push_row(["a", "bb", ""]);
    table.push_row(["ccc"]);
    table.push_row(["", "d", "e", "f"]);
    assert_eq!(table.rows(), 3);
    assert_eq!((table.cell(0, 1), table.cell(0, 2)), ("bb", ""));
    assert_eq!((table.cell(1, 0), table.cell(1, 2)), ("ccc", ""));
    assert_eq!(table.cell(2, 3), "f");

    assert_eq!(table.take_first_row(), vec!["a", "bb", ""]);
    assert_eq!(table.rows(), 2);
    assert_eq!(
        (table.cell(0, 0), table.cell(1, 1), table.cell(1, 3)),
        ("ccc", "d", "f")
    );
}

#[test]
fn infers_column_kinds() {
    assert_eq!(infer(["1", " -2 ", ""].iter()), Kind::Integer);
    assert_eq!(infer(["1", "2.5", "1e3"].iter()), Kind::Float);
    assert_eq!(infer(["TRUE", "no"].iter()), Kind::Boolean);
    assert_eq!(
        infer(["2026-01-02", "2026-01-03 10:00", "2026/02/01T1"].iter()),
        Kind::Date
    );
    assert_eq!(infer(["1", "x"].iter()), Kind::Text);
    assert_eq!(infer(["nan", "inf"].iter()), Kind::Text);
    assert_eq!(infer(["", " "].iter()), Kind::Empty);
    assert_eq!(parse_number(" 3.5 "), Some(3.5));
    assert_eq!(parse_number("abc"), None);
}
