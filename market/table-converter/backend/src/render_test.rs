use super::*;

fn sample() -> Table {
    Table {
        headers: vec!["名称".into(), "数量".into()],
        rows: vec![
            vec!["苹果".into(), "3".into()],
            vec!["a|b".into(), "10".into()],
        ],
        aligns: vec![Align::Left, Align::Right],
    }
}

#[test]
fn aligns_markdown_by_display_width() {
    let out = render(&sample(), Target::Markdown, &Options::default());
    assert_eq!(
        out,
        "| 名称 | 数量 |\n| :--- | ---: |\n| 苹果 |    3 |\n| a\\|b |   10 |"
    );
    let compact = render(
        &sample(),
        Target::Markdown,
        &Options {
            compact: true,
            ..Options::default()
        },
    );
    assert_eq!(compact.lines().nth(1), Some("| :-- | --: |"));
}

#[test]
fn writes_delimited_html_and_json() {
    assert_eq!(
        render(&sample(), Target::Csv, &Options::default()),
        "名称,数量\n苹果,3\na|b,10"
    );
    assert_eq!(
        render(&sample(), Target::Tsv, &Options::default()),
        "名称\t数量\n苹果\t3\na|b\t10"
    );
    let html = render(&sample(), Target::Html, &Options::default());
    assert!(html.contains("<th style=\"text-align: right\">数量</th>"));
    assert!(html.contains("<td style=\"text-align: left\">a|b</td>"));
    let json: serde_json::Value =
        serde_json::from_str(&render(&sample(), Target::Json, &Options::default())).unwrap();
    assert_eq!(json[1]["名称"], "a|b");
}

#[test]
fn draws_ascii_tables() {
    let out = render(&sample(), Target::Ascii, &Options::default());
    assert_eq!(
        out,
        "┌──────┬──────┐\n│ 名称 │ 数量 │\n├──────┼──────┤\n│ 苹果 │    3 │\n│ a|b  │   10 │\n└──────┴──────┘"
    );
}

#[test]
fn writes_sql_inserts() {
    let table = Table {
        headers: vec!["id".into(), "name".into(), "code".into()],
        rows: vec![
            vec!["1".into(), "O'Neil".into(), "007".into()],
            vec!["2.5".into(), "".into(), "-3".into()],
        ],
        aligns: Vec::new(),
    };
    let out = render(
        &table,
        Target::Sql,
        &Options {
            table_name: Some("users".into()),
            ..Options::default()
        },
    );
    assert_eq!(
        out,
        "INSERT INTO \"users\" (\"id\", \"name\", \"code\") VALUES\n  (1, 'O''Neil', '007'),\n  (2.5, NULL, -3);"
    );
}

#[test]
fn escapes_latex() {
    let table = Table {
        headers: vec!["a_b".into()],
        rows: vec![vec!["50% & $".into()]],
        aligns: vec![Align::Center],
    };
    let out = render(&table, Target::Latex, &Options::default());
    assert!(out.starts_with("\\begin{tabular}{c}"));
    assert!(out.contains("a\\_b \\\\") && out.contains("50\\% \\& \\$ \\\\"));
}
