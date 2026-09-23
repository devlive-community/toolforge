use super::*;

#[test]
fn parses_conventional_subjects() {
    assert_eq!(
        parse_subject("feat(plugin/json): add tree view"),
        Some(("feat", Some("plugin/json"), "add tree view"))
    );
    assert_eq!(
        parse_subject("fix!: drop legacy api"),
        Some(("fix", None, "drop legacy api"))
    );
    assert_eq!(parse_subject("Merge branch 'dev'"), None);
    assert_eq!(parse_subject("WIP: stuff"), None);
}

#[test]
fn groups_by_section_and_skips_maintenance_types() {
    let subjects = vec![
        "feat(update): add in-app auto update".to_owned(),
        "chore(deps): bump actions".to_owned(),
        "fix: keep logo visible".to_owned(),
        "ci: add release workflow".to_owned(),
    ];
    let notes = render(&subjects);
    assert_eq!(
        notes,
        "### Features\n\n- **update**: add in-app auto update\n\n### Bug Fixes\n\n- keep logo visible\n"
    );
}

#[test]
fn falls_back_when_nothing_user_facing() {
    assert_eq!(
        render(&["chore: tidy".to_owned()]),
        "Maintenance release.\n"
    );
}
