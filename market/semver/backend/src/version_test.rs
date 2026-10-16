use std::cmp::Ordering;

use super::*;

fn v(text: &str) -> Version {
    parse(text).unwrap()
}

#[test]
fn parses_components() {
    let version = v("v1.2.3-beta.11+build.7");
    assert_eq!((version.major, version.minor, version.patch), (1, 2, 3));
    assert_eq!(
        version.pre,
        vec![Identifier::Alpha("beta".into()), Identifier::Numeric(11)]
    );
    assert_eq!(version.build, vec!["build", "7"]);
    assert_eq!(version.to_string(), "1.2.3-beta.11+build.7");
}

#[test]
fn orders_by_semver_precedence() {
    let order = [
        "1.0.0-alpha",
        "1.0.0-alpha.1",
        "1.0.0-alpha.beta",
        "1.0.0-beta",
        "1.0.0-beta.2",
        "1.0.0-beta.11",
        "1.0.0-rc.1",
        "1.0.0",
    ];
    for pair in order.windows(2) {
        assert_eq!(
            v(pair[0]).precedence(&v(pair[1])),
            Ordering::Less,
            "{pair:?}"
        );
    }
    assert_eq!(v("1.0.0+a").precedence(&v("1.0.0+b")), Ordering::Equal);
}

#[test]
fn rejects_invalid_versions() {
    let code = |text: &str| parse(text).unwrap_err().code;
    assert_eq!(code("01.2.3"), "semver.leading_zero");
    assert_eq!(code("1.2.3-01"), "semver.leading_zero");
    assert_eq!(code("1.2"), "semver.invalid_core");
    assert_eq!(code("1.2.3-"), "semver.invalid_identifier");
    assert_eq!(code("1.2.3-a..b"), "semver.invalid_identifier");
    assert_eq!(code("1.2.x"), "semver.invalid_number");
    assert_eq!(code("9007199254740992.0.0"), "semver.number_too_large");
    assert_eq!(
        code(&format!("1.0.0-{}", "a".repeat(300))),
        "semver.too_long"
    );
    assert_eq!(code(" "), "semver.empty");
    // 构建元数据允许前导零
    assert!(parse("1.0.0+001").is_ok());
}
