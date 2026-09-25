use super::*;

#[test]
fn detects_expressions() {
    assert_eq!(
        detect("30 9 * * MON-FRI").unwrap(),
        Detection::new(90, "expression").with("fields", 5)
    );
    assert_eq!(detect("0 */15 * * * ?").unwrap().params["fields"], 6);
    assert_eq!(detect("@daily").unwrap().label, "alias");
    assert!(detect("1 2 3 4 5").is_none());
    assert!(detect("99 * * * *").is_none());
    assert!(detect("this is not a cron").is_none());
}
