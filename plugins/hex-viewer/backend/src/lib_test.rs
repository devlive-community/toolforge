use super::*;

fn fixture(name: &str) -> String {
    let path = std::env::temp_dir().join(format!("tfp-hex-lib-{name}-{}.png", std::process::id()));
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    bytes.extend(b"IHDR some text here and more");
    std::fs::write(&path, bytes).unwrap();
    path.to_string_lossy().into_owned()
}

fn call(function: &str, args: Value) -> PluginResult<Value> {
    HexViewer::default().call(function, args)
}

#[test]
fn opens_reads_and_inspects() {
    let path = fixture("open");
    let info = call("open", json!({ "path": path })).unwrap();
    assert_eq!(info["size"], 36);
    assert_eq!(info["kind"], "PNG image");
    assert_eq!(info["offsetWidth"], 8);
    // 偏移按行对齐
    let rows = call(
        "read",
        json!({ "path": path, "offset": 20, "length": 4096 }),
    )
    .unwrap();
    assert_eq!(rows[0]["offset"], 16);
    assert_eq!(rows.as_array().unwrap().len(), 2);
    let values = call(
        "inspect",
        json!({ "path": path, "offset": 8, "littleEndian": false }),
    )
    .unwrap();
    assert_eq!(values[0], json!({ "kind": "binary", "value": "01001001" }));
    std::fs::remove_file(path).unwrap();
}

#[test]
fn finds_text_and_hex() {
    let path = fixture("find");
    let found = call(
        "find",
        json!({ "path": path, "query": "TEXT", "mode": "text", "ignoreCase": true, "from": 0 }),
    )
    .unwrap();
    assert_eq!(found, json!({ "offset": 18, "length": 4 }));
    let none = call(
        "find",
        json!({ "path": path, "query": "TEXT", "mode": "text", "from": 0 }),
    )
    .unwrap();
    assert_eq!(none["offset"], Value::Null);
    let back = call(
        "find",
        json!({ "path": path, "query": "49 48 44 52", "from": 36, "backward": true }),
    )
    .unwrap();
    assert_eq!(back["offset"], 8);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn resolves_goto_expressions() {
    let goto = |expr: &str| {
        call(
            "goto",
            json!({ "expr": expr, "current": 0x100, "size": 0x1000 }),
        )
    };
    assert_eq!(goto("0x20").unwrap()["offset"], 0x20);
    assert_eq!(goto("1F0h").unwrap()["offset"], 0x1f0);
    assert_eq!(goto("4_000").unwrap()["offset"], 4000);
    assert_eq!(goto("+0x10").unwrap()["offset"], 0x110);
    assert_eq!(goto("-16").unwrap()["offset"], 0xf0);
    assert_eq!(goto("0x1000").unwrap_err().code, "hex.offset_out_of_range");
    assert_eq!(goto("-0x200").unwrap_err().code, "hex.offset_out_of_range");
    assert_eq!(goto("abc").unwrap_err().code, "hex.invalid_offset");
}

#[test]
fn copies_selections_in_several_formats() {
    let path = fixture("copy");
    let copy = |format: &str, length: u64| {
        call(
            "copy",
            json!({ "path": path, "offset": 0, "length": length, "format": format }),
        )
        .unwrap()["text"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    assert_eq!(copy("hex", 4), "89 50 4E 47");
    assert_eq!(copy("hexCompact", 4), "89504E47");
    assert_eq!(copy("base64", 4), "iVBORw==");
    assert_eq!(copy("python", 2), "b'\\x89\\x50'");
    assert_eq!(
        copy("c", 2),
        "const unsigned char data[2] = {\n    0x89, 0x50,\n};"
    );
    assert_eq!(copy("text", 4), "\u{fffd}PNG");
    assert_eq!(
        call(
            "copy",
            json!({ "path": path, "offset": 0, "length": 2_000_000, "format": "hex" })
        )
        .unwrap_err()
        .code,
        "hex.copy_too_large"
    );
    std::fs::remove_file(path).unwrap();
}
