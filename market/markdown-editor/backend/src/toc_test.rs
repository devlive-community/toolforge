use super::*;

#[test]
fn generates_indented_linked_toc() {
    let out = toc(Args {
        source: "# A\n\n## B\n\n### C\n\n## D".into(),
        max_depth: Some(2),
        linked: Some(true),
    })
    .unwrap();
    assert_eq!(out.entries, 3, "depth 2 excludes ### C");
    assert!(out.toc.contains("- [A](#a)"));
    assert!(out.toc.contains("  - [B](#b)"));
    assert!(!out.toc.contains("C"));
}

#[test]
fn unlinked_mode() {
    let out = toc(Args {
        source: "# A\n\n## B".into(),
        max_depth: None,
        linked: Some(false),
    })
    .unwrap();
    assert!(out.toc.contains("- A"));
    assert!(!out.toc.contains("](#"));
}

#[test]
fn anchor_check_finds_broken_links() {
    let source = "# Real\n\n## Sub\n\n[good](#real) [bad](#missing)";
    let out = toc(Args {
        source: source.into(),
        max_depth: None,
        linked: None,
    })
    .unwrap();
    assert_eq!(out.anchor_count, 2);
    assert_eq!(out.broken_anchors.len(), 1);
    assert_eq!(out.broken_anchors[0].anchor, "missing");
    assert_eq!(out.broken_anchors[0].line, 5);
}

#[test]
fn depth_clamps_instead_of_erroring() {
    // 深度 9 自动收敛到 6，不报错
    let out = toc(Args {
        source: "###### Deep".into(),
        max_depth: Some(9),
        linked: None,
    })
    .unwrap();
    assert_eq!(out.entries, 1);
}
