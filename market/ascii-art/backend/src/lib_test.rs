use super::*;

/// 生成一张纯色 PNG 图（供 data 参数使用，动态生成避免手写 base64 夹具）
fn png_data_uri(px: Rgba<u8>) -> String {
    let img = RgbaImage::from_pixel(4, 4, px);
    let mut buf = Vec::new();
    image::codecs::png::PngEncoder::new(Cursor::new(&mut buf))
        .write_image(img.as_raw(), 4, 4, image::ExtendedColorType::Rgba8)
        .expect("encode png");
    format!("data:image/png;base64,{}", BASE64.encode(&buf))
}

/// 调用 convert 并取回原始 JSON（ConvertOut 只有 Serialize，按字段解析）
fn convert_via_call(args: Value) -> PluginResult<Value> {
    AsciiArt::default().call("convert", args)
}

fn field<'a>(value: &'a Value, key: &str) -> &'a Value {
    value
        .get(key)
        .unwrap_or_else(|| panic!("missing field {key} in {value}"))
}

#[test]
fn convert_accepts_data_and_returns_bounded_preview() {
    let out = convert_via_call(serde_json::json!({
        "data": png_data_uri(Rgba([255, 255, 255, 255])),
        "width": 16,
    }))
    .expect("converts");
    assert_eq!(field(&out, "cols").as_u64(), Some(16));
    // 4:4 图、16 列、宽高比 0.5 → 8 行
    assert_eq!(field(&out, "rows").as_u64(), Some(8));
    assert_eq!(field(&out, "complete").as_bool(), Some(true));
    assert!(
        field(&out, "warnings")
            .as_array()
            .expect("warnings")
            .is_empty()
    );
    // 亮图取最亮符号，纯文本无控制字符
    let text = field(&out, "text").as_str().expect("text");
    assert!(text.lines().all(|line| line == "@@@@@@@@@@@@@@@@"));
    assert!(!text.contains('\u{1b}'));
}

#[test]
fn fetch_returns_the_full_text_and_ansi_variant() {
    let converted = convert_via_call(serde_json::json!({
        "data": png_data_uri(Rgba([255, 0, 0, 255])),
        "width": 8,
        "color": true,
    }))
    .expect("converts");
    assert_eq!(field(&converted, "complete").as_bool(), Some(true));
    let id = field(&converted, "id").as_str().expect("id").to_owned();
    let plugin = AsciiArt::default();
    let plain = plugin
        .call("fetch", serde_json::json!({ "id": id }))
        .expect("fetch plain");
    let ansi = plugin
        .call("fetch", serde_json::json!({ "id": id, "ansi": true }))
        .expect("fetch ansi");
    let plain_text = field(&plain, "text").as_str().expect("text");
    let ansi_text = field(&ansi, "text").as_str().expect("text");
    assert!(!plain_text.contains('\u{1b}'));
    assert!(ansi_text.contains("\u{1b}[38;2;255;0;0m"));
    assert!(ansi_text.lines().all(|line| line.ends_with("\u{1b}[0m")));
}

#[test]
fn fetch_with_unknown_id_reports_not_found() {
    let error = AsciiArt::default()
        .call("fetch", serde_json::json!({ "id": "a000000000000" }))
        .expect_err("unknown id");
    assert_eq!(error.code, "ascii.not_found");
}

#[test]
fn custom_glyphs_report_merges_and_skips() {
    let out = convert_via_call(serde_json::json!({
        "data": png_data_uri(Rgba([0, 0, 0, 255])),
        "width": 8,
        "chars": "##!!\n中",
    }))
    .expect("converts");
    let warnings = field(&out, "warnings").as_array().expect("warnings");
    assert!(warnings.contains(&serde_json::json!("duplicateChars")));
    assert!(warnings.contains(&serde_json::json!("skippedChars")));
    // 黑图取自定义梯度最浅符号（首个 #；梯度不以空格开头时首字符即最浅）
    let text = field(&out, "text").as_str().expect("text");
    assert!(text.lines().all(|line| line == "########"));
}

#[test]
fn invalid_inputs_carry_explicit_codes() {
    let plugin = AsciiArt::default();
    let error = plugin
        .call(
            "convert",
            serde_json::json!({ "data": png_data_uri(Rgba([255, 255, 255, 255])), "width": 600 }),
        )
        .expect_err("width above maximum");
    assert_eq!(error.code, "ascii.bad_width");
    let error = plugin
        .call(
            "convert",
            serde_json::json!({
                "data": png_data_uri(Rgba([255, 255, 255, 255])),
                "alphaMode": "fill",
                "bg": "zzzzzz",
            }),
        )
        .expect_err("invalid background");
    assert_eq!(error.code, "ascii.bad_bg");
    let error = plugin
        .call(
            "convert",
            serde_json::json!({ "data": png_data_uri(Rgba([255, 255, 255, 255])), "preset": "nope" }),
        )
        .expect_err("unknown preset");
    assert_eq!(error.code, "ascii.bad_params");
    let error = plugin
        .call("convert", serde_json::json!({}))
        .expect_err("missing input");
    assert_eq!(error.code, "ascii.bad_params");
}

#[test]
fn unknown_function_is_reported_verbatim() {
    let error = AsciiArt::default()
        .call("nope", serde_json::json!({}))
        .expect_err("unknown function");
    assert_eq!(error.code, "plugin.function_not_found");
}

#[test]
fn save_writes_the_cached_text() {
    let converted = convert_via_call(serde_json::json!({
        "data": png_data_uri(Rgba([255, 255, 255, 255])),
        "width": 8,
    }))
    .expect("converts");
    let id = field(&converted, "id").as_str().expect("id").to_owned();
    let mut path = std::env::temp_dir();
    path.push(format!("tfm-ascii-art-save-{}.txt", std::process::id()));
    let out = AsciiArt::default()
        .call(
            "save",
            serde_json::json!({ "id": id, "path": path.to_string_lossy() }),
        )
        .expect("saves");
    assert!(field(&out, "bytes").as_u64().expect("bytes") > 0);
    let written = std::fs::read_to_string(&path).expect("read back");
    assert_eq!(
        written,
        field(&converted, "text").as_str().expect("preview text")
    );
    std::fs::remove_file(&path).expect("cleanup");
}

#[test]
fn manifest_declares_cli_and_agent_for_convert_only() {
    use tf_plugin_api::interface::Channel;
    let manifest = AsciiArt::default().manifest;
    let convert = manifest.functions.get("convert").expect("convert declared");
    let declaration = convert.declaration();
    let cli = declaration
        .exposed(Channel::Cli)
        .expect("convert exposes cli");
    assert!(matches!(cli.effect, tf_plugin_api::interface::Effect::Read));
    assert!(
        declaration.exposed(Channel::Agent).is_some(),
        "agent channel"
    );
    for name in ["prepare", "fetch", "save"] {
        let function = manifest.functions.get(name).expect("declared");
        let declaration = function.declaration();
        assert!(
            declaration.exposed(Channel::Cli).is_none(),
            "{name} must not be exposed to cli"
        );
    }
}
