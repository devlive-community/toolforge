use super::*;

#[test]
fn detects_formats() {
    assert_eq!(detect("| a | b |\n|---|---|\n| 1 | 2 |"), Source::Markdown);
    assert_eq!(detect("a\tb\n1\t2"), Source::Tsv);
    assert_eq!(detect("a,b\n1,2"), Source::Csv);
    assert_eq!(detect("[{\"a\":1}]"), Source::Json);
    assert_eq!(
        detect("<p>x</p><TABLE><tr><td>1</td></tr></TABLE>"),
        Source::Html
    );
}

#[test]
fn parses_markdown_with_alignment_and_escapes() {
    let t = parse(
        "| 名称 | 数量 | 备注 |\n|:---|---:|:---:|\n| 苹果 | 3 | a \\| b |\n| 梨 | 10 |",
        Source::Markdown,
    )
    .unwrap();
    assert_eq!(t.headers, ["名称", "数量", "备注"]);
    assert_eq!(t.aligns, [Align::Left, Align::Right, Align::Center]);
    assert_eq!(t.rows[0], ["苹果", "3", "a | b"]);
    assert_eq!(t.rows[1], ["梨", "10"]);
}

#[test]
fn parses_csv_with_quotes_and_guesses_delimiters() {
    let t = parse(
        "name,note\n\"Ann\",\"likes \"\"tea\"\", coffee\"\nBob,",
        Source::Csv,
    )
    .unwrap();
    assert_eq!(t.rows[1], ["Ann", "likes \"tea\", coffee"]);
    let semi = parse("a;b;c\n1;2,5;3", Source::Csv).unwrap();
    assert_eq!(semi.rows[1], ["1", "2,5", "3"]);
    let tsv = parse("a\tb\n1\t", Source::Tsv).unwrap();
    assert_eq!(tsv.rows, [vec!["a", "b"], vec!["1", ""]]);
}

#[test]
fn parses_json_arrays() {
    let objects = parse(
        r#"[{"name":"Ann","age":31},{"name":"Bob","city":"Beijing","age":null}]"#,
        Source::Json,
    )
    .unwrap();
    assert_eq!(objects.headers, ["name", "age", "city"]);
    assert_eq!(objects.rows[0], ["Ann", "31", ""]);
    assert_eq!(objects.rows[1], ["Bob", "", "Beijing"]);
    let arrays = parse("[[1,\"a\"],[2,true]]", Source::Json).unwrap();
    assert_eq!(arrays.rows[1], ["2", "true"]);
    assert_eq!(
        parse("[1, 2]", Source::Json).unwrap_err().code,
        "table.unsupported_json"
    );
    assert_eq!(
        parse("[{\"a\":}]", Source::Json).unwrap_err().code,
        "table.invalid_json"
    );
}

#[test]
fn parses_html_tables() {
    let html = r#"<table class="x"><thead><tr><th>Name</th><th>Note</th></tr></thead>
      <tbody><tr><td><b>Ann</b></td><td>a &amp; b<br/>c &#20013;</td></tr><tr><td>Bob</td><td>  </td></tr></tbody></table>"#;
    let t = parse(html, Source::Html).unwrap();
    assert_eq!(t.headers, ["Name", "Note"]);
    assert_eq!(t.rows[0], ["Ann", "a & b c 中"]);
    assert_eq!(t.rows[1], ["Bob", ""]);
}

#[test]
fn reports_missing_tables() {
    assert_eq!(parse("  ", Source::Csv).unwrap_err().code, "table.empty");
    assert_eq!(
        parse("no table here", Source::Markdown).unwrap_err().code,
        "table.not_found"
    );
}
