use super::*;

#[test]
fn validates_semver() {
    assert!(is_semver("1.2.3"));
    assert!(is_semver("0.2.0-beta.1"));
    assert!(!is_semver("v1.2.3"));
    assert!(!is_semver("1.2"));
}

#[test]
fn replaces_only_top_level_package_version() {
    let json =
        "{\n  \"name\": \"a\",\n  \"version\": \"0.1.0\",\n  \"deps\": { \"version\": \"9\" }\n}\n";
    let out = set_package_version(json, "0.2.0").unwrap();
    assert!(out.contains("\"version\": \"0.2.0\","));
    assert!(out.contains("{ \"version\": \"9\" }"));
}

#[test]
fn replaces_workspace_package_version_only() {
    let toml = "[workspace]\nmembers = []\n\n[workspace.package]\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\nversion = \"keep\"\n";
    let out = set_workspace_version(toml, "1.0.0").unwrap();
    assert!(out.contains("[workspace.package]\nversion = \"1.0.0\""));
    assert!(out.contains("[dependencies]\nversion = \"keep\""));
}

#[test]
fn missing_fields_are_reported() {
    assert!(set_package_version("{}", "1.0.0").is_none());
    assert!(set_workspace_version("[workspace]\n", "1.0.0").is_none());
}
