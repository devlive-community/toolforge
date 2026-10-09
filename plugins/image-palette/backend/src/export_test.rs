use super::*;

const COLORS: [[u8; 3]; 2] = [[255, 0, 0], [0, 128, 255]];

#[test]
fn exports_common_formats() {
    assert_eq!(
        export(&COLORS, Format::Css, "brand"),
        ":root {\n  --brand-100: #ff0000;\n  --brand-200: #0080ff;\n}"
    );
    assert_eq!(
        export(&COLORS, Format::Scss, "brand"),
        "$brand-100: #ff0000;\n$brand-200: #0080ff;"
    );
    assert_eq!(export(&COLORS, Format::Hex, "x"), "#ff0000\n#0080ff");
    assert_eq!(
        export(&COLORS, Format::Json, "x"),
        "[\n  \"#ff0000\",\n  \"#0080ff\"\n]"
    );
    let tailwind = export(&COLORS, Format::Tailwind, "brand");
    assert!(
        tailwind.contains("brand: {") && tailwind.contains("100: '#ff0000',"),
        "{tailwind}"
    );
    let gpl = export(&COLORS, Format::Gpl, "brand");
    assert!(gpl.starts_with("GIMP Palette\nName: brand\n"));
    assert!(gpl.ends_with("  0 128 255\t#0080ff"));
}
