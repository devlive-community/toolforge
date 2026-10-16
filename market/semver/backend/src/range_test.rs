use super::*;
use crate::version::parse as v;

fn norm(range: &str) -> String {
    parse(range).unwrap().normalized().join(" || ")
}

fn sat(range: &str, version: &str) -> bool {
    parse(range).unwrap().test(&v(version).unwrap(), false) == Outcome::Match
}

#[test]
fn desugars_like_node_semver() {
    let cases = [
        ("^1.2.3", ">=1.2.3 <2.0.0-0"),
        ("^0.2.3", ">=0.2.3 <0.3.0-0"),
        ("^0.0.3", ">=0.0.3 <0.0.4-0"),
        ("^1.2.x", ">=1.2.0 <2.0.0-0"),
        ("^0.0.x", ">=0.0.0 <0.1.0-0"),
        ("^0.x", ">=0.0.0 <1.0.0-0"),
        ("^1.2.3-beta.2", ">=1.2.3-beta.2 <2.0.0-0"),
        ("~1.2.3", ">=1.2.3 <1.3.0-0"),
        ("~1.2", ">=1.2.0 <1.3.0-0"),
        ("~1", ">=1.0.0 <2.0.0-0"),
        ("~>1.2.3", ">=1.2.3 <1.3.0-0"),
        ("1.x", ">=1.0.0 <2.0.0-0"),
        ("1.2.*", ">=1.2.0 <1.3.0-0"),
        ("*", "*"),
        ("", "*"),
        ("1.2.3 - 2.3.4", ">=1.2.3 <=2.3.4"),
        ("1.2 - 2.3.4", ">=1.2.0 <=2.3.4"),
        ("1.2.3 - 2.3", ">=1.2.3 <2.4.0-0"),
        ("1.2.3 - 2", ">=1.2.3 <3.0.0-0"),
        (">1.2", ">=1.3.0"),
        (">1", ">=2.0.0"),
        ("<1.2", "<1.2.0-0"),
        ("<=1.2", "<1.3.0-0"),
        (">= 1.2.3 < 2", ">=1.2.3 <2.0.0-0"),
        ("1.2.3", "1.2.3"),
        ("v1.2.3 || =2.0.0", "1.2.3 || 2.0.0"),
        ("<*", "<0.0.0-0"),
    ];
    for (range, expected) in cases {
        assert_eq!(norm(range), expected, "{range}");
    }
}

#[test]
fn caret_on_zero_minor_rejects_next_minor() {
    assert!(sat("^0.2.3", "0.2.9"));
    assert!(!sat("^0.2.3", "0.3.0"));
    assert!(sat("^1.2.3", "1.9.0"));
    assert!(!sat("^1.2.3", "2.0.0"));
    assert!(sat(">=1.0.0 <2.0.0 || >=3.0.0", "3.1.0"));
    assert!(!sat("1.2.3 - 2.3.4", "2.3.5"));
}

#[test]
fn prereleases_need_a_matching_tuple_unless_included() {
    let range = parse("^1.2.3-beta.2").unwrap();
    let check = |version: &str, include: bool| range.test(&v(version).unwrap(), include);
    assert_eq!(check("1.2.3-beta.4", false), Outcome::Match);
    assert_eq!(check("1.2.4-beta.1", false), Outcome::Prerelease);
    assert_eq!(check("1.2.4-beta.1", true), Outcome::Match);
    assert_eq!(
        check("1.2.3-beta.1", false),
        Outcome::Failed {
            comparator: ">=1.2.3-beta.2".into()
        }
    );
    assert!(!sat("^1.0.0", "1.5.0-rc.1"));
    assert_eq!(
        parse("^1.0.0")
            .unwrap()
            .test(&v("1.5.0-rc.1").unwrap(), true),
        Outcome::Match
    );
}

#[test]
fn reports_invalid_ranges_with_columns() {
    let err = parse(">=1.2.3 <01.0").unwrap_err();
    assert_eq!(err.code, "semver.leading_zero");
    assert_eq!(err.params["column"], 9);
    let err = parse("^1.x.3").unwrap_err();
    assert_eq!(err.code, "semver.invalid_range");
    let err = parse("1.2.3 || ~").unwrap_err();
    assert_eq!(err.params["column"], 10);
    assert_eq!(
        parse(&"1".repeat(2000)).unwrap_err().code,
        "semver.range_too_long"
    );
}
