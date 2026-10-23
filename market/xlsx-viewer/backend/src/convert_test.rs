use super::*;
use calamine::{Cell, Range};

fn range(rows: Vec<Vec<Data>>) -> Range<Data> {
    let sparse: Vec<Cell<Data>> = rows
        .into_iter()
        .enumerate()
        .flat_map(|(r, row)| {
            row.into_iter()
                .enumerate()
                .map(move |(c, data)| Cell::new((r as u32, c as u32), data))
        })
        .collect();
    Range::from_sparse(sparse)
}

#[test]
fn converts_primitive_cells() {
    use Kind::*;
    assert_eq!(cell(&Data::Empty), (String::new(), Empty));
    assert_eq!(cell(&Data::Int(7)), ("7".into(), Integer));
    assert_eq!(cell(&Data::Float(0.5)), ("0.5".into(), Float));
    assert_eq!(cell(&Data::Bool(true)), ("TRUE".into(), Boolean));
    assert_eq!(cell(&Data::String("hi".into())), ("hi".into(), Text));
}

#[test]
fn formats_numbers_like_excel() {
    assert_eq!(number(1.0), "1");
    assert_eq!(number(0.1 + 0.2), "0.3");
    assert_eq!(number(36.0), "36");
    assert_eq!(number(1.0 / 3.0), "0.3333333333");
    // 大整数保留浮点表示
    assert_eq!(number(1e16), "10000000000000000");
}

#[test]
fn converts_date_serials() {
    // 0 = 1899-12-30
    assert_eq!(date_text(0.0), "1899-12-30");
    assert_eq!(date_text(1.0), "1899-12-31");
    assert_eq!(date_text(45108.0), "2023-07-01");
    // 带时间
    assert_eq!(date_text(45108.5), "2023-07-01 12:00:00");
    assert_eq!(date_text(45108.25), "2023-07-01 06:00:00");
    // 纯时间（天数为 0）
    assert_eq!(date_text(0.25), "06:00:00");
}

#[test]
fn converts_durations() {
    assert_eq!(duration_text(0.0), "0:00:00");
    assert_eq!(duration_text(0.5), "12:00:00");
    // 超过 24 小时继续累计
    assert_eq!(duration_text(1.25), "30:00:00");
}

#[test]
fn builds_table_with_truncation_and_types() {
    let data = range(vec![
        vec![Data::String("name".into()), Data::String("age".into())],
        vec![Data::String("Ada".into()), Data::Int(36)],
        vec![Data::String("Bob".into()), Data::Int(9)],
    ]);
    let t = build_table(&data, true);
    assert_eq!(t.rows(), 2);
    assert_eq!(t.width(), 2);
    assert_eq!(t.cell(0, 0), "Ada");
    assert_eq!(t.columns[0].name.as_deref(), Some("name"));
    assert_eq!(t.columns[1].kind, Kind::Integer);
    // 表头是文本，但列类型仍按数据行推断为整数
    assert_eq!(t.columns[0].kind, Kind::Text);
}

#[test]
fn truncates_trailing_empty_columns_and_keeps_blank_rows() {
    // 尾部的两列全空：截断后只剩第 1 列；中间的空列不受影响
    let data = range(vec![
        vec![Data::String("a".into()), Data::Empty, Data::Empty],
        vec![Data::Empty, Data::Empty, Data::Empty],
        vec![Data::Int(1), Data::Empty, Data::Empty],
    ]);
    let t = build_table(&data, false);
    assert_eq!(t.width(), 1);
    // 空行保留：3 行
    assert_eq!(t.rows(), 3);
    assert_eq!(t.cell(1, 0), "");
    assert_eq!(t.cell(2, 0), "1");

    // 中间的空列保留
    let data = range(vec![vec![Data::Int(1), Data::Empty, Data::Int(2)]]);
    assert_eq!(build_table(&data, false).width(), 3);
}

#[test]
fn empty_sheet_builds_empty_table() {
    let data = range(vec![vec![Data::Empty]]);
    let t = build_table(&data, true);
    assert_eq!(t.rows(), 0);
    assert_eq!(t.width(), 0);
}

/// 流式构建与 Range 构建逐格一致：起点不在 A1、中间有空行、末列只在某些行有值
#[test]
fn streaming_matches_range_build() {
    use calamine::{Reader, Xlsx};
    use rust_xlsxwriter::Workbook;

    let mut wb = Workbook::new();
    let sheet = wb.add_worksheet();
    sheet.write(2, 1, "name").unwrap();
    sheet.write(2, 2, "age").unwrap();
    sheet.write(3, 1, "Ada").unwrap();
    sheet.write(3, 2, 36).unwrap();
    sheet.write(6, 1, "Bob").unwrap();
    sheet.write(6, 3, 7.5).unwrap();
    let bytes = wb.save_to_buffer().unwrap();

    for header in [true, false] {
        let mut xlsx: Xlsx<_> = Xlsx::new(std::io::Cursor::new(bytes.clone())).unwrap();
        let name = xlsx.sheet_names()[0].clone();
        let expected = build_table(&xlsx.worksheet_range(&name).unwrap(), header);
        let mut cells = xlsx.worksheet_cells_reader(&name).unwrap();
        let streamed = stream_table(&mut cells, header, || false).unwrap().unwrap();
        assert_eq!(streamed.columns, expected.columns, "header {header}");
        assert_eq!(streamed.rows(), expected.rows(), "header {header}");
        for row in 0..expected.rows() {
            for column in 0..expected.width() {
                assert_eq!(
                    streamed.cell(row, column),
                    expected.cell(row, column),
                    "({row}, {column})"
                );
            }
        }
    }
}
