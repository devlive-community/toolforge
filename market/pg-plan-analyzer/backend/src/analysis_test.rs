use super::*;

const NESTED: &str = r#"[
  {
    "Plan": {
      "Node Type": "Nested Loop",
      "Join Type": "Inner",
      "Startup Cost": 0.42,
      "Total Cost": 1200.50,
      "Plan Rows": 100,
      "Plan Width": 16,
      "Actual Startup Time": 0.05,
      "Actual Total Time": 12.0,
      "Actual Rows": 90,
      "Actual Loops": 1,
      "Plans": [
        {
          "Node Type": "Seq Scan",
          "Relation Name": "orders",
          "Schema": "public",
          "Alias": "o",
          "Filter": "(status = 'open')",
          "Rows Removed by Filter": 50000,
          "Startup Cost": 0.0,
          "Total Cost": 900.0,
          "Plan Rows": 5000,
          "Plan Width": 8,
          "Actual Startup Time": 0.01,
          "Actual Total Time": 5.0,
          "Actual Rows": 120000,
          "Actual Loops": 1
        },
        {
          "Node Type": "Sort",
          "Sort Key": ["created_at DESC"],
          "Startup Cost": 10.0,
          "Total Cost": 90.0,
          "Plan Rows": 40,
          "Plan Width": 12,
          "Actual Startup Time": 1.0,
          "Actual Total Time": 6.5,
          "Actual Rows": 40,
          "Actual Loops": 3,
          "Temp Read Blocks": 12,
          "Temp Written Blocks": 24
        }
      ]
    },
    "Planning Time": 0.8,
    "Execution Time": 12.9
  }
]"#;

#[test]
fn builds_the_tree_and_splits_time_by_loops() {
    let report = analyze_text(NESTED, &Limits::default()).unwrap();
    assert_eq!(report.statements.len(), 1);
    let statement = &report.statements[0];
    assert!(statement.has_actual);
    assert_eq!(statement.execution_time, Some(12.9));
    assert_eq!(statement.node_count, 3);
    let root = statement.root.as_ref().unwrap();
    assert_eq!(root.id, 0);
    assert_eq!(root.label, "Nested Loop");
    assert_eq!(root.inclusive_ms, Some(12.0));
    // 子节点包含耗时：Seq Scan 5.0 × 1 + Sort 6.5 × 3 = 24.5；自身耗时不为负
    assert_eq!(root.children[0].inclusive_ms, Some(5.0));
    assert_eq!(root.children[1].inclusive_ms, Some(19.5));
    assert_eq!(root.exclusive_ms, Some(0.0));

    let scan = &root.children[0];
    assert_eq!(scan.label, "Seq Scan on public.orders as o");
    assert_eq!(scan.exclusive_ms, Some(5.0));
    assert_eq!(scan.rows_removed_by_filter, Some(50000.0));
}

#[test]
fn notes_deviation_spill_and_large_seq_scan() {
    let report = analyze_text(NESTED, &Limits::default()).unwrap();
    let root = report.statements[0].root.as_ref().unwrap();
    let scan = &root.children[0];
    // 估计 5000 行、实际 120000 行，偏差提示
    assert!(scan.notes.iter().any(|n| n.code == "pg.row_deviation"));
    // 顺序扫描行数较多只提示，不定性为错误
    assert!(scan.notes.iter().any(|n| n.code == "pg.seq_scan_many_rows"));
    let sort = &root.children[1];
    assert!(sort.notes.iter().any(|n| n.code == "pg.sort_spilled"));
}

#[test]
fn plan_only_has_no_actual_notes() {
    let report = analyze_text(
        r#"[{ "Plan": { "Node Type": "Seq Scan", "Relation Name": "t", "Startup Cost": 0, "Total Cost": 10, "Plan Rows": 500000, "Plan Width": 4 } }]"#,
        &Limits::default(),
    )
    .unwrap();
    let statement = &report.statements[0];
    assert!(!statement.has_actual);
    let root = statement.root.as_ref().unwrap();
    assert_eq!(root.inclusive_ms, None);
    assert!(root.notes.is_empty());
}

#[test]
fn hotspots_rank_exclusive_time() {
    let report = analyze_text(NESTED, &Limits::default()).unwrap();
    assert!(!report.hotspots.is_empty());
    let first = &report.hotspots[0];
    assert_eq!(first.kind, "time");
    assert_eq!(first.value, 19.5);
}

#[test]
fn supports_multiple_statements() {
    let text = format!(
        "[{},{}]",
        &NESTED[1..NESTED.len() - 1],
        &NESTED[1..NESTED.len() - 1]
    );
    let report = analyze_text(&text, &Limits::default()).unwrap();
    assert_eq!(report.statements.len(), 2);
    assert_eq!(report.statements[1].index, 1);
}

#[test]
fn enforces_limits_and_reports_broken_json() {
    let limits = Limits {
        max_nodes: 2,
        max_depth: 200,
    };
    assert_eq!(
        analyze_text(NESTED, &limits).unwrap_err().code,
        "pg.too_many_nodes"
    );
    assert_eq!(
        analyze_text("{oops", &Limits::default()).unwrap_err().code,
        "pg.invalid_json"
    );
    assert_eq!(
        analyze_text("[]", &Limits::default()).unwrap_err().code,
        "pg.empty"
    );
}

#[test]
fn markdown_report_includes_tree_and_hotspots() {
    let report = analyze_text(NESTED, &Limits::default()).unwrap();
    let md = markdown(&report);
    assert!(md.contains("# PostgreSQL Plan Analysis"));
    assert!(md.contains("Nested Loop"));
    assert!(md.contains("## Hotspots"));
    assert!(md.contains("pg.sort_spilled"));
}
