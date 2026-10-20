use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use tf_plugin_api::LogLevel;

struct Task {
    polls: AtomicUsize,
    cancel_at: usize,
}
impl Default for Task {
    fn default() -> Self {
        Self {
            polls: AtomicUsize::new(0),
            cancel_at: usize::MAX,
        }
    }
}
impl TaskContext for Task {
    fn log(&self, _: LogLevel, _: &str, _: serde_json::Value) {}
    fn progress(&self, _: u64, _: u64) {}
    fn stage(&self, _: &str) {}
    fn is_cancelled(&self) -> bool {
        self.polls.fetch_add(1, Ordering::Relaxed) >= self.cancel_at
    }
    fn open_file(&self, _: &str) -> PluginResult<Box<dyn std::io::Read + Send>> {
        unreachable!()
    }
    fn file_size(&self, _: &str) -> PluginResult<u64> {
        unreachable!()
    }
}
fn args(input: &str, expression: &str) -> Args {
    Args {
        input: input.into(),
        expression: expression.into(),
        namespaces: vec![],
        revision: 1,
    }
}
fn run(input: &str, expression: &str) -> Report {
    query(args(input, expression), &Task::default(), Limits::default()).unwrap()
}
fn scalar(input: &str, expression: &str) -> String {
    run(input, expression).scalar.unwrap()
}

#[test]
fn keeps_original_whitespace_and_mixed_cdata_text() {
    assert_eq!(
        scalar("<r> a <b><![CDATA[ b & c ]]></b> d </r>", "string(/r)"),
        " a  b & c  d "
    );
    let result = run("<r> a <![CDATA[b]]> c </r>", "/r/text()");
    assert_eq!(
        result
            .items
            .iter()
            .map(|n| n.value.as_str())
            .collect::<String>(),
        " a b c "
    );
}
#[test]
fn returns_typed_scalars_and_empty_nodeset() {
    assert_eq!(run("<r/>", "true()").kind, "boolean");
    assert_eq!(scalar("<r/>", "boolean(/r/missing)"), "false");
    assert_eq!(scalar("<r><i/><i/></r>", "count(/r/i)"), "2");
    assert_eq!(scalar("<r/>", "0 div 0"), "NaN");
    assert_eq!(scalar("<r/>", "1 div 0"), "Infinity");
    assert_eq!(scalar("<r/>", "-1 div 0"), "-Infinity");
    assert!(run("<r/>", "//missing").items.is_empty());
}
#[test]
fn resolves_namespace_uris_and_unprefixed_attributes() {
    let xml = "<r xmlns='urn:test' xmlns:b='urn:test'><b:i id='one' b:id='two'/><i/></r>";
    assert!(run(xml, "//i").items.is_empty());
    let mut a = args(xml, "//a:i/@id");
    a.namespaces = vec![Namespace {
        prefix: "a".into(),
        uri: "urn:test".into(),
    }];
    let result = query(a, &Task::default(), Limits::default()).unwrap();
    assert_eq!(result.items[0].value, "one");
    assert_eq!(result.items[0].kind, "attribute");
    let mut a = args(xml, "//a:i/@a:id");
    a.namespaces = vec![Namespace {
        prefix: "a".into(),
        uri: "urn:test".into(),
    }];
    assert_eq!(
        query(a, &Task::default(), Limits::default()).unwrap().items[0].value,
        "two"
    );
}
#[test]
fn handles_reverse_axes_before_document_order_presentation() {
    let xml = "<r><i>a</i><i>b</i><i>c</i></r>";
    assert_eq!(scalar(xml, "string(/r/i[3]/preceding-sibling::i[1])"), "b");
    assert_eq!(
        scalar(xml, "string((/r/i[3]/preceding-sibling::i)[1])"),
        "a"
    );
    assert_eq!(
        run(xml, "/r/i[3]/preceding-sibling::i")
            .items
            .iter()
            .map(|n| n.value.as_str())
            .collect::<Vec<_>>(),
        vec!["a", "b"]
    );
    assert!(run(xml, "/r/i[1.5]").items.is_empty());
}
#[test]
fn compares_nodesets_existentially_without_false_negation() {
    let xml = "<r><i>1</i><i>20</i></r>";
    assert_eq!(scalar(xml, "/r/i = '1'"), "true");
    assert_eq!(scalar(xml, "/r/i != '1'"), "true");
    assert_eq!(scalar(xml, "/r/i > 9"), "true");
    assert_eq!(scalar(xml, "/r/missing != '1'"), "false");
}
#[test]
fn rejects_dtd_external_entities_and_unbound_prefix_without_panicking() {
    for xml in [
        "<!DOCTYPE r SYSTEM 'file:///private/tmp/secret'><r/>",
        "<!DOCTYPE r [<!ENTITY x 'hello'>]><r>&x;</r>",
    ] {
        assert_eq!(
            query(args(xml, "/r"), &Task::default(), Limits::default())
                .err()
                .unwrap()
                .code,
            "xpath.dtd"
        );
    }
    assert_eq!(
        query(args("<r/>", "//x:r"), &Task::default(), Limits::default())
            .err()
            .unwrap()
            .code,
        "xpath.unbound_prefix"
    );
    assert_eq!(
        query(
            args("<r/>", "id('test')"),
            &Task::default(),
            Limits::default()
        )
        .err()
        .unwrap()
        .code,
        "xpath.unsupported"
    );
    assert_eq!(
        query(args("<r/>", "//* ["), &Task::default(), Limits::default())
            .err()
            .unwrap()
            .code,
        "xpath.syntax"
    );
}
#[test]
fn fragments_are_serialized_not_raw_and_keep_namespace_meaning() {
    let xml = "<r xmlns='u'><i xmlns=''> x &amp; y </i><p:i xmlns:p='v' p:a='1'/></r>";
    let result = run(xml, "/*");
    let fragment = result.items[0].fragment.as_ref().unwrap();
    assert_eq!(scalar(fragment, "string(/*/*[1])"), " x & y ");
    assert_eq!(scalar(fragment, "namespace-uri(/*/*[1])"), "");
    assert_eq!(scalar(fragment, "namespace-uri(/*/*[2])"), "v");
    assert_eq!(scalar(fragment, "namespace-uri(/*/*[2]/@*)"), "v");
}
#[test]
fn pages_results_and_does_not_clip_copy_or_full_report() {
    let xml = format!(
        "<r>{}</r>",
        (0..45).map(|i| format!("<i>{i}</i>")).collect::<String>()
    );
    let result = run(&xml, "/r/i");
    let page = result.page(20, 20).unwrap();
    assert_eq!(page.total, 45);
    assert_eq!(page.items[0].index, 20);
    assert_eq!(page.items.len(), 20);
    assert_eq!(result.copy(Some(44), false).unwrap(), "44");
    assert!(result.page(0, PAGE_SIZE + 1).is_err());
    let result = run(&format!("<r>{}</r>", "中".repeat(1_000)), "string(/r)");
    assert!(result.page(0, 20).unwrap().scalar_truncated);
    assert_eq!(result.copy(None, false).unwrap().chars().count(), 1_000);
}
#[test]
fn stops_expensive_traversal_comparison_and_function_growth() {
    let xml = format!("<r>{}</r>", "<i>1</i>".repeat(80));
    for expr in [
        "//i[preceding::i]",
        "//i != //i",
        "//i < //i",
        "concat(string(/r),string(/r))",
    ] {
        let result = query(
            args(&xml, expr),
            &Task::default(),
            Limits {
                work: 20,
                elapsed: Duration::from_secs(10),
            },
        );
        assert_eq!(result.err().unwrap().code, "xpath.limit", "{expr}");
    }
}
#[test]
fn cancellation_happens_inside_evaluation() {
    let xml = format!("<r>{}</r>", "<i>1</i>".repeat(100));
    // XML preflight consumes about 305 polls; this crosses into actual evaluator checks.
    let task = Task {
        polls: AtomicUsize::new(0),
        cancel_at: 400,
    };
    let result = query(args(&xml, "//i[preceding::i]"), &task, Limits::default());
    assert_eq!(result.err().unwrap().code, "task.cancelled");
    assert!(task.polls.load(Ordering::Relaxed) > 400);
}
#[test]
fn bounds_input_expression_depth_and_complete_results() {
    assert!(
        query(
            args(&"x".repeat(MAX_INPUT + 1), "/"),
            &Task::default(),
            Limits::default()
        )
        .is_err()
    );
    let xml = format!(
        "{}{}",
        "<r>".repeat(MAX_DEPTH + 1),
        "</r>".repeat(MAX_DEPTH + 1)
    );
    assert_eq!(
        query(args(&xml, "/"), &Task::default(), Limits::default())
            .err()
            .unwrap()
            .code,
        "xpath.limit"
    );
    let xml = format!(
        "<r>{}</r>",
        format!("<i>{}</i>", "x".repeat(10_000)).repeat(40)
    );
    assert!(
        query(
            args(&xml, "//i/ancestor::*"),
            &Task::default(),
            Limits::default()
        )
        .is_ok()
    );
    let xml = format!(
        "<r><a><b><c><d><e>{}</e></d></c></b></a></r>",
        format!("<i>{}</i>", "x".repeat(20_000)).repeat(24)
    );
    assert_eq!(
        query(args(&xml, "//*"), &Task::default(), Limits::default())
            .err()
            .unwrap()
            .code,
        "xpath.limit"
    );
}

#[test]
fn parenthesized_operator_operands_are_not_mistaken_for_function_calls() {
    assert_eq!(scalar("<r/>", "true() and (false() or (true()))"), "true");
    assert_eq!(scalar("<r/>", "12 div (3) + 5 mod (2)"), "5");
    assert_eq!(scalar("<r/>", "count(/r[1 and (true())])"), "1");
    assert_eq!(scalar("<r/>", "string(-0)"), "0");
    assert_eq!(scalar("<r/>", "contains('and (', 'and')"), "true");
    for expression in ["and(true())", "unknown()", "false() and id('x')"] {
        let error = query(
            args("<r/>", expression),
            &Task::default(),
            Limits::default(),
        )
        .err()
        .unwrap();
        assert_eq!(error.code, "xpath.unsupported", "{expression}");
    }
}
