use std::io::Write;

use super::*;

fn write(name: &str, content: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("tfp-log-doc-{name}-{}", std::process::id()));
    std::fs::write(&path, content).unwrap();
    path
}

const SAMPLE: &str = "2026-09-26 10:00:00 INFO server started\n\
2026-09-26 10:00:01 DEBUG cache warm\n\
2026-09-26 10:00:02 ERROR request failed user=42\n\
\tat com.example.Api.handle(Api.java:10)\n\
2026-09-26 10:00:03 WARN slow request user=7\n\
2026-09-26 10:00:04 INFO {\"user\":42,\"ok\":false}\n";

fn filter(query: &str, levels: &[&str]) -> Filter {
    Filter {
        query: query.into(),
        levels: levels.iter().map(|l| l.to_string()).collect(),
        ..Filter::default()
    }
}

#[test]
fn opens_and_pages_lines() {
    let doc = Doc::open(&write("page", SAMPLE), |_, _| {}, || false).unwrap();
    let stats = doc.stats();
    assert_eq!(stats.lines, 6);
    assert_eq!(
        (stats.counts.error, stats.counts.info, stats.counts.warn),
        (2, 2, 1)
    );
    let page = doc.page(0, 2, 2).unwrap();
    assert_eq!(page.total, 6);
    assert_eq!(page.lines[0].n, 3);
    assert_eq!(page.lines[0].level, "error");
    assert_eq!(
        page.lines[1].text,
        "\tat com.example.Api.handle(Api.java:10)"
    );
    assert_eq!(page.lines[1].level, "error");
    assert!(doc.page(0, 10, 5).unwrap().lines.is_empty());
}

#[test]
fn filters_by_text_and_level_with_marks() {
    let doc = Doc::open(&write("filter", SAMPLE), |_, _| {}, || false).unwrap();
    let (view, total) = doc
        .filter(&filter("user=", &[]), |_, _| {}, &|| false)
        .unwrap();
    assert_eq!(total, 2);
    let page = doc.page(view, 0, 10).unwrap();
    assert_eq!(page.lines.iter().map(|l| l.n).collect::<Vec<_>>(), [3, 5]);
    assert_eq!(page.lines[0].marks, vec![[41, 46]]);

    let (view, total) = doc
        .filter(&filter("", &["error"]), |_, _| {}, &|| false)
        .unwrap();
    assert_eq!(total, 2);
    assert!(
        doc.page(view, 0, 10)
            .unwrap()
            .lines
            .iter()
            .all(|l| l.level == "error")
    );
    // 旧视图失效
    assert_eq!(
        doc.page(view - 1, 0, 10).unwrap_err().code,
        "log.view_expired"
    );

    let (cleared, total) = doc
        .filter(&Filter::default(), |_, _| {}, &|| false)
        .unwrap();
    assert_eq!((cleared, total), (0, 6));
}

#[test]
fn filters_large_files_in_parallel() {
    let mut content = String::new();
    for i in 0..50_000 {
        let level = if i % 10 == 0 { "ERROR" } else { "INFO" };
        content.push_str(&format!("line {i} {level} id={i}\n"));
    }
    let doc = Doc::open(&write("large", &content), |_, _| {}, || false).unwrap();
    let (view, total) = doc
        .filter(
            &filter(r"id=\d*7$", &["error", "info"]).with_regex(),
            |_, _| {},
            &|| false,
        )
        .unwrap();
    assert_eq!(total, 5000);
    let page = doc.page(view, 4999, 1).unwrap();
    assert_eq!(page.lines[0].n, 49_998);
    let cancelled = doc.filter(&filter("id", &[]), |_, _| {}, &|| true);
    assert_eq!(cancelled.unwrap_err().code, "task.cancelled");
}

#[test]
fn follows_growth_and_truncation() {
    let path = write("follow", "INFO one\nERROR two\nWARN par");
    let doc = Doc::open(&path, |_, _| {}, || false).unwrap();
    let (view, total) = doc
        .filter(&filter("", &["error", "warn"]), |_, _| {}, &|| false)
        .unwrap();
    assert_eq!(total, 2, "the partial tail line counts");
    assert!(!doc.refresh().unwrap().changed);

    std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(b"tial\nERROR three\nINFO four\n")
        .unwrap();
    let refreshed = doc.refresh().unwrap();
    assert!(refreshed.changed && !refreshed.reset);
    assert_eq!(refreshed.stats.lines, 5);
    assert_eq!(refreshed.view_total, Some(3));
    let texts: Vec<String> = doc
        .page(view, 0, 10)
        .unwrap()
        .lines
        .into_iter()
        .map(|l| l.text)
        .collect();
    assert_eq!(texts, ["ERROR two", "WARN partial", "ERROR three"]);

    std::fs::write(&path, "ERROR rotated\n").unwrap();
    let refreshed = doc.refresh().unwrap();
    assert!(refreshed.reset);
    assert_eq!((refreshed.stats.lines, refreshed.view_total), (1, Some(1)));
}

#[test]
fn shows_details_encodings_and_errors() {
    let doc = Doc::open(&write("detail", SAMPLE), |_, _| {}, || false).unwrap();
    let detail = doc.detail(6).unwrap();
    assert_eq!(detail.level, "info");
    assert!(detail.json.unwrap().contains("\"ok\": false"));
    assert_eq!(doc.detail(0).unwrap_err().code, "log.line_out_of_range");
    assert_eq!(doc.detail(99).unwrap_err().code, "log.line_out_of_range");

    let (gbk, _, _) = encoding_rs::GB18030.encode("ERROR 数据库连接失败\n");
    let path = std::env::temp_dir().join(format!("tfp-log-doc-gbk-{}", std::process::id()));
    std::fs::write(&path, &gbk).unwrap();
    let doc = Doc::open(&path, |_, _| {}, || false).unwrap();
    let (view, total) = doc
        .filter(&filter("连接", &[]), |_, _| {}, &|| false)
        .unwrap();
    assert_eq!(total, 1);
    assert_eq!(
        doc.page(view, 0, 1).unwrap().lines[0].text,
        "ERROR 数据库连接失败"
    );

    assert_eq!(
        Doc::open(Path::new("/definitely/missing.log"), |_, _| {}, || false)
            .err()
            .unwrap()
            .code,
        "fs.not_found"
    );
    assert_eq!(
        Doc::open(&std::env::temp_dir(), |_, _| {}, || false)
            .err()
            .unwrap()
            .code,
        "log.not_a_file"
    );
}

#[test]
fn truncates_very_long_lines() {
    let long = format!("INFO {}\n", "x".repeat(50_000));
    let doc = Doc::open(&write("long", &long), |_, _| {}, || false).unwrap();
    let line = &doc.page(0, 0, 1).unwrap().lines[0];
    assert!(line.truncated);
    assert_eq!(line.text.chars().count(), DISPLAY_CHARS);
    let detail = doc.detail(1).unwrap();
    assert!(!detail.truncated);
    assert_eq!(detail.text.len(), 50_005);
}

trait WithRegex {
    fn with_regex(self) -> Self;
}

impl WithRegex for Filter {
    fn with_regex(mut self) -> Self {
        self.regex = true;
        self
    }
}

/// `TOOLFORGE_LOG_SAMPLE=<大日志文件> cargo test --release -p tfp-log-viewer -- --ignored --nocapture`
#[test]
#[ignore]
fn benchmark_large_log() {
    let Some(path) = std::env::var_os("TOOLFORGE_LOG_SAMPLE") else {
        return;
    };
    let started = std::time::Instant::now();
    let doc = Doc::open(Path::new(&path), |_, _| {}, || false).unwrap();
    let stats = doc.stats();
    println!(
        "index: {} lines, {} bytes in {} ms",
        stats.lines,
        stats.bytes,
        started.elapsed().as_millis()
    );
    for (label, filter) in [
        ("level", filter("", &["error"])),
        ("text", filter("timeout", &[])),
        ("regex", filter(r"user=\d{3}7\b", &[]).with_regex()),
    ] {
        let started = std::time::Instant::now();
        let (view, total) = doc.filter(&filter, |_, _| {}, &|| false).unwrap();
        let page = std::time::Instant::now();
        doc.page(view, total / 2, 200).unwrap();
        println!(
            "{label}: {total} matches in {} ms, page in {} µs",
            started.elapsed().as_millis(),
            page.elapsed().as_micros()
        );
    }
}
