use super::*;

fn style() -> Style {
    serde_json::from_value(serde_json::json!({})).unwrap()
}

#[test]
fn draws_ean13_with_guards_and_digit_groups() {
    let symbol = encode(Format::Ean13, "6901234567892").unwrap();
    let Symbol::Linear(bars) = &symbol else {
        panic!("linear")
    };
    assert_eq!(bars.len(), 95);
    let drawing = draw(Format::Ean13, &symbol, "6901234567892", &style()).unwrap();
    assert_eq!(drawing.pixels.0, (95 + 20) * 3);
    assert!(drawing.svg.contains(r#"viewBox="0 0 115 "#));
    assert_eq!(drawing.svg.matches("<text").count(), 13);
    // 护线比普通条长 5 个模块
    assert!(drawing.svg.contains("v65h"), "{}", drawing.svg);
    let plain = draw(
        Format::Ean13,
        &symbol,
        "6901234567892",
        &Style {
            show_text: false,
            ..style()
        },
    )
    .unwrap();
    assert!(!plain.svg.contains("<text") && !plain.svg.contains("v65h"));
}

#[test]
fn draws_matrix_codes_and_escapes_text() {
    let symbol = encode(Format::DataMatrix, "hello").unwrap();
    let Symbol::Matrix { width, height, .. } = &symbol else {
        panic!("matrix")
    };
    assert_eq!((*width, *height), (12, 12));
    let drawing = draw(Format::DataMatrix, &symbol, "hello", &style()).unwrap();
    assert_eq!(drawing.pixels, (16 * 3, 16 * 3));
    let code = encode(Format::Code128, "a<b&c").unwrap();
    let svg = draw(Format::Code128, &code, "a<b&c", &style()).unwrap().svg;
    assert!(svg.contains(">a&lt;b&amp;c</text>"));
}

#[test]
fn validates_style() {
    let symbol = encode(Format::Code39, "A").unwrap();
    let bad = |s: Style| draw(Format::Code39, &symbol, "A", &s).unwrap_err().code;
    assert_eq!(
        bad(Style {
            scale: 0,
            ..style()
        }),
        "barcode.invalid_scale"
    );
    assert_eq!(
        bad(Style {
            height: 5,
            ..style()
        }),
        "barcode.invalid_height"
    );
    assert_eq!(
        bad(Style {
            margin: Some(99),
            ..style()
        }),
        "barcode.invalid_margin"
    );
    assert_eq!(
        bad(Style {
            dark: "red".into(),
            ..style()
        }),
        "barcode.invalid_color"
    );
    assert_eq!(parse_color("#ABC", "dark").unwrap(), "#abc");
}

#[test]
fn renders_png_at_scale() {
    let symbol = encode(Format::Ean8, "96385074").unwrap();
    let drawing = draw(Format::Ean8, &symbol, "96385074", &style()).unwrap();
    let png = png(&drawing).unwrap();
    let image = image::load_from_memory(&png).unwrap();
    assert_eq!((image.width(), image.height()), drawing.pixels);
}
