use super::*;
use crate::number::parse_arabic;

fn n(input: &str) -> Number {
    parse_arabic(input).unwrap().unwrap()
}

fn rmb_of(input: &str) -> String {
    rmb(&n(input)).unwrap().0
}

#[test]
fn writes_rmb_amounts_by_the_rules() {
    let cases = [
        ("0", "零元整"),
        ("0.5", "伍角"),
        ("0.05", "伍分"),
        ("1", "壹元整"),
        ("10", "壹拾元整"),
        ("105", "壹佰零伍元整"),
        ("1010", "壹仟零壹拾元整"),
        ("1004.5", "壹仟零肆元伍角"),
        ("1000.05", "壹仟元零伍分"),
        ("6007.14", "陆仟零柒元壹角肆分"),
        ("100000", "壹拾万元整"),
        ("100001", "壹拾万零壹元整"),
        ("1001000", "壹佰万壹仟元整"),
        ("10010000", "壹仟零壹万元整"),
        ("100000000", "壹亿元整"),
        ("100010000", "壹亿零壹万元整"),
        ("100000001", "壹亿零壹元整"),
        ("1234567.89", "壹佰贰拾叁万肆仟伍佰陆拾柒元捌角玖分"),
        ("-12.3", "负壹拾贰元叁角"),
    ];
    for (input, expected) in cases {
        assert_eq!(rmb_of(input), expected, "{input}");
    }
    let (text, changed) = rmb(&n("0.125")).unwrap();
    assert_eq!(text, "壹角叁分");
    assert!(changed);
    assert_eq!(rmb(&n("1e16")).unwrap_err().code, "num.too_large_chinese");
}

#[test]
fn writes_chinese_numerals() {
    let lower = |s: &str| numerals(&n(s), Case::Lower).unwrap();
    assert_eq!(lower("10"), "十");
    assert_eq!(lower("12"), "十二");
    assert_eq!(lower("110"), "一百一十");
    assert_eq!(lower("100000"), "十万");
    assert_eq!(lower("10005"), "一万零五");
    assert_eq!(lower("120000000"), "一亿二千万");
    assert_eq!(lower("3000000000000"), "三万亿");
    assert_eq!(lower("-1.25"), "负一点二五");
    assert_eq!(lower("0"), "零");
    assert_eq!(numerals(&n("1234"), Case::Upper).unwrap(), "壹仟贰佰叁拾肆");
}

#[test]
fn parses_chinese_numbers_and_amounts() {
    let parsed = |s: &str| parse(s).unwrap().unwrap();
    assert_eq!(parsed("一千二百三十四").0.plain(), "1234");
    assert_eq!(parsed("十二").0.plain(), "12");
    assert_eq!(parsed("两千零五").0.plain(), "2005");
    assert_eq!(parsed("一万零五").0.plain(), "10005");
    assert_eq!(parsed("一亿二千万").0.plain(), "120000000");
    assert_eq!(parsed("三万亿").0.plain(), "3000000000000");
    assert_eq!(parsed("一亿零五万").0.plain(), "100050000");
    assert_eq!(parsed("二〇二四").0.plain(), "2024");
    assert_eq!(parsed("负三点一四").0.plain(), "-3.14");
    let (amount, money) = parsed("人民币壹佰贰拾叁万肆仟伍佰陆拾柒元捌角玖分");
    assert_eq!((amount.plain().as_str(), money), ("1234567.89", true));
    assert_eq!(parsed("壹仟元零伍分").0.plain(), "1000.05");
    assert_eq!(parsed("伍角").0.plain(), "0.5");
    assert_eq!(parsed("壹拾万元整").0.plain(), "100000");
    assert!(parse("hello").is_none());
    assert!(parse("一千abc").is_none() || parse("一千abc").unwrap().is_err());
}

#[test]
fn round_trips_through_both_directions() {
    for value in [
        "7",
        "19",
        "20",
        "101",
        "1010",
        "10001",
        "100100",
        "1000001",
        "20230615",
        "987654321012",
    ] {
        let number = n(value);
        for case in [Case::Lower, Case::Upper] {
            let text = numerals(&number, case).unwrap();
            assert_eq!(parse(&text).unwrap().unwrap().0, number, "{value} → {text}");
        }
        let money = rmb(&number).unwrap().0;
        assert_eq!(
            parse(&money).unwrap().unwrap().0,
            number,
            "{value} → {money}"
        );
    }
}
