use super::*;
use crate::test_support::{dir, workbook};

#[test]
fn formats_numbers_and_dates() {
    assert_eq!(number(0.1 + 0.2), "0.3");
    assert_eq!(number(1200.0), "1200");
    assert_eq!(number(-3.25), "-3.25");
    assert_eq!(excel_date(45931.0), "2025-10-01");
    assert_eq!(excel_date(45931.5), "2025-10-01 12:00:00");
    assert_eq!(excel_date(0.75), "18:00:00");
}

#[test]
fn reads_sheets_and_writes_word_tables() {
    let base = dir("excel");
    let input = base.join("book.xlsx");
    workbook(&input);
    let sheets = read(&input).unwrap();
    let names: Vec<&str> = sheets.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["销售", "Other"], "empty sheets are skipped");
    assert_eq!(sheets[0].1[1], ["北京", "0.3", "2025-10-01"]);
    assert_eq!(sheets[0].1[2], ["上海\n浦东", "1200", ""]);

    let output = base.join("book.docx");
    let count = to_word(
        &input,
        &output,
        &Options {
            header_row: true,
            sheet_titles: true,
        },
    )
    .unwrap();
    assert_eq!(count, 2);
    let crate::word::Content::Tables(tables) = crate::word::read(&output).unwrap() else {
        panic!("expected tables");
    };
    assert_eq!(tables.len(), 2);
    assert_eq!(tables[0][0], ["城市", "金额", "日期"]);
    assert_eq!(tables[0][2], ["上海\n浦东", "1200", ""]);
    std::fs::remove_dir_all(base).unwrap();
}

#[test]
fn rejects_empty_or_broken_workbooks() {
    let base = dir("excel-bad");
    let empty = base.join("empty.xlsx");
    let mut book = rust_xlsxwriter::Workbook::new();
    book.add_worksheet();
    book.save(&empty).unwrap();
    let options = Options {
        header_row: true,
        sheet_titles: true,
    };
    assert_eq!(
        to_word(&empty, &base.join("e.docx"), &options)
            .unwrap_err()
            .code,
        "doc.empty_workbook"
    );
    let broken = base.join("broken.xlsx");
    std::fs::write(&broken, b"not a workbook").unwrap();
    assert_eq!(
        to_word(&broken, &base.join("b.docx"), &options)
            .unwrap_err()
            .code,
        "doc.read_failed"
    );
    std::fs::remove_dir_all(base).unwrap();
}
