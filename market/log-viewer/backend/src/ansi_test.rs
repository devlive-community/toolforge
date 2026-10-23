use super::*;

fn styled(text: &[u8]) -> Vec<(String, Style)> {
    runs(text)
        .into_iter()
        .map(|r| {
            (
                String::from_utf8_lossy(&text[r.start..r.end]).into_owned(),
                r.style,
            )
        })
        .collect()
}

#[test]
fn parses_basic_bright_and_reset() {
    let out = styled(b"\x1b[1;31mERROR\x1b[0m done \x1b[92mok\x1b[39m!");
    assert_eq!(out.len(), 4);
    assert_eq!(out[0].0, "ERROR");
    assert_eq!(
        out[0].1,
        Style {
            fg: Some(Color::Basic(1)),
            bold: true,
            ..Style::default()
        }
    );
    assert!(out[1].1.is_plain());
    assert_eq!(out[2].1.fg, Some(Color::Basic(10)));
    assert!(out[3].1.is_plain());
}

#[test]
fn parses_256_and_truecolor() {
    let out = styled(b"\x1b[38;5;208mA\x1b[48;5;244mB\x1b[38;2;255;0;128mC\x1b[38:5:3mD");
    assert_eq!(out[0].1.fg, Some(Color::Rgb("#ff8700".into())));
    assert_eq!(out[1].1.bg, Some(Color::Rgb("#808080".into())));
    assert_eq!(out[2].1.fg, Some(Color::Rgb("#ff0080".into())));
    assert_eq!(out[3].1.fg, Some(Color::Basic(3)));
}

#[test]
fn drops_other_control_sequences() {
    let out = styled(b"\x1b]0;title\x07a\x1b[2Kb\x1b(Bc\x1b[mx");
    let text: String = out.iter().map(|(t, _)| t.as_str()).collect();
    assert_eq!(text, "abcx");
    assert!(out.iter().all(|(_, s)| s.is_plain()));
}

#[test]
fn toggles_attributes_off() {
    let out = styled(b"\x1b[1;3;4;9;7ma\x1b[22;23;24;29;27mb");
    let first = &out[0].1;
    assert!(first.bold && first.italic && first.underline && first.strike && first.inverse);
    assert!(out[1].1.is_plain());
}
