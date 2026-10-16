use super::*;

const SIMPLE: &str = r#"{
  "version": 3,
  "file": "app.min.js",
  "sources": ["src/a.ts", "src/b.ts"],
  "sourcesContent": ["line0\nline1\nline2\nline3\nline4", null],
  "names": ["doWork", "helper"],
  "mappings": "AAAA,IAAM;;ECAA,EAAE"
}"#;

#[test]
fn decodes_regular_mappings() {
    let data = parse_text(SIMPLE).unwrap();
    let Kind::Regular(map) = &data.kind else {
        panic!("expected regular map");
    };
    assert_eq!(map.segments, 4);
    assert_eq!(map.mapped_lines, 2);
    assert_eq!(map.per_source, vec![2, 2]);
    assert_eq!(data.file.as_deref(), Some("app.min.js"));

    // 段覆盖 [gen_col, 下一段 gen_col)：起点与区间内都命中同一段
    let hit = data.resolve(0, 0).unwrap();
    assert_eq!(hit.source, Some("src/a.ts"));
    assert_eq!(hit.source_index, Some(0));
    assert_eq!((hit.src_line, hit.src_col), (0, 0));
    let hit = data.resolve(0, 3).unwrap();
    assert_eq!((hit.src_line, hit.src_col), (0, 0));
    let hit = data.resolve(0, 5).unwrap();
    assert_eq!(hit.src_col, 6);

    let hit = data.resolve(2, 3).unwrap();
    assert_eq!(hit.source, Some("src/b.ts"));
    assert_eq!(hit.source_index, Some(1));

    // 空行与越界行没有映射
    assert!(data.resolve(1, 0).is_none());
    assert!(data.resolve(3, 0).is_none());
    // 该行第一段之前没有映射
    let second = &map.lines[2];
    assert_eq!(second[0].gen_col, 2);
    assert!(map.resolve(2, 1).is_none());
}

#[test]
fn accepts_string_version_and_resolves_names() {
    // version 允许字符串 "3"；五字段段带名称索引
    let data = parse_text(
        r#"{ "version": "3", "sources": ["a.js"], "names": ["doWork"], "mappings": "AAAAA" }"#,
    )
    .unwrap();
    let hit = data.resolve(0, 0).unwrap();
    assert_eq!(hit.name, Some("doWork"));
}

#[test]
fn rejects_bad_versions_and_broken_mappings() {
    assert_eq!(
        parse_text(&SIMPLE.replace("\"version\": 3", "\"version\": 2"))
            .unwrap_err()
            .code,
        "sourcemap.unsupported_version"
    );
    assert_eq!(
        parse_text(&SIMPLE.replace("\"version\": 3,", ""))
            .unwrap_err()
            .code,
        "sourcemap.unsupported_version"
    );
    assert_eq!(
        parse_text(&SIMPLE.replace(
            "\"mappings\": \"AAAA,IAAM;;ECAA,EAAE\"",
            "\"mappings\": null"
        ))
        .unwrap_err()
        .code,
        "sourcemap.mappings_missing"
    );
    assert_eq!(
        parse_text(&SIMPLE.replace("AAAA,IAAM;;ECAA,EAAE", "A!AA"))
            .unwrap_err()
            .code,
        "sourcemap.invalid_mappings"
    );
    // 来源下标越界：F = +5，超出 2 个来源
    assert_eq!(
        parse_text(&SIMPLE.replace("AAAA,IAAM;;ECAA,EAAE", "AKAA"))
            .unwrap_err()
            .code,
        "sourcemap.index_out_of_range"
    );
    // 名称下标越界：5 字段段的名称索引 +5，超出 2 个名称
    assert_eq!(
        parse_text(&SIMPLE.replace("AAAA,IAAM;;ECAA,EAAE", "AAAAK"))
            .unwrap_err()
            .code,
        "sourcemap.index_out_of_range"
    );
}

#[test]
fn parses_index_maps_with_sections() {
    let data = parse_text(
        r#"{
        "version": 3,
        "file": "bundle.js",
        "sections": [
            { "offset": { "line": 0, "column": 0 }, "map": { "version": 3, "sources": ["a.js"], "names": [], "mappings": "AAAA,EAAA" } },
            { "offset": { "line": 5, "column": 10 }, "map": { "version": 3, "sources": ["b.js"], "names": [], "mappings": "AAAA" } }
        ]
    }"#,
    )
    .unwrap();
    let Kind::Index(sections) = &data.kind else {
        panic!("expected index map");
    };
    assert_eq!(sections.len(), 2);
    assert_eq!(data.segments, 3);

    let hit = data.resolve(0, 0).unwrap();
    assert_eq!(hit.source, Some("a.js"));
    assert_eq!(hit.section, Some((0, 0)));
    let hit = data.resolve(0, 7).unwrap();
    assert_eq!(hit.src_line, 0);
    let hit = data.resolve(5, 15).unwrap();
    assert_eq!(hit.source, Some("b.js"));
    assert_eq!(hit.section, Some((5, 10)));
    // section 起始列之前、下一个 section 之后、内容行之外都没有映射
    assert!(data.resolve(5, 5).is_none());
    assert!(data.resolve(5, 10).is_some());
    assert!(data.resolve(4, 0).is_none());
    assert!(data.resolve(6, 0).is_none());
}

#[test]
fn skips_external_section_urls_with_a_diagnostic() {
    let data = parse_text(
        r#"{
        "version": 3,
        "sections": [
            { "offset": { "line": 0, "column": 0 }, "url": "https://example.com/partial.map" }
        ]
    }"#,
    )
    .unwrap();
    assert!(matches!(&data.kind, Kind::Index(list) if list.is_empty()));
    assert_eq!(data.diagnostics.len(), 1);
    assert_eq!(data.diagnostics[0].code, "sourcemap.section_url_skipped");
    assert!(data.resolve(0, 0).is_none());
}

#[test]
fn rejects_nested_sections_and_bad_offsets() {
    let nested = r#"{
        "version": 3,
        "sections": [
            { "offset": { "line": 0, "column": 0 }, "map": { "version": 3, "sections": [] } }
        ]
    }"#;
    assert_eq!(
        parse_text(nested).unwrap_err().code,
        "sourcemap.nested_sections"
    );
    let negative = r#"{
        "version": 3,
        "sections": [
            { "offset": { "line": -1, "column": 0 }, "map": { "version": 3, "sources": [], "names": [], "mappings": "" } }
        ]
    }"#;
    assert_eq!(
        parse_text(negative).unwrap_err().code,
        "sourcemap.invalid_offset"
    );
}

#[test]
fn enforces_limits() {
    let limits = Limits {
        max_segments: 2,
        max_sources: 2,
        max_names: 2,
    };
    assert_eq!(
        parse_text_with(SIMPLE, limits).unwrap_err().code,
        "sourcemap.too_many_segments"
    );
    let three_sources = r#"{
        "version": 3,
        "sources": ["a", "b", "c"],
        "names": [],
        "mappings": "AAAA"
    }"#;
    assert_eq!(
        parse_text_with(three_sources, limits).unwrap_err().code,
        "sourcemap.too_many_sources"
    );
}

#[test]
fn rejects_broken_json() {
    assert_eq!(
        parse_text("{oops").unwrap_err().code,
        "sourcemap.invalid_json"
    );
}
