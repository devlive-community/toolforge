use super::*;

fn run(pattern: &str, input: &str) -> Report {
    analyze(Args {
        pattern: pattern.into(),
        flags: Flags::default(),
        input: input.into(),
    })
    .unwrap()
}

fn kinds(report: &Report) -> Vec<(Kind, Severity)> {
    report
        .findings
        .iter()
        .map(|f| (f.kind, f.severity))
        .collect()
}

#[test]
fn flags_nested_quantifiers_and_shows_exponential_growth() {
    let report = run(r"^(a+)+$", "aaaa");
    assert_eq!(
        kinds(&report),
        vec![(Kind::NestedQuantifier, Severity::High)]
    );
    assert_eq!(report.findings[0].snippet, "(a+)+");
    assert_eq!(report.findings[0].attack, format!("{}!", "a".repeat(24)));
    assert!(!report.backtracking_engine);
    assert_eq!(report.growth, Some(Growth::Exponential));
    assert!(report.sample.as_ref().unwrap().matched);
}

#[test]
fn separated_repetition_is_not_flagged() {
    for pattern in [
        r"^(a+b)+$",
        r"^(\w+\.)+com$",
        r"^\d+$",
        r"[a-z]+@[a-z]+\.com",
    ] {
        let report = run(pattern, "");
        assert!(
            report.findings.is_empty(),
            "{pattern}: {:?}",
            kinds(&report)
        );
    }
}

#[test]
fn whitespace_words_pattern_is_flagged_with_prefix() {
    let report = run(r"^id:(\w+\s?)*$", "");
    assert_eq!(report.findings[0].kind, Kind::NestedQuantifier);
    assert!(report.findings[0].attack.starts_with("id:a"));
    assert_eq!(report.growth, Some(Growth::Exponential));
}

#[test]
fn flags_overlapping_alternation() {
    let report = run(r"^(\w|\d)+$", "");
    assert_eq!(
        kinds(&report),
        vec![(Kind::OverlappingAlternation, Severity::High)]
    );
    let report = run(r"^(ab|ac)+$", "");
    assert_eq!(
        kinds(&report),
        vec![(Kind::OverlappingAlternation, Severity::Medium)]
    );
    assert!(run(r"^(a|b)+$", "").findings.is_empty());
}

#[test]
fn flags_adjacent_quantifiers_as_polynomial() {
    let report = run(r"^\d+\d+$", "");
    assert_eq!(
        kinds(&report),
        vec![(Kind::AdjacentQuantifiers, Severity::Medium)]
    );
    assert_eq!(report.findings[0].snippet, r"\d+\d+");
    assert_eq!(report.growth, Some(Growth::Polynomial));
}

#[test]
fn simulates_backrefs_and_lookarounds() {
    let report = run(r"(\w)\1(?=!)", "xyzz!");
    assert!(report.backtracking_engine);
    assert!(report.simulated);
    let sample = report.sample.unwrap();
    assert!(sample.matched && !sample.exceeded);
    let report = run(r"(?<=\$)\d+", "cost $42");
    assert!(report.sample.unwrap().matched);
    let report = run(r"(?>a+)b", "aaac");
    assert!(!report.sample.unwrap().matched);
}

#[test]
fn renders_snippets() {
    let tree = Expr::parse_tree(r"(?:ab)*?[0-9]{2,3}x{4}").unwrap().expr;
    assert_eq!(render(&tree), "(?:ab)*?[0-9]{2,3}x{4}");
}

#[test]
fn rejects_invalid_patterns() {
    let err = analyze(Args {
        pattern: "(".into(),
        flags: Flags::default(),
        input: String::new(),
    })
    .unwrap_err();
    assert_eq!(err.code, "regex.invalid");
}
