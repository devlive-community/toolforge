use super::*;
use lopdf::{Dictionary, Document, Object, Stream};

fn name(s: &str) -> Object {
    Object::Name(s.as_bytes().to_vec())
}

/// 造一个多页 PDF：每页 612×792 点，用 Helvetica 写一行 48pt 的文字
fn write_pdf(path: &std::path::Path, texts: &[&str]) {
    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();
    let font_id = doc.add_object({
        let mut font = Dictionary::new();
        font.set("Type", name("Font"));
        font.set("Subtype", name("Type1"));
        font.set("BaseFont", name("Helvetica"));
        font
    });
    let mut kids = Vec::new();
    for text in texts {
        let content = format!("BT /F1 48 Tf 72 700 Td ({text}) Tj ET");
        let content_id = doc.add_object(Stream::new(Dictionary::new(), content.into_bytes()));
        let mut fonts = Dictionary::new();
        fonts.set("F1", font_id);
        let mut resources = Dictionary::new();
        resources.set("Font", fonts);
        let mut page = Dictionary::new();
        page.set("Type", name("Page"));
        page.set("Parent", pages_id);
        page.set(
            "MediaBox",
            vec![
                Object::Integer(0),
                Object::Integer(0),
                Object::Integer(612),
                Object::Integer(792),
            ],
        );
        page.set("Resources", resources);
        page.set("Contents", content_id);
        kids.push(Object::Reference(doc.add_object(page)));
    }
    let mut pages = Dictionary::new();
    pages.set("Type", name("Pages"));
    pages.set("Kids", Object::Array(kids));
    pages.set("Count", Object::Integer(texts.len() as i64));
    doc.objects.insert(pages_id, Object::Dictionary(pages));
    let mut catalog = Dictionary::new();
    catalog.set("Type", name("Catalog"));
    catalog.set("Pages", pages_id);
    let catalog_id = doc.add_object(catalog);
    doc.trailer.set("Root", catalog_id);
    doc.save(path).unwrap();
}

fn temp(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("tfp-ocr-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

#[test]
fn renders_letter_pages_with_ink() {
    let path = temp("hello.pdf");
    write_pdf(&path, &["Hello OCR"]);
    let doc = PdfDoc::open(path.to_str().unwrap()).unwrap();
    assert_eq!(doc.page_count(), 1);
    // 612 × 792 点 × 2.5 倍率
    let image = doc.render_page(0).unwrap();
    assert_eq!((image.width(), image.height()), (1530, 1980));
    // Helvetica 内置字体渲染出的文字，页面上应有非白像素
    assert!(
        image.as_raw().iter().any(|&b| b < 250),
        "the rendered page should not be blank"
    );
}

#[test]
fn counts_pages_and_rejects_out_of_range() {
    let path = temp("two.pdf");
    write_pdf(&path, &["first", "second"]);
    let doc = PdfDoc::open(path.to_str().unwrap()).unwrap();
    assert_eq!(doc.page_count(), 2);
    assert_eq!(doc.render_page(1).unwrap().width(), 1530);
    let err = doc.render_page(2).unwrap_err();
    assert_eq!(err.code, "ocr.pdf_page");
}

#[test]
fn rejects_broken_files() {
    let path = temp("broken.pdf");
    std::fs::write(&path, b"not a pdf at all").unwrap();
    let err = match PdfDoc::open(path.to_str().unwrap()) {
        Ok(_) => panic!("a broken file should not open"),
        Err(err) => err,
    };
    assert_eq!(err.code, "ocr.pdf_failed");
}
