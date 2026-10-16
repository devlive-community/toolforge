use super::*;
use crate::lcov::parse_text;

fn checksum(text: &str) -> String {
    BASE64_NOPAD.encode(&Md5::digest(text.as_bytes()))
}

#[test]
fn summarizes_files_and_directories() {
    let report = parse_text(
        "SF:/repo/src/a.rs\nDA:1,1\nDA:2,0\nend_of_record\nSF:/repo/src/util/b.rs\nDA:1,1\nend_of_record\nSF:/repo/build.rs\nDA:1,0\nend_of_record\n",
    );
    let o = overview(&report, 7);
    assert_eq!(o.token, 7);
    assert_eq!(o.common_prefix, "/repo/");
    assert_eq!((o.totals.lines.found, o.totals.lines.hit), (4, 2));
    let dir = |p: &str| o.dirs.iter().find(|d| d.path == p).unwrap();
    assert_eq!(
        (dir("/repo/src").files, dir("/repo/src").metrics.lines.hit),
        (2, 2)
    );
    assert_eq!(dir("/repo").files, 3);
    assert_eq!(dir("/").files, 3);
    assert_eq!(common_prefix(&["a.rs".into(), "b.rs".into()]), "");
    assert_eq!(parent_dirs("C:\\w\\x.rs"), ["C:/w", "C:"]);
}

#[test]
fn annotates_source_lines_and_finds_problems() {
    let source =
        "fn main() {\n    let a = 1;\n    // note\n    if a > 0 {\n        b();\n    }\n}\n";
    let lcov = format!(
        "SF:m.rs\nDA:1,1\nDA:2,1,{}\nDA:4,1,wrong\nDA:5,0\nDA:6,0\nDA:20,0\nBRDA:4,0,0,1\nBRDA:4,0,1,0\nFN:1,main\nFNDA:1,main\nend_of_record\n",
        checksum("    let a = 1;")
    );
    let report = parse_text(&lcov);
    let d = detail("m.rs", &report.files["m.rs"], Some(source));
    assert_eq!(d.lines.len(), 7);
    assert_eq!(d.lines[2].hits, None);
    assert_eq!(d.lines[3].branches, Some((1, 2)));
    assert_eq!(d.lines[4].text.as_deref(), Some("        b();"));
    assert_eq!(
        d.source_issues,
        [
            SourceIssue {
                kind: "checksumMismatch",
                line: 4
            },
            SourceIssue {
                kind: "beyondSource",
                line: 20
            },
        ]
    );
    // 5–6 连续未命中；20 之前隔着非可执行行，也并入同一段
    assert_eq!(d.uncovered, [Range { start: 5, end: 20 }]);
    assert_eq!(d.functions[0].name, "main");

    let bare = detail("m.rs", &report.files["m.rs"], None);
    assert_eq!(bare.lines.len(), 6);
    assert!(bare.source_issues.is_empty());
}

#[test]
fn exports_summaries() {
    let report = parse_text(
        "SF:/r/a.rs\nDA:1,1\nDA:2,0\nend_of_record\nSF:/r/b|c.rs\nDA:1,1\nend_of_record\n",
    );
    let csv = export(&report, ExportFormat::Csv).unwrap();
    assert_eq!(
        csv.lines().nth(1),
        Some("/r/a.rs,1,2,50.00%,0,0,N/A,0,0,N/A,0")
    );
    let md = export(&report, ExportFormat::Markdown).unwrap();
    assert!(md.contains("| **Total** | 66.67% (2/3) | N/A (0/0) | N/A (0/0) |"));
    assert!(md.contains("| `b\\|c.rs` | 100.00% | N/A | N/A |"));
}
