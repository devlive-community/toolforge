use super::*;

fn options(text: &str) -> Options {
    Options {
        text: text.into(),
        ecc: Ecc::M,
        scale: 4,
        margin: 4,
        dark: default_dark(),
        light: default_light(),
    }
}

fn png_bytes(options: &Options) -> Vec<u8> {
    Matrix::new(options).unwrap().png().unwrap()
}

#[test]
fn generates_and_round_trips() {
    let text = "https://toolforge.dev/?q=二维码";
    let out = generate(options(text)).unwrap();
    assert!(out.image.starts_with("data:image/png;base64,"));
    assert_eq!(out.pixels, (out.modules as u32 + 8) * 4);
    assert_eq!(out.bytes, text.len());

    let decoded = decode_bytes(&png_bytes(&options(text))).unwrap();
    assert_eq!(decoded.len(), 1);
    assert_eq!(decoded[0].text, text);
    assert_eq!(decoded[0].version, out.version as usize);
}

#[test]
fn decodes_every_ecc_level() {
    for ecc in [Ecc::L, Ecc::M, Ecc::Q, Ecc::H] {
        let mut o = options("level");
        o.ecc = ecc;
        assert_eq!(decode_bytes(&png_bytes(&o)).unwrap()[0].ecc, ecc);
    }
}

#[test]
fn higher_ecc_needs_more_modules() {
    let text = "a".repeat(100);
    let mut low = options(&text);
    low.ecc = Ecc::L;
    let mut high = options(&text);
    high.ecc = Ecc::H;
    assert!(generate(high).unwrap().modules > generate(low).unwrap().modules);
}

#[test]
fn custom_colors_still_decode() {
    let mut o = options("hello");
    o.dark = "#1e3a8a".into();
    o.light = "#fef3c7".into();
    assert_eq!(decode_bytes(&png_bytes(&o)).unwrap()[0].text, "hello");
}

#[test]
fn parses_hex_colors() {
    assert_eq!(
        parse_hex("#fff", "dark").unwrap(),
        Rgba([255, 255, 255, 255])
    );
    assert_eq!(parse_hex("00000080", "dark").unwrap(), Rgba([0, 0, 0, 128]));
    let err = parse_hex("#12", "light").unwrap_err();
    assert_eq!(
        (err.code.as_str(), &err.params["field"]),
        ("qr.invalid_color", &serde_json::json!("light"))
    );
    assert_eq!(
        parse_hex("#gggggg", "dark").unwrap_err().code,
        "qr.invalid_color"
    );
}

#[test]
fn validates_options() {
    assert_eq!(generate(options("")).unwrap_err().code, "qr.empty");
    let mut o = options("x");
    o.scale = 0;
    assert_eq!(generate(o).unwrap_err().code, "qr.invalid_scale");
    let mut o = options("x");
    o.margin = 17;
    assert_eq!(generate(o).unwrap_err().code, "qr.invalid_margin");
    let mut o = options(&"x".repeat(4000));
    o.ecc = Ecc::H;
    assert_eq!(generate(o).unwrap_err().code, "qr.too_long");
}

#[test]
fn saves_png_and_svg() {
    let dir = std::env::temp_dir().join(format!("tfp-qr-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let png = dir.join("code.png");
    let svg = dir.join("code.svg");
    for (path, format) in [(&png, SaveFormat::Png), (&svg, SaveFormat::Svg)] {
        save(SaveArgs {
            options: options("save me"),
            format,
            path: path.to_string_lossy().into_owned(),
        })
        .unwrap();
    }
    let decoded = decode(DecodeArgs {
        path: Some(png.to_string_lossy().into_owned()),
        clipboard: false,
    })
    .unwrap();
    assert_eq!(decoded[0].text, "save me");
    let svg = std::fs::read_to_string(svg).unwrap();
    assert!(svg.starts_with("<svg") && svg.contains("fill=\"#000000\""));
}

#[test]
fn reports_missing_codes() {
    let blank = RgbaImage::from_pixel(64, 64, Rgba([255, 255, 255, 255]));
    let mut bytes = Vec::new();
    blank
        .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
        .unwrap();
    assert_eq!(decode_bytes(&bytes).unwrap_err().code, "qr.not_found");
    assert_eq!(decode_bytes(b"nope").unwrap_err().code, "qr.decode_failed");
    assert_eq!(
        decode(DecodeArgs {
            path: Some("/nope.png".into()),
            clipboard: false,
        })
        .unwrap_err()
        .code,
        "fs.not_found"
    );
}
