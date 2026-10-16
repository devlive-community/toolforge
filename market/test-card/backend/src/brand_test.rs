use super::*;

#[test]
fn luhn_matches_known_numbers() {
    assert!(luhn_valid("4242424242424242"));
    assert!(!luhn_valid("4242424242424241"));
    assert!(luhn_valid("378282246310005"));
    assert_eq!(check_digit("424242424242424"), 2);
    assert!(!luhn_valid(""));
}

#[test]
fn detects_brands_by_prefix() {
    let cases = [
        ("4242424242424242", Brand::Visa),
        ("5555555555554444", Brand::Mastercard),
        ("2223003122003222", Brand::Mastercard),
        ("378282246310005", Brand::Amex),
        ("6011111111111117", Brand::Discover),
        ("6500000000000002", Brand::Discover),
        ("3566002020360505", Brand::Jcb),
        ("6200000000000005", Brand::Unionpay),
        ("36227206271667", Brand::Diners),
        ("3056930009020004", Brand::Diners),
    ];
    for (number, brand) in cases {
        assert_eq!(detect(number), Some(brand), "{number}");
    }
    assert_eq!(detect("9999"), None);
    assert_eq!(detect("2721000000000000"), None);
}
