use super::*;
use crate::tests::{Context, parsed};

#[test]
fn real_pytest_report_retains_parameterized_results() {
    let r = parsed(include_str!("../fixtures/pytest.xml"));
    assert_eq!(
        (
            r.counts.tests,
            r.counts.passed,
            r.counts.failures,
            r.counts.skipped
        ),
        (4, 2, 1, 1)
    );
    assert!(
        r.cases
            .iter()
            .any(|c| c.name == "test_values[2]" && c.status == Status::Failure)
    );
    assert!(
        r.content
            .iter()
            .any(|c| c.kind == "failure" && c.text.contains("AssertionError"))
    );
    assert!(!r.diagnostics.iter().any(|d| d.kind == "counts"));
}
#[test]
fn real_surefire_report_distinguishes_assertions_and_exceptions() {
    let r = parsed(include_str!("../fixtures/surefire.xml"));
    assert_eq!(
        (
            r.counts.tests,
            r.counts.passed,
            r.counts.failures,
            r.counts.errors,
            r.counts.skipped
        ),
        (4, 1, 1, 1, 1)
    );
    assert!(
        r.content.iter().any(|c| c.kind == "error"
            && c.type_name.as_deref() == Some("java.lang.IllegalStateException"))
    );
    assert!(!r.diagnostics.iter().any(|d| d.kind == "counts"));
}
#[test]
fn nested_suites_count_cases_once_and_compare_declared_numbers() {
    let r = parsed(
        "<testsuites tests='3'><testsuite name='outer' tests='2'><testcase name='x' time='0'/><testsuite name='inner' tests='2'><testcase name='x' time='2'/></testsuite></testsuite></testsuites>",
    );
    assert_eq!(r.counts.tests, 2);
    assert_eq!(r.suites[0].counts.tests, 2);
    assert_eq!(r.suites[1].counts.tests, 2);
    assert_eq!(r.suites[2].counts.tests, 1);
    assert_eq!(r.cases[0].name, r.cases[1].name);
    assert_ne!(r.cases[0].id, r.cases[1].id);
    assert_eq!(
        r.diagnostics.iter().filter(|d| d.kind == "counts").count(),
        2
    );
}
#[test]
fn missing_and_invalid_time_are_unknown_while_zero_is_known() {
    let r = parsed(
        "<testsuite tests='invalid' time='NaN'><testcase name='missing'/><testcase name='zero' time='0'/><testcase name='negative' time='-1'/><testcase name='infinity' time='inf'/><testcase name='bad' time='word'/></testsuite>",
    );
    assert_eq!(r.counts.missing_time, 4);
    assert_eq!(r.cases[1].seconds, Some(0.0));
    assert_eq!(r.counts.known_seconds, 0.0);
    assert_eq!(r.diagnostics.iter().filter(|d| d.kind == "time").count(), 4);
    assert_eq!(r.suites[0].declared.tests, None);
}
#[test]
fn cdata_and_entities_preserve_plain_text_and_mixed_order() {
    let r = parsed(
        "<testsuite><testcase name='a &amp; b'><failure message='a &lt; b' type='Assertion'><![CDATA[<script>]]>&amp;next</failure><system-err>甲&#x1f9f6;</system-err></testcase></testsuite>",
    );
    assert_eq!(r.cases[0].name, "a & b");
    assert_eq!(r.content[0].message.as_deref(), Some("a < b"));
    assert_eq!(r.content[0].text, "<script>&next");
    assert_eq!(r.content[1].text, "甲🧶");
}
#[test]
fn unknown_extensions_and_conflicting_states_are_never_silent_passes() {
    let r = parsed(
        "<testsuite><custom/><testcase name='flaky'><flakyFailure>old failure</flakyFailure></testcase><testcase name='state' status='pending'/><testcase name='conflict'><failure/><error/></testcase><testcase/></testsuite>",
    );
    assert_eq!(r.counts.unknown, 4);
    assert_eq!(r.counts.passed, 0);
    assert_eq!(r.content[0].text, "old failure");
    assert!(r.diagnostics.iter().any(|d| d.kind == "extension"));
    assert!(r.diagnostics.iter().any(|d| d.kind == "conflict"));
}
#[test]
fn zero_cases_and_suite_logs_are_preserved() {
    let r = parsed(
        "<testsuite name='empty' tests='0'><system-out>suite output</system-out></testsuite>",
    );
    assert_eq!(r.counts.tests, 0);
    assert_eq!(r.content[0].case, None);
    assert_eq!(r.suites[0].content, vec![0]);
    assert!(r.diagnostics.is_empty());
}
#[test]
fn dtd_external_entities_and_damaged_xml_are_rejected() {
    for xml in [
        "<!DOCTYPE testsuite SYSTEM 'file:///etc/passwd'><testsuite/>",
        "<!DOCTYPE testsuite [<!ENTITY x 'hello'>]><testsuite/>",
    ] {
        assert_eq!(
            parse(&mut Report::default(), "x".into(), xml, &Context::default())
                .err()
                .unwrap()
                .code,
            "junit.dtd"
        );
    }
    for xml in [
        "<testsuite>",
        "<testsuite/><testsuite/>",
        "<testsuite>&undefined;</testsuite>",
        "<testsuite><testcase></testsuite>",
        "garbage",
        "<wrong/>",
    ] {
        assert!(
            parse(&mut Report::default(), "x".into(), xml, &Context::default()).is_err(),
            "{xml}"
        );
    }
}
#[test]
fn explicit_size_depth_attribute_and_entry_budgets() {
    let ctx = Context::default();
    for (xml, code) in [
        (" ".repeat(FILE_LIMIT + 1), "junit.size"),
        (
            format!("{}{}", "<testsuite>".repeat(65), "</testsuite>".repeat(65)),
            "junit.depth",
        ),
        (
            format!("<testsuite name='{}'/>", "x".repeat(4097)),
            "junit.attribute",
        ),
        (
            format!(
                "<testsuite>{}</testsuite>",
                "<testcase name='x'/>".repeat(ENTRY_LIMIT)
            ),
            "junit.entries",
        ),
    ] {
        assert_eq!(
            parse(&mut Report::default(), "x".into(), &xml, &ctx)
                .err()
                .unwrap()
                .code,
            code
        );
    }
}
#[test]
fn cancellation_during_streaming_returns_no_completed_report() {
    let xml = format!(
        "<testsuite>{}</testsuite>",
        "<testcase name='x'/>".repeat(5000)
    );
    let ctx = Context {
        cancel_after: Some(100),
        ..Context::default()
    };
    assert!(parse(&mut Report::default(), "x".into(), &xml, &ctx).is_err());
}

#[test]
fn encoding_declarations_and_bom_are_checked() {
    assert_eq!(
        parsed("\u{feff}<?xml version='1.0' encoding='UTF-8'?><testsuite/>")
            .counts
            .tests,
        0
    );
    for encoding in ["UTF-16", "ISO-8859-1"] {
        let xml = format!("<?xml version='1.0' encoding='{encoding}'?><testsuite/>");
        assert_eq!(
            parse(
                &mut Report::default(),
                "x".into(),
                &xml,
                &Context::default()
            )
            .err()
            .unwrap()
            .code,
            "junit.encoding"
        );
    }
}

#[test]
fn status_and_result_must_agree_and_extensions_keep_case_identity() {
    let r = parsed(
        "<testsuite><testcase name='same' status='passed' result='failed'/><testcase name='same' status='run' result='completed'/></testsuite>",
    );
    assert_eq!(r.cases[0].status, Status::Unknown);
    assert_eq!(r.cases[1].status, Status::Passed);
    assert_ne!(r.cases[0].id, r.cases[1].id);
    assert_eq!(r.diagnostics[0].kind, "conflict");
}
