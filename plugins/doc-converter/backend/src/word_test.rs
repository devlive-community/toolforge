use calamine::{Data, Reader, open_workbook};

use super::*;
use crate::test_support::{dir, document};

#[test]
fn keeps_codes_and_long_numbers_as_text() {
    assert_eq!(as_number("8500.5"), Some(8500.5));
    assert_eq!(as_number("-12"), Some(-12.0));
    assert_eq!(as_number("0.5"), Some(0.5));
    assert_eq!(as_number("007"), None);
    assert_eq!(as_number("110101199003077777"), None);
    assert_eq!(as_number("1,200"), None);
    assert_eq!(sheet_name("a/b:c*d?[e]"), "abcde");
}

#[test]
fn writes_tables_to_sheets() {
    let base = dir("word");
    let input = base.join("list.docx");
    document(&input, true);
    let output = base.join("list.xlsx");
    let options = Options {
        header_row: true,
        table_name: "表格".into(),
        text_name: "正文".into(),
    };
    assert_eq!(to_excel(&input, &output, &options).unwrap(), (1, false));
    let mut book: calamine::Xlsx<_> = open_workbook(&output).unwrap();
    assert_eq!(book.sheet_names(), ["表格 1"]);
    let range = book.worksheet_range("表格 1").unwrap();
    assert_eq!(range.get((1, 1)), Some(&Data::String("007".into())));
    assert_eq!(range.get((1, 2)), Some(&Data::Float(8500.5)));
    assert_eq!(
        range.get((2, 1)),
        Some(&Data::String("110101199003077777".into()))
    );
    assert_eq!(range.get((2, 2)), Some(&Data::Float(-12.0)));
    std::fs::remove_dir_all(base).unwrap();
}

#[test]
fn falls_back_to_paragraphs() {
    let base = dir("word-text");
    let input = base.join("text.docx");
    document(&input, false);
    let output = base.join("text.xlsx");
    let options = Options {
        header_row: true,
        table_name: "Table".into(),
        text_name: "Text".into(),
    };
    assert_eq!(to_excel(&input, &output, &options).unwrap(), (1, true));
    let mut book: calamine::Xlsx<_> = open_workbook(&output).unwrap();
    let range = book.worksheet_range("Text").unwrap();
    assert_eq!(range.get((1, 0)), Some(&Data::String("第二段".into())));
    std::fs::remove_dir_all(base).unwrap();
}
