use super::*;

fn list(text: &str) -> Vec<(String, usize)> {
    extract(text).into_iter().collect()
}

fn names(text: &str) -> Vec<String> {
    extract(text).into_keys().collect()
}

#[test]
fn extracts_common_placeholder_styles() {
    assert_eq!(
        names("Hello {{name}}, {{ count, number }}"),
        ["{{count}}", "{{name}}"]
    );
    assert_eq!(names("Hi {name}, you have {0} items"), ["{0}", "{name}"]);
    assert_eq!(names("%1$s owes %2$d"), ["%1$s", "%2$d"]);
    assert_eq!(names("Hello %{user}"), ["%{user}"]);
    // 宽度与精度不参与比较
    assert_eq!(names("Done %@ %.2f"), ["%@", "%f"]);
}

#[test]
fn icu_plural_branches_are_not_placeholders() {
    assert_eq!(
        names("{count, plural, one {# item for {name}} other {# items}}"),
        ["{count}"]
    );
}

#[test]
fn counts_positional_printf_only_once_and_sequential_ones_each_time() {
    assert_eq!(list("%s and %s"), [("%s".to_owned(), 2)]);
    assert_eq!(list("{a} {a}"), [("{a}".to_owned(), 1)]);
}

#[test]
fn ignores_plain_text_and_percent_signs() {
    assert!(extract("50% off, 100%% sure").is_empty());
    assert!(extract("{ not a placeholder }").is_empty());
    assert!(extract("{{unterminated").is_empty());
}
