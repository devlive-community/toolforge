use super::*;

struct Ctx {
    index: usize,
    name: Name,
    captured: Option<jiff::Zoned>,
}

impl Context for Ctx {
    fn index(&self) -> usize {
        self.index
    }
    fn original(&self) -> &Name {
        &self.name
    }
    fn parent(&self) -> &str {
        "Holiday"
    }
    fn modified(&self) -> Option<jiff::Zoned> {
        Some("2026-09-26T08:30:00[UTC]".parse().unwrap())
    }
    fn captured(&self) -> Option<jiff::Zoned> {
        self.captured.clone()
    }
}

fn steps(json: serde_json::Value) -> Vec<Step> {
    serde_json::from_value(json).unwrap()
}

fn run(name: &str, index: usize, json: serde_json::Value) -> String {
    let rules = compile(&steps(json)).unwrap();
    apply(
        &rules,
        &Ctx {
            index,
            name: Name::parse(name),
            captured: None,
        },
    )
    .full()
}

#[test]
fn splits_names() {
    assert_eq!(
        Name::parse("photo.final.JPG"),
        Name {
            stem: "photo.final".into(),
            ext: "JPG".into()
        }
    );
    assert_eq!(
        Name::parse(".gitignore"),
        Name {
            stem: ".gitignore".into(),
            ext: String::new()
        }
    );
    assert_eq!(
        Name::parse("README"),
        Name {
            stem: "README".into(),
            ext: String::new()
        }
    );
    assert_eq!(Name::parse("archive.").full(), "archive.");
}

#[test]
fn replaces_text_and_regex() {
    let plain = serde_json::json!([{ "kind": "replace", "find": "IMG_", "replace": "$1 " }]);
    assert_eq!(run("img_0042.jpg", 0, plain), "$1 0042.jpg");
    let regex = serde_json::json!([{ "kind": "replace", "find": r"(\d+)", "replace": "#$1", "regex": true }]);
    assert_eq!(run("a1b22.txt", 0, regex), "a#1b#22.txt");
    let cased = serde_json::json!([{ "kind": "replace", "find": "a", "replace": "x", "caseSensitive": true, "part": "full" }]);
    assert_eq!(run("Aa.a", 0, cased), "Ax.x");
    let bad = compile(&steps(
        serde_json::json!([{ "kind": "replace", "find": "(", "regex": true }]),
    ));
    assert_eq!(bad.err().unwrap().code, "rename.invalid_regex");
    // 停用的规则不生效
    assert_eq!(
        run(
            "a.txt",
            0,
            serde_json::json!([{ "kind": "replace", "find": "a", "replace": "b", "enabled": false }])
        ),
        "a.txt"
    );
}

#[test]
fn inserts_removes_and_changes_case() {
    assert_eq!(
        run(
            "report.pdf",
            0,
            serde_json::json!([{ "kind": "insert", "text": "2026-", "place": "start" }])
        ),
        "2026-report.pdf"
    );
    assert_eq!(
        run(
            "report.pdf",
            0,
            serde_json::json!([{ "kind": "insert", "text": "_v2", "place": "end" }])
        ),
        "report_v2.pdf"
    );
    assert_eq!(
        run(
            "报告.pdf",
            0,
            serde_json::json!([{ "kind": "insert", "text": "年度", "place": "index", "index": 1 }])
        ),
        "报年度告.pdf"
    );
    assert_eq!(
        run(
            "DSC_0001.jpg",
            0,
            serde_json::json!([{ "kind": "remove", "place": "start", "count": 4 }])
        ),
        "0001.jpg"
    );
    assert_eq!(
        run(
            "draft-copy.md",
            0,
            serde_json::json!([{ "kind": "remove", "place": "end", "count": 5 }])
        ),
        "draft.md"
    );
    assert_eq!(
        run(
            "abcdef.txt",
            0,
            serde_json::json!([{ "kind": "remove", "place": "index", "index": 2, "count": 2 }])
        ),
        "abef.txt"
    );
    assert_eq!(
        run(
            "ab",
            0,
            serde_json::json!([{ "kind": "remove", "count": 9 }])
        ),
        ""
    );
    assert_eq!(
        run(
            "my summer_trip-day one.JPG",
            0,
            serde_json::json!([{ "kind": "case", "mode": "title" }])
        ),
        "My Summer_Trip-Day One.JPG"
    );
    assert_eq!(
        run(
            "HELLO World.TXT",
            0,
            serde_json::json!([{ "kind": "case", "mode": "sentence" }])
        ),
        "Hello world.TXT"
    );
    assert_eq!(
        run(
            "Mixed.TXT",
            0,
            serde_json::json!([{ "kind": "case", "mode": "lower", "part": "full" }])
        ),
        "mixed.txt"
    );
}

#[test]
fn numbers_and_templates() {
    let number = serde_json::json!([{ "kind": "number", "start": 10, "step": 5, "pad": 3, "place": "end", "separator": "_" }]);
    assert_eq!(run("scan.png", 2, number), "scan_020.png");
    let number = serde_json::json!([{ "kind": "number", "start": -1, "pad": 2, "separator": " " }]);
    assert_eq!(run("x.png", 0, number), "-01 x.png");
    let template = serde_json::json!([{ "kind": "template", "pattern": "{parent}_{date:%Y%m%d}_{n:3}_{name}" }]);
    assert_eq!(
        run("IMG_1.HEIC", 4, template),
        "Holiday_20260926_005_IMG_1.HEIC"
    );
    let exif = serde_json::json!([{ "kind": "template", "pattern": "{exif:%Y-%m-%d %H.%M}" }]);
    // 没有 EXIF 时使用修改时间
    assert_eq!(run("a.jpg", 0, exif.clone()), "2026-09-26 08.30.jpg");
    let rules = compile(&steps(exif)).unwrap();
    let ctx = Ctx {
        index: 0,
        name: Name::parse("a.jpg"),
        captured: Some("2024-01-02T03:04:05[UTC]".parse().unwrap()),
    };
    assert_eq!(apply(&rules, &ctx).full(), "2024-01-02 03.04.jpg");
    assert_eq!(
        run(
            "a.txt",
            0,
            serde_json::json!([{ "kind": "template", "pattern": "{name} (copy" }])
        ),
        "a (copy.txt"
    );
    for (pattern, code) in [
        ("{size}", "rename.unknown_token"),
        ("{n:x}", "rename.invalid_token"),
        ("{date:%}", "rename.invalid_token"),
    ] {
        let err = compile(&steps(
            serde_json::json!([{ "kind": "template", "pattern": pattern }]),
        ))
        .err()
        .unwrap();
        assert_eq!(err.code, code, "{pattern}");
    }
}

#[test]
fn changes_extensions_and_cleans() {
    assert_eq!(
        run(
            "a.JPEG",
            0,
            serde_json::json!([{ "kind": "extension", "mode": "lower" }])
        ),
        "a.jpeg"
    );
    assert_eq!(
        run(
            "a.jpeg",
            0,
            serde_json::json!([{ "kind": "extension", "mode": "set", "value": ".jpg" }])
        ),
        "a.jpg"
    );
    assert_eq!(
        run(
            "a.tmp",
            0,
            serde_json::json!([{ "kind": "extension", "mode": "remove" }])
        ),
        "a"
    );
    assert_eq!(
        run(
            "README",
            0,
            serde_json::json!([{ "kind": "extension", "mode": "set", "value": "md" }])
        ),
        "README.md"
    );
    let clean = serde_json::json!([{ "kind": "clean", "spacesTo": "_" }]);
    assert_eq!(run("  My   file: v1?  .txt", 0, clean), "My_file_v1.txt");
    let keep = serde_json::json!([{ "kind": "clean", "trim": false, "removeIllegal": false }]);
    assert_eq!(run(" a  b .txt", 0, keep), " a b .txt");
}

#[test]
fn applies_rules_in_order() {
    let json = serde_json::json!([
        { "kind": "replace", "find": " ", "replace": "-" },
        { "kind": "case", "mode": "lower" },
        { "kind": "number", "pad": 2, "separator": "-" },
        { "kind": "extension", "mode": "lower" }
    ]);
    assert_eq!(run("Team Photo.PNG", 1, json), "02-team-photo.png");
}
