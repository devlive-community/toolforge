use super::*;

const VALID: &str = "91350100M000100Y43";

fn args(input: &str, mode: Mode, column: &str) -> Args {
    Args {
        input: input.into(),
        mode,
        column: column.into(),
        delimiter: ',',
    }
}

#[test]
fn checks_lines_and_skips_blank_ones_visibly() {
    let out = run(args(
        &format!("{VALID}\n\n 91350100m000100y43 \n91350100M000100Y44\nabc"),
        Mode::Lines,
        "",
    ))
    .unwrap();
    assert_eq!(
        (out.summary.valid, out.summary.invalid, out.summary.empty),
        (2, 2, 1)
    );
    assert_eq!(out.rows[1].line, 3);
    assert!(out.rows[1].normalized.uppercased && out.rows[1].normalized.trimmed);
    assert_eq!(out.rows[1].raw, " 91350100m000100y43 ");
    assert_eq!(out.rules, code::RULES);
}

#[test]
fn batch_matches_single_checks() {
    let input = format!("{VALID}\n91350100M000100Y44");
    let out = run(args(&input, Mode::Lines, "")).unwrap();
    for row in &out.rows {
        assert_eq!(row.report, code::check(&row.normalized.value));
    }
}

#[test]
fn reads_csv_column_by_name_or_index() {
    let csv = format!("name,code\n甲公司,{VALID}\n\"乙, 公司\",91350100M000100Y44\n");
    let by_name = run(args(&csv, Mode::Csv, "code")).unwrap();
    assert_eq!(by_name.columns, vec!["name", "code"]);
    assert_eq!((by_name.summary.valid, by_name.summary.invalid), (1, 1));
    assert_eq!(by_name.rows[0].line, 2);
    let by_index = run(args(&csv, Mode::Csv, "2")).unwrap();
    assert_eq!(by_index.summary.valid, 1);
    assert_eq!(
        run(args(&csv, Mode::Csv, "missing")).unwrap_err().code,
        "uscc.column_not_found"
    );
}

#[test]
fn exports_csv_and_json() {
    let input = format!("{VALID}\n91350100M000100Y44");
    let csv = export(ExportArgs {
        args: args(&input, Mode::Lines, ""),
        format: ExportFormat::Csv,
    })
    .unwrap();
    assert!(csv.contains(
        "1,91350100M000100Y43,91350100M000100Y43,valid,,industry_commerce,enterprise,350100,\n"
    ));
    assert!(csv.contains(",invalid,check:4->3,"));
    let json = export(ExportArgs {
        args: args(&input, Mode::Lines, ""),
        format: ExportFormat::Json,
    })
    .unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["summary"]["valid"], 1);
    assert_eq!(value["rows"][1]["problems"][0]["kind"], "check");
}
