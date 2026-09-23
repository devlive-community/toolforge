use super::*;

fn rule(id: &str) -> Rule {
    rules().into_iter().find(|r| r.id == id).unwrap()
}

#[test]
fn detects_inline_rust_tests() {
    let r = rule("rust-inline-tests");
    assert!(r.matches("mod tests {"));
    assert!(r.matches("    mod tests{"));
    assert!(!r.matches("mod tests;"));
    assert!(!r.applies_to("crates/a/src/store_test.rs"));
    assert!(r.applies_to("crates/a/src/store.rs"));
}

#[test]
fn detects_browser_storage() {
    let r = rule("browser-storage");
    assert!(r.matches("localStorage.getItem('x')"));
    assert!(r.matches("document.cookie = 'a'"));
    assert!(!r.matches("const storage = useStore()"));
}

#[test]
fn detects_frontend_data_processing() {
    let r = rule("frontend-data-processing");
    assert!(r.matches("const v = JSON.parse(input)"));
    assert!(r.matches("btoa(text)"));
    assert!(!r.matches("JSON.stringify(args)"));
}

#[test]
fn native_controls_are_allowed_inside_ui_package() {
    let r = rule("native-controls");
    assert!(r.matches("<select value={v}>"));
    assert!(r.matches(r#"<Foo type="checkbox" />"#));
    assert!(r.matches("<input className='x' />"));
    assert!(!r.matches("<Input leading={icon} />"));
    assert!(!r.applies_to("packages/ui/src/components/Input.tsx"));
    assert!(r.applies_to("apps/desktop/src/App.tsx"));
}

#[test]
fn detects_raw_palette_and_hex_colors() {
    let r = rule("raw-palette");
    assert!(r.matches("className=\"bg-gray-100\""));
    assert!(r.matches("className=\"text-white\""));
    assert!(r.matches("className=\"bg-[#fff]\""));
    assert!(r.matches("color: '#1c9a4f'"));
    assert!(!r.matches("className=\"bg-surface text-fg-muted tile-violet\""));
    assert!(!r.matches("accent: 'violet'"));
}

#[test]
fn allow_marker_skips_line() {
    let r = rule("browser-storage");
    assert!(!r.matches("localStorage.clear() // tf-allow: migration cleanup"));
}

#[test]
fn detects_banned_dependencies() {
    let json = r#"{"dependencies":{"react":"19","js-yaml":"4"},"devDependencies":{"diff":"5"}}"#;
    assert_eq!(banned_deps_in(json), vec!["diff", "js-yaml"]);
    assert!(banned_deps_in(r#"{"dependencies":{"react-diff-view":"1"}}"#).is_empty());
}
