use super::*;

#[test]
fn parses_preamble_sections_and_problems() {
    let cfg = parse(
        "\u{feff}# comment\nroot = TRUE\n\n[*]\nIndent_Style = Space\n; note\nkey = value # not a comment\n[]\nbroken line\n",
    );
    assert!(cfg.root);
    assert_eq!(cfg.sections.len(), 2);
    assert_eq!(cfg.sections[0].glob, "*");
    assert_eq!(cfg.sections[0].pairs[0].key, "indent_style");
    assert_eq!(cfg.sections[0].pairs[0].value, "Space");
    // 规范不支持行尾注释
    assert_eq!(cfg.sections[0].pairs[1].value, "value # not a comment");
    let kinds: Vec<&str> = cfg.problems.iter().map(|p| p.kind).collect();
    assert_eq!(kinds, ["emptySection", "invalidLine"]);
    assert_eq!(cfg.problems[1].line, 9);
}

#[test]
fn reports_bad_preamble_entries() {
    let cfg = parse("root = maybe\nindent_size = 2\n");
    assert!(!cfg.root);
    let kinds: Vec<&str> = cfg.problems.iter().map(|p| p.kind).collect();
    assert_eq!(kinds, ["invalidRoot", "pairOutsideSection"]);
}
