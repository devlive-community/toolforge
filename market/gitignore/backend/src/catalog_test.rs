use super::*;

fn ids(args: Args) -> Vec<String> {
    run(args)
        .unwrap()
        .templates
        .into_iter()
        .map(|template| template.id)
        .collect()
}

#[test]
fn catalog_has_pinned_source_and_unique_nonempty_templates() {
    let output = run(Args::default()).unwrap();
    let mut seen = std::collections::HashSet::new();
    assert_eq!(output.templates.len(), 23);
    for template in output.templates {
        assert!(seen.insert(template.id.clone()));
        assert!(template.rule_count > 0);
        assert_ne!(template.category, Category::All);
        assert!(!find(&template.id).unwrap().text.is_empty());
    }
    assert_eq!(output.source.revision, REVISION);
    assert_eq!(output.source.revision.len(), 40);
    assert!(output.source.license_text.contains("CC0 1.0 Universal"));
    assert!(!output.source.license_text.contains('\0'));
}

#[test]
fn search_is_case_insensitive_and_supports_keywords_and_categories() {
    let query = |query: &str, category| Args {
        query: query.into(),
        category,
        ..Args::default()
    };
    assert_eq!(ids(query("  PNPM ", Category::All)), vec!["node"]);
    assert_eq!(ids(query("intellij", Category::Editor)), vec!["jetbrains"]);
    assert_eq!(ids(query("操作系统", Category::All)), Vec::<&str>::new());
    assert_eq!(ids(query("苹果", Category::Os)), vec!["macos"]);
    assert_eq!(ids(query("NODE", Category::Framework)), vec!["angular"]);
    assert_eq!(ids(query("visual code", Category::Editor)), vec!["vscode"]);
    assert!(ids(query("Rust", Category::Os)).is_empty());
    assert_eq!(
        ids(query("", Category::Os)),
        vec!["macos", "windows", "linux"]
    );
}

#[test]
fn search_rejects_excessive_input_before_normalizing() {
    let err = run(Args {
        query: "x".repeat(MAX_QUERY_BYTES + 1),
        ..Args::default()
    })
    .err()
    .unwrap();
    assert_eq!(err.code, "gitignore.queryTooLong");
    assert_eq!(err.params["max"], MAX_QUERY_BYTES);
}
