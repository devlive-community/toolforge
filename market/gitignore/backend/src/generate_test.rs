use super::*;

fn generate(base: &str, custom: &str) -> Output {
    run(Args {
        base_content: base.into(),
        custom_rules: custom.into(),
        ..Args::default()
    })
    .unwrap()
}

#[test]
fn empty_input_has_no_synthetic_rules_or_sections() {
    let output = run(Args::default()).unwrap();
    assert!(output.output.is_empty());
    assert!(output.sections.is_empty());
    assert_eq!(output.stats.line_count, 0);
    assert_eq!(output.stats.rule_count, 0);
    assert_eq!(output.stats.duplicate_count, 0);
}

#[test]
fn preserves_selection_order_and_deduplicates_selected_ids() {
    let output = run(Args {
        template_ids: vec!["windows".into(), "node".into(), "windows".into()],
        base_content: "before/\n".into(),
        custom_rules: "after/\n".into(),
        ..Args::default()
    })
    .unwrap();
    let sections: Vec<&str> = output
        .sections
        .iter()
        .map(|section| section.id.as_str())
        .collect();
    assert_eq!(sections, ["base", "windows", "node", "custom"]);
    assert_eq!(output.stats.template_count, 2);
    assert_eq!(output.sections[0].rule_count, 1);
    assert_eq!(output.sections[3].rule_count, 1);
    assert_eq!(output.stats.line_count, output.output.lines().count());
    assert!(output.output.ends_with('\n'));
}

#[test]
fn removes_only_exact_positive_duplicates_between_negations() {
    let output = generate(
        "# Original comment\n*.log\n*.log\n!important.log\n*.log\n",
        "*.log\n!important.log\n!important.log\n*.log\n",
    );
    let rules: Vec<&str> = output.output.lines().filter(|line| is_rule(line)).collect();
    assert_eq!(
        rules,
        [
            "*.log",
            "!important.log",
            "*.log",
            "!important.log",
            "!important.log",
            "*.log"
        ]
    );
    assert_eq!(output.stats.rule_count, 6);
    assert_eq!(output.stats.duplicate_count, 2);
    assert!(output.output.contains("# Original comment\n"));
}

#[test]
fn leading_spaces_escapes_tabs_and_trailing_spaces_are_preserved() {
    let input = " #literal\n !literal\n\\#literal\n\\!literal\nescaped\\ \nordinary  \n\t\n   \n#comment\n!\n";
    let output = generate(input, input);
    assert!(output.output.contains(input));
    for pattern in [
        " #literal",
        " !literal",
        "\\#literal",
        "\\!literal",
        "escaped\\ ",
        "ordinary  ",
        "\t",
    ] {
        assert!(output.output.lines().any(|line| line == pattern));
    }
    assert_eq!(output.stats.rule_count, 14);
    // The bare ! is kept and conservatively resets duplicate suppression.
    assert_eq!(output.stats.duplicate_count, 0);
    assert_eq!(
        output
            .output
            .lines()
            .filter(|line| *line == "#comment")
            .count(),
        2
    );
}

#[test]
fn only_ascii_space_is_inert_and_only_first_character_marks_a_comment() {
    for line in ["", "   ", "#comment", "!"] {
        assert!(!is_rule(line), "{line:?}");
    }
    for line in [
        "\t",
        "\u{a0}",
        " #comment",
        " !rule",
        "\\#rule",
        "\\!rule",
        "\\ ",
    ] {
        assert!(is_rule(line), "{line:?}");
    }
}

#[test]
fn normalizes_git_line_endings_and_preserves_additional_or_interior_cr() {
    let output = generate(
        "\u{feff}first\r\nsecond\r",
        "\u{feff}third\rvalue\nfourth\r\r\n",
    );
    assert!(!output.output.contains('\u{feff}'));
    assert!(output.output.contains("first\nsecond\n"));
    assert!(output.output.contains("third\rvalue\nfourth\r \n"));
    let crlf = run(Args {
        base_content: "first\r\nsecond\n".into(),
        line_ending: LineEnding::Crlf,
        ..Args::default()
    })
    .unwrap();
    assert!(crlf.output.contains("first\r\nsecond\r\n"));
    assert!(!crlf.output.replace("\r\n", "").contains('\n'));
    assert_eq!(crlf.stats.line_count, crlf.output.lines().count());
}

#[test]
fn rejects_unknown_ids_nul_and_oversized_combined_text() {
    let unknown = run(Args {
        template_ids: vec!["not-a-template".into()],
        ..Args::default()
    })
    .err()
    .unwrap();
    assert_eq!(unknown.code, "gitignore.unknownTemplate");
    assert_eq!(unknown.params["id"], "not-a-template");
    for field in ["baseContent", "customRules"] {
        let args = if field == "baseContent" {
            Args {
                base_content: "a\0b".into(),
                ..Args::default()
            }
        } else {
            Args {
                custom_rules: "a\0b".into(),
                ..Args::default()
            }
        };
        let error = run(args).err().unwrap();
        assert_eq!(error.code, "gitignore.invalidText");
        assert_eq!(error.params["field"], field);
    }
    let error = run(Args {
        base_content: "a".repeat(MAX_USER_BYTES / 2),
        custom_rules: "a".repeat(MAX_USER_BYTES / 2 + 1),
        ..Args::default()
    })
    .err()
    .unwrap();
    assert_eq!(error.code, "gitignore.tooLarge");
    let error = run(Args {
        template_ids: vec!["node".into(); MAX_SELECTIONS + 1],
        ..Args::default()
    })
    .err()
    .unwrap();
    assert_eq!(error.code, "gitignore.tooManyTemplates");
}

#[test]
fn every_template_combination_keeps_rules_and_source_sections() {
    let args = Args {
        template_ids: catalog::TEMPLATES
            .iter()
            .map(|template| template.id.into())
            .collect(),
        ..Args::default()
    };
    let output = run(args).unwrap();
    assert_eq!(output.stats.template_count, 23);
    assert_eq!(output.sections.len(), 23);
    let original_rules: usize = catalog::TEMPLATES
        .iter()
        .map(|template| {
            source_lines(template.text)
                .filter(|line| is_rule(line))
                .count()
        })
        .sum();
    assert_eq!(
        output.stats.rule_count + output.stats.duplicate_count,
        original_rules
    );
    assert!(output.stats.duplicate_count > 0);
    assert!(output.output.contains("!"));
    assert!(output.output.contains("# --- Node.js ---"));
    assert!(output.output.contains("# --- Linux ---"));
}
