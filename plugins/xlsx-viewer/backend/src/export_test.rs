use super::*;
use crate::table::{Column, Kind};

/// 四列：name / age / ok / name（重名），对应 csv 场景中已识别好类型的表格
fn table() -> (Table, bool) {
    let mut t = Table::new();
    t.push_row(["name", "age", "ok", "name"]);
    t.push_row(["Ada, L", "36", "yes", "x"]);
    t.push_row(["Bob", "", "no", "y|z"]);
    let header = t.take_first_row();
    t.columns = vec![
        Column {
            name: Some(header[0].clone()),
            kind: Kind::Text,
        },
        Column {
            name: Some(header[1].clone()),
            kind: Kind::Integer,
        },
        Column {
            name: Some(header[2].clone()),
            kind: Kind::Boolean,
        },
        Column {
            name: Some(header[3].clone()),
            kind: Kind::Text,
        },
    ];
    (t, true)
}

fn numbers() -> (Table, bool) {
    let mut t = Table::new();
    t.push_row(["1", "2"]);
    t.push_row(["3", "4"]);
    t.columns = vec![
        Column {
            name: None,
            kind: Kind::Integer,
        },
        Column {
            name: None,
            kind: Kind::Integer,
        },
    ];
    (t, false)
}

fn render(t: &Table, view: Option<&[u32]>, format: Format, header: bool) -> String {
    let mut out = Vec::new();
    write(t, view, format, header, &mut out, |_| {}, || false).unwrap();
    String::from_utf8(out).unwrap()
}

#[test]
fn exports_delimited_text() {
    let (t, header) = table();
    assert_eq!(
        render(&t, None, Format::Csv, header),
        "name,age,ok,name\n\"Ada, L\",36,yes,x\nBob,,no,y|z\n"
    );
    assert_eq!(
        render(&t, Some(&[1]), Format::Tsv, header),
        "name\tage\tok\tname\nBob\t\tno\ty|z\n"
    );
}

#[test]
fn exports_typed_json_with_unique_keys() {
    let (t, header) = table();
    assert_eq!(
        render(&t, None, Format::Json, header),
        "[\n  {\"name\":\"Ada, L\",\"age\":36,\"ok\":true,\"name_2\":\"x\"},\n  {\"name\":\"Bob\",\"age\":null,\"ok\":false,\"name_2\":\"y|z\"}\n]\n"
    );
    assert_eq!(render(&t, Some(&[]), Format::Json, header), "[]\n");
    let (plain, header) = numbers();
    assert!(render(&plain, None, Format::Json, header).contains("{\"column1\":1,\"column2\":2}"));
}

#[test]
fn exports_markdown() {
    let (t, header) = table();
    let md = render(&t, None, Format::Markdown, header);
    assert!(
        md.starts_with("| name | age | ok | name_2 |\n| --- | ---: | --- | --- |\n"),
        "{md}"
    );
    assert!(md.contains("| Bob |  | no | y\\|z |"), "{md}");
}

#[test]
fn reports_progress_and_cancels() {
    let (t, header) = numbers();
    let mut big = t;
    for i in 0..25_000 {
        big.push_row([i.to_string().as_str(), "0"]);
    }
    big.columns.iter_mut().for_each(|c| c.kind = Kind::Integer);
    let mut seen = Vec::new();
    write(
        &big,
        None,
        Format::Csv,
        header,
        &mut Vec::new(),
        |n| seen.push(n),
        || false,
    )
    .unwrap();
    // numbers() 已有 2 行，加上循环写入的 25000 行
    assert_eq!(seen, vec![0, 10_000, 20_000, 25_002]);
    let err = write(
        &big,
        None,
        Format::Csv,
        header,
        &mut Vec::new(),
        |_| {},
        || true,
    )
    .unwrap_err();
    assert_eq!(err.code, "task.cancelled");
}
