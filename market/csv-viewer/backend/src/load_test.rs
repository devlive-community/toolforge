use super::*;

fn load(text: &str, header: Option<bool>) -> (Table, bool) {
    let delimiter = detect_delimiter(text);
    let mut table = parse(text, delimiter, |_| {}, || false).unwrap();
    let has_header = finish(&mut table, header);
    (table, has_header)
}

#[test]
fn decodes_common_encodings() {
    assert_eq!(
        decode(b"\xEF\xBB\xBFa,b".to_vec(), None).unwrap(),
        ("a,b".into(), "UTF-8".into())
    );
    // 「名称」的 GBK 编码
    assert_eq!(
        decode(b"\xC3\xFB\xB3\xC6,1".to_vec(), None).unwrap(),
        ("名称,1".into(), "gb18030".into())
    );
    let utf16: Vec<u8> = [0xFF, 0xFE]
        .into_iter()
        .chain("a\tb".encode_utf16().flat_map(u16::to_le_bytes))
        .collect();
    assert_eq!(
        decode(utf16, None).unwrap(),
        ("a\tb".into(), "UTF-16LE".into())
    );
    assert_eq!(decode(b"\xC3\xFB".to_vec(), Some("gbk")).unwrap().0, "名");
    assert_eq!(
        decode(vec![], Some("klingon")).unwrap_err().code,
        "csv.unknown_encoding"
    );
}

#[test]
fn detects_delimiters() {
    assert_eq!(detect_delimiter("a,b,c\n1,2,3\n"), b',');
    assert_eq!(detect_delimiter("a\tb\n1,5\t2\n"), b'\t');
    assert_eq!(detect_delimiter("a;b;c\n\"1;x\";2;3\n"), b';');
    assert_eq!(detect_delimiter("a|b\n1|2\n"), b'|');
    assert_eq!(detect_delimiter("single\nvalue\n"), b',');
    assert_eq!(parse_delimiter(Some("\\t")).unwrap(), Some(b'\t'));
    assert_eq!(parse_delimiter(Some("auto")).unwrap(), None);
    assert_eq!(
        parse_delimiter(Some("ab")).unwrap_err().code,
        "csv.invalid_delimiter"
    );
}

#[test]
fn parses_quotes_headers_and_kinds() {
    let (table, header) = load(
        "id,name,score,joined\n1,\"Lovelace, Ada\",9.5,2026-01-02\n2,\"say \"\"hi\"\"\n next\",,2026-02-03\n3,Bob\n",
        None,
    );
    assert!(header);
    assert_eq!(table.rows(), 3);
    assert_eq!(
        table
            .columns
            .iter()
            .map(|c| c.name.as_deref().unwrap())
            .collect::<Vec<_>>(),
        vec!["id", "name", "score", "joined"]
    );
    assert_eq!(
        table.columns.iter().map(|c| c.kind).collect::<Vec<_>>(),
        vec![Kind::Integer, Kind::Text, Kind::Float, Kind::Date]
    );
    assert_eq!(table.cell(0, 1), "Lovelace, Ada");
    assert_eq!(table.cell(1, 1), "say \"hi\"\n next");
    assert_eq!(table.cell(2, 3), "");
}

#[test]
fn header_detection_and_override() {
    let (table, header) = load("1,2\n3,4\n", None);
    assert!(!header);
    assert_eq!(table.rows(), 2);
    assert_eq!(table.columns[0].name, None);

    let (table, header) = load("a,a\nx,y\n", None);
    assert!(!header, "duplicate names are not a header");
    assert_eq!(table.rows(), 2);

    let (table, header) = load("1,2\n3,4\n", Some(true));
    assert!(header);
    assert_eq!(
        (table.rows(), table.columns[1].name.as_deref()),
        (1, Some("2"))
    );

    // 表头比数据宽或窄时，列数取最大值
    let (table, _) = load("a,b\n1,2,3\n", None);
    assert_eq!(table.width(), 3);
    assert_eq!(table.columns[2].name, None);
}

#[test]
fn cancels_large_parses() {
    let text = "a\n".repeat(20_000);
    assert_eq!(
        parse(&text, b',', |_| {}, || true).unwrap_err().code,
        "task.cancelled"
    );
    let mut seen = Vec::new();
    parse(&text, b',', |p| seen.push(p), || false).unwrap();
    assert_eq!(seen.last(), Some(&(text.len() as u64)));
}
