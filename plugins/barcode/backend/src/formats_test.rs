use super::*;

#[test]
fn computes_gs1_check_digits() {
    assert_eq!(gs1_check_digit("690123456789"), '2');
    assert_eq!(gs1_check_digit("400638133393"), '1');
    assert_eq!(gs1_check_digit("9638507"), '4');
    assert_eq!(gs1_check_digit("03600029145"), '2');
    assert_eq!(gs1_check_digit("1540014128876"), '3');
}

#[test]
fn completes_or_verifies_retail_codes() {
    let added = prepare(Format::Ean13, "690123456789").unwrap();
    assert_eq!(
        (added.content.as_str(), added.check_digit),
        ("6901234567892", Some('2'))
    );
    let given = prepare(Format::Ean13, "4006381333931").unwrap();
    assert_eq!(
        (given.content.as_str(), given.check_digit),
        ("4006381333931", None)
    );
    let wrong = prepare(Format::Ean13, "4006381333932").unwrap_err();
    assert_eq!(wrong.code, "barcode.check_digit");
    assert_eq!(wrong.params["expected"], "1");
    assert_eq!(
        prepare(Format::Ean8, "9638507").unwrap().content,
        "96385074"
    );
    assert_eq!(
        prepare(Format::UpcA, "03600029145").unwrap().content,
        "036000291452"
    );
    assert_eq!(
        prepare(Format::Ean13, "12345").unwrap_err().code,
        "barcode.length"
    );
    let letter = prepare(Format::Ean13, "69012345678A").unwrap_err();
    assert_eq!(
        (letter.code.as_str(), &letter.params["char"]),
        ("barcode.invalid_char", &serde_json::json!("A"))
    );
}

#[test]
fn handles_itf_lengths() {
    assert_eq!(
        prepare(Format::Itf, "1540014128876").unwrap().content,
        "15400141288763"
    );
    assert_eq!(prepare(Format::Itf, "123456").unwrap().content, "123456");
    assert_eq!(
        prepare(Format::Itf, "12345").unwrap_err().code,
        "barcode.itf_even"
    );
}

#[test]
fn checks_character_sets() {
    assert!(prepare(Format::Code39, "ABC-123").unwrap().notes.is_empty());
    assert_eq!(
        prepare(Format::Code39, "abc").unwrap().notes,
        ["code39Extended"]
    );
    assert_eq!(
        prepare(Format::Code128, "价格").unwrap_err().code,
        "barcode.invalid_char"
    );
    assert_eq!(
        prepare(Format::Code128, "Hello, World!").unwrap().content,
        "Hello, World!"
    );
    assert_eq!(
        prepare(Format::Codabar, "12345").unwrap().content,
        "A12345A"
    );
    assert_eq!(
        prepare(Format::Codabar, "b40156d").unwrap().content,
        "B40156D"
    );
    assert_eq!(
        prepare(Format::Codabar, "12X45").unwrap_err().code,
        "barcode.invalid_char"
    );
    assert_eq!(
        prepare(Format::DataMatrix, "任意文字 ✓").unwrap().content,
        "任意文字 ✓"
    );
    assert_eq!(
        prepare(Format::Aztec, "").unwrap_err().code,
        "barcode.empty"
    );
}
