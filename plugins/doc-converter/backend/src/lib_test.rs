use super::*;
use crate::test_support::{Ctx, dir, workbook};

#[test]
fn converts_a_batch_without_overwriting() {
    let base = dir("batch");
    let input = base.join("book.xlsx");
    workbook(&input);
    std::fs::write(base.join("book.docx"), b"existing").unwrap();
    std::fs::write(base.join("notes.txt"), b"x").unwrap();
    let ctx = Ctx::default();
    let out = DocConverter::default()
        .run_task(
            "convert",
            json!({
                "mode": "excelToWord",
                "inputs": [input.to_string_lossy(), base.join("notes.txt").to_string_lossy()],
            }),
            &ctx,
        )
        .unwrap();
    assert_eq!(
        (out["converted"].as_u64(), out["failed"].as_u64()),
        (Some(1), Some(1))
    );
    assert_eq!(
        out["items"][0]["output"],
        base.join("book (1).docx").to_string_lossy().as_ref()
    );
    assert_eq!(out["items"][1]["error"]["code"], "doc.unsupported_input");
    assert_eq!(std::fs::read(base.join("book.docx")).unwrap(), b"existing");
    assert_eq!(ctx.0.lock().unwrap().last().unwrap(), "doc.done");
    std::fs::remove_dir_all(base).unwrap();
}

#[test]
fn rejects_bad_requests() {
    let ctx = Ctx::default();
    let err = |args: Value| {
        DocConverter::default()
            .run_task("convert", args, &ctx)
            .unwrap_err()
            .code
    };
    assert_eq!(
        err(json!({ "mode": "wordToPdf", "inputs": [] })),
        "doc.no_files"
    );
    assert_eq!(
        err(
            json!({ "mode": "excelToWord", "inputs": ["a.xlsx"], "outputDir": "/definitely/not/here" })
        ),
        "doc.output_dir_missing"
    );
    assert!(Mode::PdfToWord.accepts("pdf") && !Mode::PdfToWord.accepts("docx"));
    assert!(Mode::WordToPdf.accepts("doc") && Mode::ExcelToWord.accepts("xls"));
}
