use super::*;
use crate::load::{finish, parse};

fn table(text: &str) -> (Table, bool) {
    let mut t = parse(text, b',', |_| {}, || false).unwrap();
    let header = finish(&mut t, None);
    (t, header)
}

fn render(t: &Table, view: Option<&[u32]>, format: Format, header: bool) -> String {
    let mut out = Vec::new();
    write(t, view, format, header, &mut out, |_| {}, || false).unwrap();
    String::from_utf8(out).unwrap()
}

const TEXT: &str = "name,age,ok,name\n\"Ada, L\",36,yes,x\nBob,,no,y|z\n";

#[test]
fn exports_delimited_text() {
    let (t, header) = table(TEXT);
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
    let (t, header) = table(TEXT);
    assert_eq!(
        render(&t, None, Format::Json, header),
        "[\n  {\"name\":\"Ada, L\",\"age\":36,\"ok\":true,\"name_2\":\"x\"},\n  {\"name\":\"Bob\",\"age\":null,\"ok\":false,\"name_2\":\"y|z\"}\n]\n"
    );
    assert_eq!(render(&t, Some(&[]), Format::Json, header), "[]\n");
    let (plain, header) = table("1,2\n3,4\n");
    assert!(render(&plain, None, Format::Json, header).contains("{\"column1\":1,\"column2\":2}"));
}

#[test]
fn exports_markdown() {
    let (t, header) = table(TEXT);
    let md = render(&t, None, Format::Markdown, header);
    assert!(
        md.starts_with("| name | age | ok | name_2 |\n| --- | ---: | --- | --- |\n"),
        "{md}"
    );
    assert!(md.contains("| Bob |  | no | y\\|z |"), "{md}");
}

#[test]
fn reports_progress_and_cancels() {
    let text = format!("a\n{}", "1\n".repeat(25_000));
    let (t, header) = table(&text);
    let mut seen = Vec::new();
    write(
        &t,
        None,
        Format::Csv,
        header,
        &mut Vec::new(),
        |n| seen.push(n),
        || false,
    )
    .unwrap();
    assert_eq!(seen, vec![0, 10_000, 20_000, 25_000]);
    let err = write(
        &t,
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
