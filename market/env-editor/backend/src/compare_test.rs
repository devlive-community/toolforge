use super::*;
use crate::document::{parse, serialize};

fn cell(state: CellState, duplicate: bool, same_as_base: Option<bool>) -> Cell {
    Cell {
        state,
        duplicate,
        same_as_base,
    }
}

#[test]
fn reports_missing_extra_empty_duplicate_and_differing_keys() {
    let base = parse("A=1\nB=2\nC=\n");
    let prod = parse("A=1\nA=9\nC=x\nD=4\n");
    let result = compare(&[base, prod]);

    let keys: Vec<&str> = result.rows.iter().map(|row| row.key.as_str()).collect();
    assert_eq!(keys, ["A", "B", "C", "D"]);

    // 重复键取最后一次的值与基准比较
    assert_eq!(
        result.rows[0].cells[1],
        cell(CellState::Set, true, Some(false))
    );
    assert_eq!(result.rows[0].issues, ["duplicate", "differs"]);
    assert_eq!(
        result.rows[1].cells[1],
        cell(CellState::Missing, false, None)
    );
    assert_eq!(result.rows[1].issues, ["missing"]);
    assert_eq!(result.rows[2].cells[0], cell(CellState::Empty, false, None));
    assert_eq!(result.rows[2].issues, ["empty", "differs"]);
    assert_eq!(
        result.rows[3].cells[0],
        cell(CellState::Missing, false, None)
    );
    assert_eq!(result.rows[3].issues, ["extra"]);

    assert_eq!(
        result.summaries[0],
        Summary {
            entries: 3,
            empty: 1,
            ..Summary::default()
        }
    );
    assert_eq!(
        result.summaries[1],
        Summary {
            entries: 3,
            missing: 1,
            extra: 1,
            duplicate: 1,
            differs: 2,
            ..Summary::default()
        }
    );
}

#[test]
fn compares_values_literally_without_shell_expansion() {
    let base = parse("HOME_DIR=$HOME\nCMD=$(whoami)\n");
    let other = parse("HOME_DIR=$HOME\nCMD='$(whoami)'\n");
    let result = compare(&[base, other]);
    assert!(result.rows.iter().all(|row| row.issues.is_empty()));
}

#[test]
fn merges_missing_keys_after_their_predecessor() {
    let base = parse(
        "# db\nDB_HOST=localhost\n# port of db\nDB_PORT=\"5432\" # default\n\nexport API_KEY='secret'\n",
    );
    let target = parse("# production\nDB_HOST=db.internal\n\nOTHER=1\n");
    let (lines, added) = merge(&base, &target, false);
    assert_eq!(added, ["DB_PORT", "API_KEY"]);
    assert_eq!(
        serialize(&lines),
        "# production\nDB_HOST=db.internal\n# port of db\nDB_PORT=\"\" # default\nexport API_KEY=''\n\nOTHER=1\n"
    );

    let (lines, _) = merge(&base, &target, true);
    assert!(serialize(&lines).contains("DB_PORT=\"5432\" # default\nexport API_KEY='secret'\n"));
}

#[test]
fn appends_keys_without_an_anchor_and_keeps_existing_lines() {
    let base = parse("FIRST=1\nSECOND=2\nFIRST=3\n");
    let target = parse("# only comments\nSECOND=x");
    let (lines, added) = merge(&base, &target, false);
    // 基准中的重复键只合并一次
    assert_eq!(added, ["FIRST"]);
    assert_eq!(serialize(&lines), "# only comments\nSECOND=x\n\nFIRST=\n");

    let (lines, added) = merge(&target, &target, false);
    assert!(added.is_empty());
    assert_eq!(lines, target);
}
