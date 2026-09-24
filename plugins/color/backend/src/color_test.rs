use super::*;

fn args(input: &str) -> Args {
    Args {
        input: input.into(),
        compare: String::new(),
    }
}

fn value<'a>(out: &'a Output, key: &str) -> &'a str {
    &out.formats.iter().find(|f| f.key == key).unwrap().value
}

#[test]
fn parses_css_and_platform_syntaxes() {
    for input in [
        "#ff0000",
        "ff0000",
        "red",
        "rgb(255 0 0)",
        "rgb(255, 0, 0)",
        "hsl(0 100% 50%)",
        "0xFF0000",
        "0xFFFF0000",
    ] {
        assert_eq!(
            parse(input).unwrap().to_rgba8(),
            [255, 0, 0, 255],
            "{input}"
        );
    }
    assert_eq!(parse("0x80FF0000").unwrap().to_rgba8(), [255, 0, 0, 128]);
    assert_eq!(parse("  ").unwrap_err().code, "color.empty");
    assert_eq!(parse("notacolor").unwrap_err().code, "color.invalid");
}

#[test]
fn converts_to_all_formats() {
    let out = convert(args("#3b82f6")).unwrap();
    assert_eq!(out.hex, "#3b82f6");
    assert_eq!(value(&out, "rgb"), "rgb(59 130 246)");
    assert_eq!(value(&out, "rgbLegacy"), "rgb(59, 130, 246)");
    assert_eq!(value(&out, "cmyk"), "cmyk(76% 47% 0% 4%)");
    assert_eq!(value(&out, "argb"), "0xFF3B82F6");
    assert!(value(&out, "oklch").starts_with("oklch("));
    assert!(out.name.is_none());

    let named = convert(args("rebeccapurple")).unwrap();
    assert_eq!(named.name, Some("rebeccapurple"));
    assert_eq!(named.formats[1].key, "name");
}

#[test]
fn alpha_is_kept() {
    let out = convert(args("rgba(0, 0, 0, 0.5)")).unwrap();
    assert_eq!(out.alpha, 0.5);
    assert_eq!(out.hex, "#000000");
    assert_eq!(value(&out, "rgbLegacy"), "rgba(0, 0, 0, 0.5)");
    assert_eq!(value(&out, "hex"), "#00000080");
}

#[test]
fn wcag_contrast() {
    let black = parse("#000").unwrap();
    let white = parse("#fff").unwrap();
    let c = contrast(&black, &white);
    assert_eq!(c.ratio, 21.0);
    assert!(c.aaa);
    let grey = contrast(&parse("#777").unwrap(), &white);
    assert_eq!(grey.ratio, 4.48);
    assert!(!grey.aa && grey.aa_large);

    let mut a = args("#777");
    a.compare = "#fff".into();
    let out = convert(a).unwrap();
    assert_eq!(out.compare.unwrap().ratio, 4.48);
    assert_eq!(out.compare_hex.as_deref(), Some("#ffffff"));

    let mut bad = args("#777");
    bad.compare = "nope".into();
    let err = convert(bad).unwrap_err();
    assert_eq!(
        (err.code.as_str(), &err.params["field"]),
        ("color.invalid", &serde_json::json!("compare"))
    );
}

#[test]
fn palettes() {
    let out = convert(args("#ff0000")).unwrap();
    assert_eq!(out.tints.len(), 9);
    assert_eq!(out.shades.len(), 9);
    // 越往后越接近白 / 黑
    assert!(luminance(&parse(&out.tints[8]).unwrap()) > luminance(&parse(&out.tints[0]).unwrap()));
    assert!(
        luminance(&parse(&out.shades[8]).unwrap()) < luminance(&parse(&out.shades[0]).unwrap())
    );
    let keys: Vec<_> = out
        .harmonies
        .iter()
        .map(|h| (h.key, h.colors.len()))
        .collect();
    assert_eq!(
        keys,
        vec![
            ("complementary", 2),
            ("analogous", 3),
            ("triadic", 3),
            ("tetradic", 4)
        ]
    );
    // 纯红与黑色对比更好，文字用黑色
    assert!(!out.dark);
    assert_eq!(out.text, "#000000");
}
