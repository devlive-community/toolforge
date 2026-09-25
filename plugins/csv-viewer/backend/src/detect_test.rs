use super::*;

#[test]
fn detects_tables() {
    let tsv = detect("name\tage\nAda\t36\n").unwrap();
    assert_eq!(
        tsv,
        Detection::new(72, "tsv").with("rows", 2).with("columns", 2)
    );
    assert_eq!(detect("a,b,c\n1,2,3\n4,5,6").unwrap().label, "csv");
    assert!(
        detect("a,b\n1,2").is_none(),
        "two short comma lines are too weak"
    );
    assert!(detect("a\tb\n1\t2\t3").is_none());
    assert!(detect("hello, world").is_none());
}
