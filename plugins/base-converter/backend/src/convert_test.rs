use super::*;

fn args(input: &str, from: u32) -> Args {
    Args {
        input: input.into(),
        from,
        custom: 36,
        uppercase: false,
        group: false,
        toggle: None,
    }
}

fn big(n: i128) -> BigInt {
    BigInt::from(n)
}

#[test]
fn parses_prefixes_signs_and_separators() {
    assert_eq!(parse("0xFF", 0).unwrap(), (big(255), 16));
    assert_eq!(parse(" -0b1010 ", 0).unwrap(), (big(-10), 2));
    assert_eq!(parse("0o17", 0).unwrap(), (big(15), 8));
    assert_eq!(parse("1_000,000", 0).unwrap(), (big(1_000_000), 10));
    assert_eq!(parse("dead beef", 16).unwrap(), (big(0xdead_beef), 16));
    // 显式进制与前缀一致时剥离前缀
    assert_eq!(parse("0x10", 16).unwrap(), (big(16), 16));
    assert_eq!(parse("zz", 36).unwrap(), (big(1295), 36));
}

#[test]
fn reports_invalid_input() {
    let err = parse("12a4", 10).unwrap_err();
    assert_eq!(err.code, "base.invalid_digit");
    assert_eq!(err.params["char"], "a");
    assert_eq!(err.params["position"], 3);
    assert_eq!(parse("-0x", 0).unwrap_err().code, "base.empty");
    assert_eq!(parse("   ", 0).unwrap_err().code, "base.empty");
    assert_eq!(parse("1", 37).unwrap_err().code, "base.invalid_base");
    assert_eq!(
        parse(&"9".repeat(MAX_DIGITS + 1), 10).unwrap_err().code,
        "base.too_long"
    );
    // 显式二进制下 0x 不是前缀
    assert_eq!(parse("0x1", 2).unwrap_err().code, "base.invalid_digit");
}

#[test]
fn converts_to_every_base() {
    let out = convert(args("255", 10)).unwrap();
    assert_eq!(
        (
            out.binary.as_str(),
            out.octal.as_str(),
            out.hex.as_str(),
            out.custom.as_str()
        ),
        ("11111111", "377", "ff", "73")
    );
    assert_eq!((out.bit_length, out.byte_length), (8, 1));
    assert_eq!(out.source, "255");
}

#[test]
fn grouping_and_uppercase() {
    let mut a = args("0xdeadbeef", 0);
    a.uppercase = true;
    a.group = true;
    let out = convert(a).unwrap();
    assert_eq!(out.hex, "DEAD BEEF");
    assert_eq!(out.decimal, "3,735,928,559");
    assert_eq!(out.source, "DEADBEEF");
    assert_eq!(group("1234567", 3, ','), "1,234,567");
    assert_eq!(group("101", 4, ' '), "101");
}

#[test]
fn twos_complement_widths() {
    let out = convert(args("-1", 10)).unwrap();
    assert!(out.negative);
    assert_eq!(out.widths.len(), 5);
    assert_eq!(out.widths[0].hex, "ff");
    assert_eq!(out.widths[0].unsigned, "255");
    assert_eq!(out.widths[0].signed, "-1");
    assert_eq!(out.bits.as_deref(), Some("1".repeat(64).as_str()));

    // 200 在 8 位下无符号可表示，有符号解释为 -56
    let out = convert(args("200", 10)).unwrap();
    assert_eq!(out.widths[0].signed, "-56");
    // -129 超出 8 位范围
    let out = convert(args("-129", 10)).unwrap();
    assert_eq!(out.widths[0].bits, 16);
    assert_eq!(twos_complement(&big(-128), 8), Some(big(128)));
    assert_eq!(twos_complement(&big(256), 8), None);
}

#[test]
fn huge_values_have_no_bit_grid() {
    let out = convert(args(&"f".repeat(40), 16)).unwrap();
    assert!(out.bits.is_none());
    assert!(out.widths.is_empty());
    assert_eq!(out.bit_length, 160);
}

#[test]
fn toggles_bits() {
    let mut a = args("0b1000", 0);
    a.toggle = Some(0);
    let out = convert(a).unwrap();
    assert_eq!(out.decimal, "9");
    assert_eq!(out.source, "1001");

    assert_eq!(toggle_bit(&big(-1), 0).unwrap(), big(-2));
    assert_eq!(toggle_bit(&big(0), 63).unwrap(), big(1 << 63));
    assert_eq!(
        toggle_bit(&big(0), 64).unwrap_err().code,
        "base.invalid_bit"
    );
    assert_eq!(
        toggle_bit(&(BigInt::one() << 70), 0).unwrap_err().code,
        "base.out_of_range"
    );
}
