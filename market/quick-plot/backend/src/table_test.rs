use super::*;

fn auto(text: &str) -> Table {
    parse(text, Format::Auto, HeaderMode::Auto).unwrap()
}

fn names(table: &Table) -> Vec<&str> {
    table.columns.iter().map(|c| c.name.as_str()).collect()
}

fn kinds(table: &Table) -> Vec<Kind> {
    table.columns.iter().map(|c| c.kind).collect()
}

#[test]
fn detects_delimiters_and_headers() {
    let cases = [
        ("name,qps,p99\na,100,2.5\nb,200,3.1\n", Delimiter::Comma),
        ("name\tqps\tp99\na\t100\t2.5\nb\t200\t3.1\n", Delimiter::Tab),
        ("name;qps;p99\na;100;2,5\nb;200;3,1\n", Delimiter::Semicolon),
        ("name|qps|p99\na|100|2.5\nb|200|3.1\n", Delimiter::Pipe),
        (
            "name   qps   p99\na      100   2.5\nb      200   3.1\n",
            Delimiter::Whitespace,
        ),
    ];
    for (text, delimiter) in cases {
        let table = auto(text);
        assert_eq!(table.delimiter, Some(delimiter), "{text:?}");
        assert!(table.header);
        assert_eq!(names(&table), ["name", "qps", "p99"]);
        assert_eq!(table.rows, 2);
        assert_eq!(table.columns[1].kind, Kind::Number);
    }
    // 分号分隔时逗号是小数点的写法不认作数值
    assert_eq!(auto("a;b\n1;2,5\n3;4,5\n").columns[1].kind, Kind::Text);
}

#[test]
fn handles_headerless_and_quoted_csv() {
    let table = auto("1,10\n2,20\n3,30\n");
    assert!(!table.header);
    assert_eq!(names(&table), ["#1", "#2"]);
    assert_eq!(table.rows, 3);
    let quoted = auto("label,value\n\"a, b\",1\n\"say \"\"hi\"\"\",2\n");
    assert_eq!(quoted.columns[0].raw, ["a, b", "say \"hi\""]);
    // 强制有 / 无表头
    let forced = parse("x,y\n1,2\n", Format::Csv, HeaderMode::No).unwrap();
    assert_eq!((forced.header, forced.rows), (false, 2));
    let forced = parse("1,2\n3,4\n", Format::Csv, HeaderMode::Yes).unwrap();
    assert_eq!(names(&forced), ["1", "2"]);
}

#[test]
fn pads_ragged_rows_and_reports_them() {
    let table = auto("a,b,c\n1,2\n3,4,5\n");
    assert_eq!(table.columns[2].raw, ["", "5"]);
    assert_eq!(table.notes[0].code, "plot.ragged_rows");
}

#[test]
fn infers_column_types() {
    let text = "time,latency,status,epoch\n\
                2024-01-02 03:04:05,12.5,ok,1700000000\n\
                2024-01-02 03:04:06,null,ok,1700000001\n\
                2024-01-02 03:04:07,13%,fail,1700000002\n";
    let table = auto(text);
    assert_eq!(
        kinds(&table),
        [Kind::Time, Kind::Number, Kind::Text, Kind::Number]
    );
    assert_eq!(table.columns[1].values, [Some(12.5), None, Some(13.0)]);
    let ts = auto("ts,v\n1700000000,1\n1700000001,2\n");
    assert_eq!(ts.columns[0].kind, Kind::Time);
    assert_eq!(ts.columns[0].values[0], Some(1_700_000_000_000.0));
}

#[test]
fn flattens_json_arrays_with_dot_paths() {
    let text = r#"[
        {"name": "a", "stats": {"p50": 1, "p99": {"value": 9}}, "tags": ["x"], "ok": true},
        {"name": "b", "stats": {"p50": 2}, "extra": null}
    ]"#;
    let table = auto(text);
    assert_eq!(table.format, Format::Json);
    assert_eq!(
        names(&table),
        [
            "name",
            "stats.p50",
            "stats.p99.value",
            "tags",
            "ok",
            "extra"
        ]
    );
    assert_eq!(table.columns[1].kind, Kind::Number);
    assert_eq!(table.columns[2].raw, ["9", ""]);
    assert_eq!(table.columns[3].raw[0], "[\"x\"]");
    assert_eq!(table.columns[4].raw, ["true", ""]);
}

#[test]
fn finds_the_data_array_inside_api_responses() {
    let table =
        auto(r#"{"code": 0, "data": {"items": [{"t": "2024-01-01", "v": 3}], "total": 1}}"#);
    assert_eq!(table.json_path.as_deref(), Some("data.items"));
    assert_eq!(table.notes[0].code, "plot.json_path");
    assert_eq!(names(&table), ["t", "v"]);
    assert_eq!(table.columns[0].kind, Kind::Time);
    let err = parse(r#"{"a": 1}"#, Format::Auto, HeaderMode::Auto)
        .err()
        .unwrap();
    assert_eq!(err.code, "plot.json_no_array");
}

#[test]
fn reads_arrays_of_arrays_and_scalars() {
    assert_eq!(names(&auto("[[1, 2], [3, 4]]")), ["#1", "#2"]);
    let scalars = auto("[3, 1, 4]");
    assert_eq!(names(&scalars), ["value"]);
    assert_eq!(scalars.columns[0].values, [Some(3.0), Some(1.0), Some(4.0)]);
}

#[test]
fn parses_json_lines() {
    let text = "{\"t\": 1, \"v\": 2}\n\n{\"t\": 2, \"v\": 3, \"w\": 1}\n";
    let table = auto(text);
    assert_eq!(table.format, Format::Jsonl);
    assert_eq!(names(&table), ["t", "v", "w"]);
    assert_eq!(table.columns[2].raw, ["", "1"]);
    let err = parse("{\"a\":1}\n{oops}\n", Format::Jsonl, HeaderMode::Auto)
        .err()
        .unwrap();
    assert_eq!(
        (err.code.as_str(), err.params["line"].as_u64()),
        ("plot.jsonl_invalid", Some(2))
    );
    // 单个对象不是 JSON Lines
    assert_eq!(auto("{\"rows\": [{\"a\": 1}]}").format, Format::Json);
}

#[test]
fn reports_bad_input() {
    for (text, code) in [
        ("   \n", "plot.empty_input"),
        ("[1, 2", "plot.json_invalid"),
        ("[]", "plot.no_rows"),
    ] {
        assert_eq!(
            parse(text, Format::Auto, HeaderMode::Auto)
                .err()
                .unwrap()
                .code,
            code,
            "{text:?}"
        );
    }
    let err = parse("a,b\n\"unterminated,1\n", Format::Csv, HeaderMode::Auto);
    // 未闭合的引号按 CSV 规则吞掉后续内容，不 panic
    assert!(err.is_ok() || err.err().unwrap().code == "plot.csv_invalid");
}

#[test]
fn caps_rows() {
    let mut text = String::from("v\n");
    for i in 0..MAX_ROWS + 10 {
        text.push_str(&format!("{i}\n"));
    }
    let table = auto(&text);
    assert_eq!(table.rows, MAX_ROWS);
    assert!(table.notes.iter().any(|n| n.code == "plot.too_many_rows"));
}
