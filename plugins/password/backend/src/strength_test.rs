use super::*;

fn analyze_str(password: &str) -> Report {
    analyze(Args {
        password: password.into(),
    })
    .unwrap()
}

#[test]
fn common_passwords_are_weakest() {
    let report = analyze_str("Password123");
    assert_eq!(report.score, 0);
    assert!(report.warnings.contains(&"common"));
    assert_eq!(report.crack_time.unit, "instant");
}

#[test]
fn detects_patterns() {
    let report = analyze_str("aaaabcqwe");
    assert!(report.warnings.contains(&"repeated"));
    assert!(report.warnings.contains(&"sequence"));
    assert!(report.warnings.contains(&"single_class"));
    assert!(report.warnings.contains(&"short"));
    // 规律字符降低有效熵
    assert!(report.entropy < entropy_bits(26, 9));
}

#[test]
fn strong_random_password() {
    let report = analyze_str("vR7#qL9!mZ2@xK4$wP8&");
    assert_eq!(report.score, 4);
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    assert_eq!(report.pool_size, 95);
    assert_eq!(report.crack_time.unit, "forever");
    assert_eq!(
        report.classes,
        Classes {
            upper: true,
            lower: true,
            digits: true,
            symbols: true,
            other: false
        }
    );
}

#[test]
fn unicode_counts_as_other() {
    let report = analyze_str("密码很长很长很长很长很长");
    assert!(report.classes.other);
    assert_eq!(report.length, 12);
}

#[test]
fn crack_time_units() {
    assert_eq!(crack_time(10.0).unit, "instant");
    assert_eq!(crack_time(40.0).unit, "seconds");
    assert_eq!(crack_time(50.0).unit, "hours");
    assert_eq!(crack_time(60.0).unit, "years");
    assert_eq!(crack_time(80.0).unit, "centuries");
    assert_eq!(crack_time(200.0).unit, "forever");
}

#[test]
fn rejects_empty_and_huge() {
    assert_eq!(
        analyze(Args {
            password: String::new()
        })
        .unwrap_err()
        .code,
        "password.empty"
    );
    assert_eq!(
        analyze(Args {
            password: "a".repeat(2000)
        })
        .unwrap_err()
        .code,
        "password.too_long"
    );
}
