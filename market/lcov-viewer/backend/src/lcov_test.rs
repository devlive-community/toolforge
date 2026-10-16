use super::*;

const SAMPLE: &str = "TN:unit
SF:/ci/work/src/a.rs
FN:1,main
FN:5,9,helper
FNDA:1,main
FNDA:0,helper
DA:1,1
DA:2,1
DA:5,0
BRDA:2,0,0,1
BRDA:2,0,1,0
BRDA:6,0,0,-
LF:3
LH:2
BRF:3
BRH:1
end_of_record
TN:integration
SF:/ci/work/src/a.rs
FNDA:2,helper
DA:5,3
DA:7,0
BRDA:6,0,0,-
BRDA:2,0,1,4
end_of_record
";

#[test]
fn merges_records_across_test_groups() {
    let report = parse_text(SAMPLE);
    assert_eq!(report.tests, ["unit", "integration"]);
    assert_eq!(report.records, 2);
    let file = &report.files["/ci/work/src/a.rs"];
    assert_eq!(file.lines[&5].hits, 3);
    assert_eq!(file.lines.len(), 4);
    assert_eq!(file.functions["helper"].hits, 2);
    assert_eq!(file.functions["helper"].end_line, Some(9));
    assert_eq!(file.branches[&(2, "0".into(), "1".into())], Some(4));
    // 两边都是 - 时仍是 -
    assert_eq!(file.branches[&(6, "0".into(), "0".into())], None);
    let m = file.metrics();
    assert_eq!((m.lines.found, m.lines.hit), (4, 3));
    assert_eq!((m.functions.found, m.functions.hit), (2, 2));
    assert_eq!(
        (m.branches.found, m.branches.hit, m.branches_not_executed),
        (3, 2, 1)
    );
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
}

#[test]
fn understands_function_alias_records() {
    let report = parse_text(
        "SF:a.c\nFNL:0,10,20\nFNA:0,5,main\nFN:30,name,with,commas\nFNDA:0,name,with,commas\nend_of_record\n",
    );
    let file = &report.files["a.c"];
    assert_eq!(file.functions["main"].line, Some(10));
    assert_eq!(file.functions["main"].end_line, Some(20));
    assert_eq!(file.functions["main"].hits, 5);
    assert_eq!(file.functions["name,with,commas"].line, Some(30));
}

#[test]
fn reports_problems_without_inventing_numbers() {
    let report = parse_text(
        "VER:2.0\nXYZ:1\nDA:1,1\nSF:a.c\nDA:1,1\nDA:x,1\nMCDC:1,2\nLF:5\nend_of_record\nSF:b.c\nDA:1,0\nSF:c.c\nDA:2,2\nTN:\nSF:c.c\nend_of_record\n",
    );
    assert_eq!(report.version.as_deref(), Some("2.0"));
    assert_eq!(report.ignored["XYZ"], 1);
    assert_eq!(report.ignored["MCDC"], 1);
    let kinds: Vec<&str> = report.diagnostics.iter().map(|d| d.kind).collect();
    assert_eq!(
        kinds,
        [
            "outsideRecord",
            "malformed",
            "declaredMismatch",
            "unterminated",
            "unterminated",
            "duplicateRecord"
        ]
    );
    let mismatch = &report.diagnostics[2];
    assert_eq!(mismatch.detail.as_deref(), Some("LF: 5 ≠ 1"));
    assert_eq!(mismatch.line, Some(4));
    assert_eq!(report.files.len(), 3);
}

#[test]
fn tolerates_odd_hit_counts_and_crlf() {
    let report = parse_text(
        "SF:a\r\nDA:1,-1\r\nDA:2,3.7\r\nDA:3,18446744073709551615\r\nDA:3,5\r\nend_of_record\r\n",
    );
    let file = &report.files["a"];
    assert_eq!(file.lines[&1].hits, 0);
    assert_eq!(file.lines[&2].hits, 3);
    assert_eq!(file.lines[&3].hits, u64::MAX);
}
