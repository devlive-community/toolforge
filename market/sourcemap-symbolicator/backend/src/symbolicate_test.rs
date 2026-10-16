use super::*;
use crate::sourcemap;

const SIMPLE: &str = r#"{
  "version": 3,
  "file": "app.min.js",
  "sources": ["src/a.ts", "src/b.ts"],
  "sourcesContent": ["line0\nline1\nline2\nline3\nline4", null],
  "names": ["doWork", "helper"],
  "mappings": "AAAA,IAAM;;ECAA,EAAE"
}"#;

const NO_FILE: &str = r#"{
  "version": 3,
  "sources": ["src/a.ts"],
  "sourcesContent": ["only"],
  "names": [],
  "mappings": "AAAA"
}"#;

fn parse(text: &str) -> sourcemap::SourceMapData {
    sourcemap::parse_text(text).unwrap()
}

#[test]
fn resolves_frames_and_keeps_messages() {
    let data = parse(SIMPLE);
    let stack = "Uncaught TypeError: boom\n    at doWork (http://x/assets/app.min.js:1:9)";
    let result = run(&data, Some(stack), None, None).unwrap();
    // 关联产物取 map 的 file 字段
    assert_eq!(result.asset.as_deref(), Some("app.min.js"));
    assert_eq!(result.frames.len(), 2);
    assert_eq!(result.frames[0].kind, "message");
    assert_eq!(result.frames[0].status, "message");

    let frame = &result.frames[1];
    assert_eq!(frame.status, "resolved");
    assert_eq!(frame.function.as_deref(), Some("doWork"));
    let resolved = frame.resolved.as_ref().unwrap();
    assert_eq!(resolved.source.as_deref(), Some("src/a.ts"));
    assert_eq!(resolved.source_index, 0);
    // 生成列 9（1 基）→ 0 基 8 → 命中 gen_col 4 的段 → 源列 6（0 基）→ 展示 7
    assert_eq!((resolved.line, resolved.column), (1, 7));
    assert_eq!(resolved.name, None);

    // sourcesContent 存在时附上下文
    let context = resolved.context.as_ref().unwrap();
    assert_eq!(context.lines.len(), 3);
    assert_eq!(context.lines[0].text, "line0");
    assert_eq!(context.target, 0);
    assert_eq!(result.stats.resolved, 1);
    assert_eq!(result.stats.messages, 1);
}

#[test]
fn marks_unrelated_and_unmapped_frames() {
    let data = parse(SIMPLE);
    let stack = "    at a (http://x/assets/other.js:1:9)\n    at b (app.min.js:9:1)\n    at c (app.min.js:1:1)";
    let result = run(&data, Some(stack), None, None).unwrap();
    assert_eq!(result.frames[0].status, "unrelated");
    assert_eq!(
        result.frames[0].reason.as_ref().unwrap().code,
        "sourcemap.frame_unrelated"
    );
    assert_eq!(result.frames[1].status, "no_mapping");
    assert_eq!(
        result.frames[1].reason.as_ref().unwrap().code,
        "sourcemap.frame_no_mapping"
    );
    // 第一段之前的位置没有映射
    assert_eq!(result.frames[2].status, "resolved");
    assert_eq!(result.stats.unrelated, 1);
    assert_eq!(result.stats.no_mapping, 1);
    assert_eq!(result.stats.resolved, 1);
}

#[test]
fn keeps_frames_without_a_column() {
    let data = parse(SIMPLE);
    let result = run(&data, Some("doWork@app.min.js:12"), None, None).unwrap();
    let frame = &result.frames[0];
    assert_eq!(frame.status, "no_column");
    assert!(frame.resolved.is_none());
    assert_eq!(result.stats.no_column, 1);
}

#[test]
fn marks_frames_unassociated_without_asset() {
    let data = parse(NO_FILE);
    let result = run(&data, Some("at f (app.min.js:1:1)"), None, None).unwrap();
    assert_eq!(result.asset, None);
    assert_eq!(result.frames[0].status, "unassociated");
    assert_eq!(
        result.frames[0].reason.as_ref().unwrap().code,
        "sourcemap.frame_unassociated"
    );
}

#[test]
fn explicit_asset_overrides_the_file_field() {
    let data = parse(SIMPLE);
    let result = run(
        &data,
        Some("at f (webpack:///bundle.min.js:3:5)"),
        Some("bundle.min.js"),
        None,
    )
    .unwrap();
    assert_eq!(result.asset.as_deref(), Some("bundle.min.js"));
    assert_eq!(result.frames[0].status, "resolved");
    assert_eq!(
        result.frames[0]
            .resolved
            .as_ref()
            .unwrap()
            .source
            .as_deref(),
        Some("src/b.ts")
    );
}

#[test]
fn supports_single_position_mode() {
    let data = parse(SIMPLE);
    let result = run(&data, None, Some("app.min.js"), Some((1, 5))).unwrap();
    assert_eq!(result.frames.len(), 1);
    let frame = &result.frames[0];
    assert_eq!(frame.raw, "1:5");
    assert_eq!(frame.status, "resolved");
    let resolved = frame.resolved.as_ref().unwrap();
    assert_eq!((resolved.line, resolved.column), (1, 7));
}

#[test]
fn reports_missing_input() {
    let data = parse(SIMPLE);
    assert_eq!(
        run(&data, None, None, None).unwrap_err().code,
        "sourcemap.no_input"
    );
}

#[test]
fn caret_aligns_by_utf16_units() {
    // 👍 在 UTF-16 里占两个 code unit
    let text = "const \u{1F44D} = 1";
    assert_eq!(caret(text, 6), "      ^");
    assert_eq!(caret(text, 8), "        ^");
    // 超出宽度时收敛到行尾（宽度按 UTF-16 计）
    assert_eq!(caret(text, 999), format!("{}^", " ".repeat(12)));
}

#[test]
fn markdown_report_contains_frames_and_context() {
    let data = parse(SIMPLE);
    let stack = "at a (other.js:1:1)\nat doWork (app.min.js:1:9)";
    let result = run(&data, Some(stack), None, None).unwrap();
    let report = markdown(&result);
    assert!(report.contains("# Source Map Symbolication"));
    assert!(report.contains("## Frame 1"));
    assert!(report.contains("`sourcemap.frame_unrelated`"));
    assert!(report.contains("src/a.ts"));
    assert!(report.contains("line0"));
    assert!(report.contains('\u{21d2}'));
}

#[test]
fn json_export_round_trips() {
    let data = parse(SIMPLE);
    let result = run(&data, Some("at doWork (app.min.js:1:9)"), None, None).unwrap();
    let json = serde_json::to_string(&result).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["frames"][0]["status"], "resolved");
    assert_eq!(parsed["stats"]["resolved"], 1);
}

#[test]
fn truncates_long_stacks() {
    let data = parse(SIMPLE);
    let stack = vec!["at f (app.min.js:1:9)"; crate::MAX_FRAMES + 1].join("\n");
    let result = run(&data, Some(&stack), None, None).unwrap();
    assert_eq!(result.frames.len(), crate::MAX_FRAMES);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "sourcemap.too_many_frames")
    );
}
