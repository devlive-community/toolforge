use super::*;

#[test]
fn stores_cells_compactly_and_reads_back() {
    let mut t = Table::new();
    t.push_row(["a", "bb", ""]);
    t.push_row(["ccc"]);
    t.columns = vec![
        Column {
            name: None,
            kind: Kind::Text,
        },
        Column {
            name: None,
            kind: Kind::Text,
        },
        Column {
            name: None,
            kind: Kind::Empty,
        },
    ];
    assert_eq!((t.rows(), t.width()), (2, 3));
    assert_eq!(t.cell(0, 0), "a");
    assert_eq!(t.cell(0, 1), "bb");
    assert_eq!(t.cell(0, 2), "");
    assert_eq!(t.cell(1, 0), "ccc");
    // 缺失的单元格视为空
    assert_eq!(t.cell(1, 1), "");
    assert_eq!(t.cell(1, 2), "");
    assert_eq!(t.cell(1, 3), "");
    assert_eq!(t.row_len(0), 3);
    assert_eq!(t.row_len(1), 1);
}

#[test]
fn takes_first_row_as_header() {
    let mut t = Table::new();
    t.push_row(["h1", "h2"]);
    t.push_row(["x", "yy"]);
    t.push_row(["p", ""]);
    let header = t.take_first_row();
    assert_eq!(header, vec!["h1", "h2"]);
    assert_eq!(t.rows(), 2);
    assert_eq!(t.cell(0, 0), "x");
    assert_eq!(t.cell(1, 1), "");
}

#[test]
fn infers_column_kinds() {
    use Kind::*;
    // 一致的类型
    assert_eq!(infer_column([Integer, Integer, Empty]), Integer);
    // 整数与浮点混合归为浮点
    assert_eq!(infer_column([Integer, Float]), Float);
    // 混合其他类型是文本
    assert_eq!(infer_column([Integer, Date]), Text);
    assert_eq!(infer_column([Boolean, Text]), Text);
    // 全空
    assert_eq!(infer_column([Empty, Empty]), Empty);
    assert_eq!(infer_column([]), Empty);
}

#[test]
fn parses_numbers() {
    assert_eq!(parse_number("12"), Some(12.0));
    assert_eq!(parse_number(" -3.5 "), Some(-3.5));
    assert_eq!(parse_number(""), None);
    assert_eq!(parse_number("abc"), None);
    assert_eq!(parse_number("inf"), None);
}
