use super::*;

fn args(versions: &str, range: &str) -> Args {
    Args {
        versions: versions.into(),
        range: range.into(),
        include_prerelease: false,
        order: Order::Asc,
    }
}

#[test]
fn sorts_and_tests_versions() {
    let out = run(args(
        "1.0.0\n1.0.0-beta.11\n\n1.0.0-beta.2\n0.2.9\n0.3.0\nnope",
        "^0.2.3",
    ))
    .unwrap();
    let order: Vec<&str> = out.rows.iter().map(|r| r.input.as_str()).collect();
    assert_eq!(
        order,
        [
            "0.2.9",
            "0.3.0",
            "1.0.0-beta.2",
            "1.0.0-beta.11",
            "1.0.0",
            "nope"
        ]
    );
    assert_eq!((out.valid, out.invalid, out.matched), (5, 1, Some(1)));
    assert_eq!(out.sets, vec![">=0.2.3 <0.3.0-0"]);
    assert_eq!(out.rows[0].outcome, Some(Outcome::Match));
    assert_eq!(out.rows[5].line, 7);
    assert!(out.csv.starts_with("line,version,normalized,result,detail\n5,0.2.9,0.2.9,match,\n6,0.3.0,0.3.0,no-match,<0.3.0-0\n"));
}

#[test]
fn reports_equal_precedence_and_desc_order() {
    let mut a = args("1.0.0+b\n1.0.0+a\n2.0.0", "");
    a.order = Order::Desc;
    let out = run(a).unwrap();
    assert_eq!(out.rows[0].input, "2.0.0");
    assert_eq!(out.equal_precedence, vec![[1, 2]]);
    assert_eq!(out.matched, None);
}

#[test]
fn keeps_range_errors_separate() {
    let out = run(args("1.0.0", "^01.0")).unwrap();
    assert_eq!(out.range_error.unwrap().code, "semver.leading_zero");
    assert!(out.rows[0].outcome.is_none());
    let many = "1.0.0\n".repeat(MAX_VERSIONS + 1);
    assert_eq!(run(args(&many, "")).unwrap_err().code, "semver.too_many");
}
