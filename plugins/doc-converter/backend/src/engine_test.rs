use super::*;
use crate::test_support::{Ctx, dir, document};

#[test]
fn uses_a_file_url_for_the_profile() {
    assert!(profile().starts_with("file:///"), "{}", profile());
    assert!(!candidates().is_empty());
}

/// 需要本机安装 LibreOffice：cargo test -p tfp-doc-converter -- --ignored
#[test]
#[ignore]
fn converts_with_libreoffice() {
    let engine = find().expect("LibreOffice is installed");
    let base = dir("engine");
    let input = base.join("名单.docx");
    document(&input, true);
    let ctx = Ctx::default();
    let pdf = convert(&engine, &input, &base, "pdf", None, &ctx).unwrap();
    assert!(std::fs::read(&pdf).unwrap().starts_with(b"%PDF"));
    let back_dir = base.join("back");
    std::fs::create_dir_all(&back_dir).unwrap();
    let docx = convert(
        &engine,
        &pdf,
        &back_dir,
        "docx:MS Word 2007 XML",
        Some("writer_pdf_import"),
        &ctx,
    )
    .unwrap();
    assert!(std::fs::read(&docx).unwrap().starts_with(b"PK"));
    std::fs::remove_dir_all(base).unwrap();
}
