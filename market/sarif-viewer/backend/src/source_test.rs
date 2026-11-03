use super::*;
use crate::tests::{Context, parsed};
use std::io::Write;

fn location_row(uri: Option<&str>, base: Option<&str>, line: u64) -> LocationRow {
    LocationRow {
        id: 0,
        run: 0,
        result: 0,
        kind: "physical".into(),
        artifact: None,
        uri: uri.map(str::to_owned),
        uri_base_id: base.map(str::to_owned),
        region: Some(Region {
            start_line: Some(line),
            start_column: Some(3),
            end_line: Some(line),
            end_column: None,
        }),
        snippet: None,
        logical: None,
        message: None,
        order: None,
        nesting: None,
    }
}

fn base_report() -> Report {
    parsed(
        r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"s","rules":[]}},
        "originalUriBaseIds":{"SRCROOT":{"uri":"file:///nowhere/"}},"results":[]}]}"#,
    )
}

#[test]
fn percent_decode_handles_escapes_and_rejects_invalid() {
    assert_eq!(percent_decode("a%20b%2Fc"), Some("a b/c".into()));
    assert_eq!(percent_decode("%zz"), None);
    assert_eq!(percent_decode("%e2%98%83"), Some("☃".into()));
    assert_eq!(percent_decode("plain"), Some("plain".into()));
}

#[test]
fn resolve_prefers_file_uris_then_bases_then_root() {
    let report = base_report();
    // Absolute file URI is used as-is (still subject to containment).
    let (path, notes) = resolve(
        &report,
        &location_row(Some("file:///tmp/x/y.ts"), None, 1),
        "/root",
    )
    .unwrap();
    assert_eq!(path, "/tmp/x/y.ts");
    assert!(notes.is_empty());
    // Relative URI with a known base id joins the base.
    let (path, _) = resolve(
        &report,
        &location_row(Some("src/a.ts"), Some("SRCROOT"), 1),
        "/root",
    )
    .unwrap();
    assert_eq!(path, "/nowhere/src/a.ts");
    // Percent-encoded segments are decoded before joining.
    let (path, notes) = resolve(
        &report,
        &location_row(Some("src%2Fa%20b.ts"), Some("SRCROOT"), 1),
        "/root",
    )
    .unwrap();
    assert_eq!(path, "/nowhere/src/a b.ts");
    assert_eq!(notes, vec!["percent".to_owned()]);
    // Unknown base id falls back to the root with a note.
    let (path, notes) = resolve(
        &report,
        &location_row(Some("src/a.ts"), Some("OTHER"), 1),
        "/root",
    )
    .unwrap();
    assert_eq!(path, "/root/src/a.ts");
    assert_eq!(notes, vec!["baseUnknown".to_owned()]);
    // Non-file base falls back to the root with a note.
    let report = parsed(
        r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"s","rules":[]}},
        "originalUriBaseIds":{"WEB":{"uri":"https://example.com/src/"}},"results":[]}]}"#,
    );
    let (path, notes) = resolve(
        &report,
        &location_row(Some("a.ts"), Some("WEB"), 1),
        "/root",
    )
    .unwrap();
    assert_eq!(path, "/root/a.ts");
    assert_eq!(notes, vec!["baseNotLocal".to_owned()]);
}

#[test]
fn traversal_backslash_and_nul_are_rejected() {
    let report = base_report();
    for uri in ["../secret.ts", "a/../../b.ts", "a\\..\\b.ts", "a\0b"] {
        assert_eq!(
            resolve(
                &report,
                &location_row(Some(uri), Some("SRCROOT"), 1),
                "/root"
            )
            .err()
            .unwrap()
            .code,
            "sarif.escape",
            "uri {uri}"
        );
    }
    // Encoded traversal is decoded first, then caught.
    assert_eq!(
        resolve(
            &report,
            &location_row(Some("%2e%2e/secret"), Some("SRCROOT"), 1),
            "/root"
        )
        .err()
        .unwrap()
        .code,
        "sarif.escape"
    );
}

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("tf-sarif-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn read_returns_paged_lines_with_region_context() {
    let dir = scratch("read");
    let file = dir.join("src.ts");
    let mut out = std::fs::File::create(&file).unwrap();
    for i in 1..=20 {
        writeln!(out, "line {i}").unwrap();
    }
    drop(out);
    let mut report = base_report();
    let args = ReadSourceArgs {
        handle: "1".into(),
        location: 0,
        root: dir.to_string_lossy().into(),
        context: 2,
    };
    report
        .locations
        .push(location_row(Some("src.ts"), None, 10));
    let value = read(&report, &args, &Context::default()).unwrap();
    let lines = value["lines"].as_array().unwrap();
    assert_eq!(lines.len(), 5);
    assert_eq!(lines[0]["no"], 8);
    assert_eq!(lines[2]["text"], "line 10");
    assert_eq!(value["region"]["startLine"], 10);
    assert_eq!(value["columnKind"], "utf16CodeUnits");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn containment_symlinks_and_missing_files_are_enforced() {
    let dir = scratch("contain");
    let outside = scratch("outside");
    std::fs::write(dir.join("inside.ts"), "ok\n").unwrap();
    std::fs::write(outside.join("secret.ts"), "secret\n").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(outside.join("secret.ts"), dir.join("link.ts")).unwrap();
    let mut report = base_report();
    report
        .locations
        .push(location_row(Some("inside.ts"), None, 1));
    let args = ReadSourceArgs {
        handle: "1".into(),
        location: 0,
        root: dir.to_string_lossy().into(),
        context: 2,
    };
    assert!(read(&report, &args, &Context::default()).is_ok());
    #[cfg(unix)]
    {
        report.locations[0].uri = Some("link.ts".into());
        assert_eq!(
            read(&report, &args, &Context::default())
                .err()
                .unwrap()
                .code,
            "sarif.outside"
        );
    }
    report.locations[0].uri = Some("gone.ts".into());
    assert_eq!(
        read(&report, &args, &Context::default())
            .err()
            .unwrap()
            .code,
        "sarif.missing"
    );
    // Outside root is refused before any read.
    report.locations[0].uri = Some("/etc/passwd".into());
    assert_eq!(
        read(&report, &args, &Context::default())
            .err()
            .unwrap()
            .code,
        "sarif.outside"
    );
    std::fs::remove_dir_all(dir).unwrap();
    std::fs::remove_dir_all(outside).unwrap();
}

#[test]
fn missing_region_or_uri_refuses_to_guess() {
    let dir = scratch("noregion");
    std::fs::write(dir.join("a.ts"), "one\n").unwrap();
    let mut report = base_report();
    let args = ReadSourceArgs {
        handle: "1".into(),
        location: 0,
        root: dir.to_string_lossy().into(),
        context: 2,
    };
    report.locations.push(location_row(Some("a.ts"), None, 1));
    report.locations[0].region = None;
    assert_eq!(
        read(&report, &args, &Context::default())
            .err()
            .unwrap()
            .code,
        "sarif.region"
    );
    report.locations[0] = location_row(Some("a.ts"), None, 1);
    report.locations[0].uri = None;
    assert_eq!(
        read(&report, &args, &Context::default())
            .err()
            .unwrap()
            .code,
        "sarif.noUri"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn oversized_lines_are_flagged_not_executed() {
    let dir = scratch("longline");
    let long = "x".repeat(STRING_LIMIT + 100);
    std::fs::write(dir.join("a.ts"), format!("{long}\nsecond\n")).unwrap();
    let mut report = base_report();
    report.locations.push(location_row(Some("a.ts"), None, 1));
    let args = ReadSourceArgs {
        handle: "1".into(),
        location: 0,
        root: dir.to_string_lossy().into(),
        context: 0,
    };
    let value = read(&report, &args, &Context::default()).unwrap();
    assert_eq!(value["truncated"], true);
    assert_eq!(
        value["lines"][0]["text"].as_str().unwrap().chars().count(),
        STRING_LIMIT + 1
    );
    std::fs::remove_dir_all(dir).unwrap();
}
