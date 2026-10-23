use super::*;
use crate::test_support::{COLORS, encrypted, sample, workspace};

fn src(path: &str) -> Source {
    Source {
        path: path.into(),
        password: None,
    }
}

fn page(source: usize, page: u32, rotate: i64) -> PageRef {
    PageRef {
        source,
        page,
        rotate,
    }
}

/// 页面内容流中的填充颜色，用来识别是哪一页
fn colors(doc: &Document) -> Vec<String> {
    doc.get_pages()
        .values()
        .map(|id| {
            let content = String::from_utf8_lossy(&doc.get_page_content(*id)).into_owned();
            content.split(" rg").next().unwrap_or_default().to_owned()
        })
        .collect()
}

fn color(index: usize) -> String {
    let (r, g, b) = COLORS[index];
    format!("{r} {g} {b}")
}

#[test]
fn merges_reorders_and_rotates() {
    let dir = workspace("merge");
    let a = sample(&dir, "a.pdf", 4);
    let b = sample(&dir, "b.pdf", 2);
    let sources = [src(&a), src(&b)];
    let pages = [
        page(1, 2, 0),
        page(0, 1, 90),
        page(0, 3, 90),
        page(1, 1, -90),
    ];
    let doc = compose(&sources, &pages, &Options::default()).unwrap();
    let output = dir.join("out.pdf");
    let size = save(doc, &output).unwrap();
    assert!(size > 0);

    let reloaded = Document::load(&output).unwrap();
    assert_eq!(reloaded.get_pages().len(), 4);
    assert_eq!(colors(&reloaded), [color(1), color(0), color(2), color(0)]);
    let rotations: Vec<i64> = reloaded
        .get_pages()
        .values()
        .map(|id| source::page_rotation(&reloaded, *id))
        .collect();
    assert_eq!(
        rotations,
        [0, 90, 180, 270],
        "extra rotation adds to the inherited one"
    );
    // 继承的页面尺寸与字体随页面一起带过来
    let first = reloaded.get_pages()[&1];
    assert_eq!(source::page_size(&reloaded, first), (300.0, 400.0));
    assert!(source::inherited(&reloaded, first, b"Resources").is_some());
    assert_eq!(
        source::metadata(&reloaded).title,
        "b.pdf",
        "metadata comes from the first source used"
    );
}

#[test]
fn duplicates_pages_and_writes_metadata() {
    let dir = workspace("dup");
    let a = sample(&dir, "a.pdf", 2);
    let options = Options {
        metadata: Some(Metadata {
            title: "报告 Report".into(),
            author: "张三".into(),
            ..Metadata::default()
        }),
        compress: false,
    };
    let doc = compose(
        &[src(&a)],
        &[page(0, 1, 0), page(0, 1, 0), page(0, 2, 0)],
        &options,
    )
    .unwrap();
    let output = dir.join("dup.pdf");
    save(doc, &output).unwrap();
    let reloaded = Document::load(&output).unwrap();
    let ids: Vec<ObjectId> = reloaded.get_pages().values().copied().collect();
    assert_eq!(ids.len(), 3);
    assert_ne!(ids[0], ids[1], "a repeated page gets its own page object");
    let meta = source::metadata(&reloaded);
    assert_eq!(
        (
            meta.title.as_str(),
            meta.author.as_str(),
            meta.producer.as_str()
        ),
        ("报告 Report", "张三", "ToolForge")
    );
}

#[test]
fn drops_unused_pages_and_decrypts() {
    let dir = workspace("prune");
    let a = sample(&dir, "a.pdf", 4);
    let full = std::fs::metadata(&a).unwrap().len();
    let doc = compose(
        &[src(&a)],
        &[page(0, 4, 0)],
        &Options {
            compress: false,
            ..Options::default()
        },
    )
    .unwrap();
    let output = dir.join("one.pdf");
    let size = save(doc, &output).unwrap();
    assert!(size < full, "{size} >= {full}");
    let reloaded = Document::load(&output).unwrap();
    assert_eq!(colors(&reloaded), [color(3)]);

    let locked = encrypted(&dir, "e.pdf", "pw");
    let sources = [Source {
        path: locked,
        password: Some("pw".into()),
    }];
    let doc = compose(&sources, &[page(0, 2, 0)], &Options::default()).unwrap();
    let output = dir.join("unlocked.pdf");
    save(doc, &output).unwrap();
    let reloaded = Document::load(&output).unwrap();
    assert!(
        !reloaded.is_encrypted(),
        "the output is saved without encryption"
    );
    assert_eq!(colors(&reloaded), [color(1)]);
}

#[test]
fn rejects_bad_requests() {
    let dir = workspace("bad");
    let a = sample(&dir, "a.pdf", 2);
    let sources = [src(&a)];
    let code = |pages: &[PageRef]| {
        compose(&sources, pages, &Options::default())
            .unwrap_err()
            .code
    };
    assert_eq!(code(&[]), "pdf.no_pages_selected");
    assert_eq!(code(&[page(0, 3, 0)]), "pdf.page_out_of_range");
    assert_eq!(code(&[page(5, 1, 0)]), "pdf.invalid_source");
    assert_eq!(code(&[page(0, 1, 45)]), "pdf.invalid_rotation");
}

#[test]
fn parses_ranges_and_plans_splits() {
    assert_eq!(
        parse_ranges("1-3, 5，8-", 10).unwrap(),
        vec![(1, 3), (5, 5), (8, 10)]
    );
    assert_eq!(parse_ranges("-2", 10).unwrap(), vec![(1, 2)]);
    for bad in ["0", "3-1", "11", "a-b", "  "] {
        assert!(parse_ranges(bad, 10).is_err(), "{bad}");
    }
    assert_eq!(
        split_parts(Split::Every { size: 4 }, "", 10).unwrap(),
        vec![(1, 4), (5, 8), (9, 10)]
    );
    assert_eq!(
        split_parts(Split::Single, "", 3).unwrap(),
        vec![(1, 1), (2, 2), (3, 3)]
    );
    assert!(split_parts(Split::Every { size: 0 }, "", 3).is_err());

    let dir = workspace("names");
    let first = part_path(&dir, "report", (1, 3));
    assert!(first.ends_with("report_p1-3.pdf"));
    std::fs::write(&first, b"x").unwrap();
    assert!(part_path(&dir, "report", (1, 3)).ends_with("report_p1-3 (1).pdf"));
    assert!(part_path(&dir, "report", (7, 7)).ends_with("report_p7.pdf"));
}

/// 处理真实世界的 PDF：`TOOLFORGE_PDF_DIR=<含 book.pdf 与 apple-locked.pdf 的目录> cargo test -p tfp-pdf interop -- --ignored --nocapture`
#[test]
#[ignore]
fn interop() {
    let Some(dir) = std::env::var_os("TOOLFORGE_PDF_DIR") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    let book = Source {
        path: dir.join("book.pdf").to_string_lossy().into_owned(),
        password: None,
    };
    let locked = Source {
        path: dir.join("apple-locked.pdf").to_string_lossy().into_owned(),
        password: Some("user123".into()),
    };
    let sources = [book, locked];
    // 第 6、1 页（第 1 页旋转 90 度），再接加密文件的第 2 页
    let pages = [page(0, 6, 0), page(0, 1, 90), page(1, 2, 0)];
    let doc = compose(&sources, &pages, &Options::default()).unwrap();
    let size = save(doc, &dir.join("out-merged.pdf")).unwrap();
    println!("merged: {size} bytes");
    for (index, part) in split_parts(Split::Every { size: 4 }, "", 6)
        .unwrap()
        .into_iter()
        .enumerate()
    {
        let refs: Vec<PageRef> = (part.0..=part.1).map(|p| page(0, p, 0)).collect();
        let doc = compose(&sources[..1], &refs, &Options::default()).unwrap();
        save(doc, &dir.join(format!("out-part{}.pdf", index + 1))).unwrap();
    }
}
