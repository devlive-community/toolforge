use super::*;
use crate::tests::{Context, parsed};

#[test]
fn case_filters_include_descendants_and_preserve_missing_duration() {
    let r = parsed(
        "<testsuites><testsuite name='a'><testcase name='test [1]' classname='Demo' time='0'/><testsuite name='nested'><testcase name='test [2]' classname='Demo' time='2'><failure/></testcase><testcase name='other'/></testsuite></testsuite><testsuite name='b'><testcase name='test [2]' time='5'/></testsuite></testsuites>",
    );
    let ctx = Context::default();
    let a = cases(
        &r,
        &Filter {
            suite: Some(1),
            slowest: true,
            ..Filter::default()
        },
        &ctx,
    )
    .unwrap();
    assert_eq!(
        a.iter().map(|c| c.seconds).collect::<Vec<_>>(),
        vec![Some(2.0), Some(0.0), None]
    );
    let b = cases(
        &r,
        &Filter {
            source: Some(0),
            query: "demo".into(),
            status: Some(Status::Failure),
            ..Filter::default()
        },
        &ctx,
    )
    .unwrap();
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].name, "test [2]");
}
#[test]
fn all_lists_are_paged_with_real_total_and_stable_ids() {
    let r = parsed(&format!(
        "<testsuite>{}</testsuite>",
        "<testcase name='same'/>".repeat(60)
    ));
    let ctx = Context::default();
    let args: BrowseArgs =
        serde_json::from_value(json!({"handle":"1","kind":"cases","offset":25,"limit":25}))
            .unwrap();
    let page = browse(&r, &args, &ctx).unwrap();
    assert_eq!(page["total"], 60);
    assert_eq!(page["items"].as_array().unwrap().len(), 25);
    assert_eq!(page["items"][0]["id"], 25);
    assert!(validate_page(0, 0).is_err());
    assert!(validate_page(0, 101).is_err());
    assert!(
        cases(
            &r,
            &Filter {
                suite: Some(99),
                ..Filter::default()
            },
            &ctx
        )
        .is_err()
    );
}
#[test]
fn exported_cells_escape_formulas_and_markdown_markup() {
    let r =
        parsed("<testsuite><testcase name='=SUM(1)' classname='&lt;script&gt;|x'/></testsuite>");
    let ctx = Context::default();
    let csv = crate::export::render(&r, &Filter::default(), "csv", &ctx).unwrap();
    assert!(csv.contains("\"'=SUM(1)\""));
    let md = crate::export::render(&r, &Filter::default(), "markdown", &ctx).unwrap();
    assert!(!md.contains("<script>"));
    assert!(md.contains("&lt;script&gt;\\|x"));
}
