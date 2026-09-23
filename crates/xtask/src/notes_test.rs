use super::*;

fn commit(hash: &str, subject: &str) -> Commit {
    Commit {
        hash: hash.into(),
        subject: subject.into(),
    }
}

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
fn lists_every_commit_grouped_by_type() {
    let commits = vec![
        commit("a1", "feat(update): add in-app auto update"),
        commit("b2", "chore(deps): bump actions"),
        commit("c3", "fix: keep logo visible"),
        commit("d4", "ci: add release workflow"),
        commit("e5", "Update README"),
    ];
    let notes = render(&commits, None);
    assert_eq!(
        notes,
        "### Features\n\n- **update**: add in-app auto update (a1)\n\n\
         ### Bug Fixes\n\n- keep logo visible (c3)\n\n\
         ### Build & CI\n\n- add release workflow (d4)\n\n\
         ### Chores\n\n- **deps**: bump actions (b2)\n\n\
         ### Other Changes\n\n- Update README (e5)\n"
    );
}

#[test]
fn skips_release_commits_and_adds_compare_link() {
    let commits = vec![
        commit("a1", "chore(release): v0.2.0"),
        commit("b2", "feat: add hash plugin"),
    ];
    let notes = render(&commits, Some(("v0.1.0", "v0.2.0")));
    assert!(!notes.contains("v0.2.0 (a1)"));
    assert!(notes.ends_with(
        "**Full Changelog**: https://github.com/devlive-community/toolforge/compare/v0.1.0...v0.2.0\n"
    ));
}

#[test]
fn falls_back_when_empty() {
    assert_eq!(
        render(&[commit("a1", "chore(release): v1.0.0")], None),
        "Maintenance release.\n"
    );
}
