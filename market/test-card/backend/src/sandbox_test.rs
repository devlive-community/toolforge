use super::*;
use crate::brand;

#[test]
fn official_cases_match_their_brand_and_luhn() {
    for case in CASES {
        assert_eq!(
            brand::detect(case.number),
            Some(case.brand),
            "{}",
            case.number
        );
        // 唯一故意不通过 Luhn 的是「卡号错误」用例
        assert_eq!(
            brand::luhn_valid(case.number),
            case.scenario != "incorrectNumber",
            "{}",
            case.number
        );
        assert!(case.number.bytes().all(|b| b.is_ascii_digit()));
    }
    let numbers: std::collections::HashSet<_> = CASES.iter().map(|c| c.number).collect();
    assert_eq!(numbers.len(), CASES.len(), "no duplicate numbers");
}
