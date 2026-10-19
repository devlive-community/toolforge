use super::*;

#[test]
fn extracts_runs_and_empty_line() {
    assert_eq!(extract_clue(&[true, true, false, true]), vec![2, 1]);
    assert_eq!(extract_clue(&[false, false]), vec![0]);
    assert_eq!(extract_clue(&[true; 5]), vec![5]);
}

#[test]
fn line_solver_determines_single_placement() {
    // 线索 [5] 在 5 格线上：全部确定为黑
    let out = solve_line(&[5], &[CELL_UNKNOWN; 5]).unwrap();
    assert!(out.iter().all(|&value| value == Some(true)));
    // 线索 [0]：全部为白
    let out = solve_line(&[0], &[CELL_UNKNOWN; 5]).unwrap();
    assert!(out.iter().all(|&value| value == Some(false)));
    // 线索 [2,2] 在 5 格：唯一放置 (0-1)(3-4)，格 2 白
    let out = solve_line(&[2, 2], &[CELL_UNKNOWN; 5]).unwrap();
    assert_eq!(
        out,
        vec![Some(true), Some(true), Some(false), Some(true), Some(true)]
    );
}

#[test]
fn line_solver_respects_known_cells() {
    // 已知两端白，线索 [2] → 中段 (1,2) 或 (2,3)：格 2 必黑、两端白、1/3 未定
    let mut line = [CELL_UNKNOWN; 5];
    line[0] = CELL_WHITE;
    line[4] = CELL_WHITE;
    let out = solve_line(&[2], &line).unwrap();
    assert_eq!(out, vec![Some(false), None, Some(true), None, Some(false)]);
    // 与已知黑矛盾：线索 [3] 但已标 4 白（仅 1 格可用）→ 无解
    let mut line = [CELL_WHITE; 5];
    line[2] = CELL_UNKNOWN;
    assert!(solve_line(&[3], &line).is_none());
}

#[test]
fn solve_unique_5x5() {
    // 简单十字图案：唯一解
    // # . # . #
    // . # # # .
    // # # # # #
    // . # # # .
    // # . # . #
    let solution = vec![
        true, false, true, false, true, //
        false, true, true, true, false, //
        true, true, true, true, true, //
        false, true, true, true, false, //
        true, false, true, false, true,
    ];
    let clues = extract_clues(&solution, 5);
    assert_eq!(solve(&clues, MAX_NODES), SolveResult::Solved(solution));
}

#[test]
fn solve_detects_multiple_solutions() {
    // 对角图案 [T,F;F,T]：行列都是 [1],[1] → 两个解（两条对角线）
    let solution = vec![true, false, false, true];
    let clues = extract_clues(&solution, 2);
    assert_eq!(solve(&clues, MAX_NODES), SolveResult::Multiple);
}

#[test]
fn solve_detects_unsolvable() {
    // 行线索 [3] 但列线索全 [0] → 矛盾
    let clues = Clues {
        size: 2,
        rows: vec![vec![3], vec![0]],
        cols: vec![vec![0], vec![0]],
    };
    assert_eq!(solve(&clues, MAX_NODES), SolveResult::Unsolvable);
}

#[test]
fn check_lines_done_and_conflict() {
    let clues = Clues {
        size: 2,
        rows: vec![vec![1], vec![0]],
        cols: vec![vec![1], vec![0]],
    };
    // 玩家已正确完成：黑在 (0,0)，全行无未定
    let board = vec![CELL_BLACK, CELL_WHITE, CELL_WHITE, CELL_WHITE];
    let (row_done, col_done, row_conflict, col_conflict) = check_lines(&clues, &board);
    assert_eq!(row_done, vec![true, true]);
    assert_eq!(col_done, vec![true, true]);
    assert!(!row_conflict.iter().any(|&x| x));
    assert!(!col_conflict.iter().any(|&x| x));
    // 行 0 黑格片段匹配但仍有未定格：不算完成，也不算矛盾
    let board = vec![CELL_BLACK, CELL_UNKNOWN, CELL_WHITE, CELL_WHITE];
    let (row_done, _col_done, row_conflict, _col_conflict) = check_lines(&clues, &board);
    assert_eq!(row_done, vec![false, true]);
    assert!(!row_conflict[0]);
    // 行 0 黑格在错位：(0,1) 黑 → 与列线索 [1]（第 0 列）矛盾
    let board = vec![CELL_WHITE, CELL_BLACK, CELL_WHITE, CELL_WHITE];
    let (_row_done, _col_done, _row_conflict, col_conflict) = check_lines(&clues, &board);
    assert!(col_conflict[0]);
}

#[test]
fn hint_finds_overlap_and_explains_source() {
    // 行 0：线索 [3] 在 5 格 → 中间格必黑（行推理）
    let clues = Clues {
        size: 5,
        rows: vec![vec![3], vec![0], vec![0], vec![0], vec![0]],
        cols: vec![vec![0], vec![0], vec![1], vec![0], vec![0]],
    };
    let board = vec![CELL_UNKNOWN; 25];
    let hint = find_hint(&clues, &board).unwrap();
    assert_eq!((hint.row, hint.col), (0, 2));
    assert!(hint.black);
    assert_eq!(hint.via, 0);
    // 行 0 已无单线新格（格 2 黑后 [3] 仍三种摆法）、行 1-4 已完成 → 列推理接手
    let clues = Clues {
        size: 5,
        rows: vec![vec![3], vec![0], vec![0], vec![0], vec![0]],
        cols: vec![vec![0], vec![0], vec![1], vec![1], vec![0]],
    };
    let mut board = vec![CELL_UNKNOWN; 25];
    board[2] = CELL_BLACK;
    for r in 1..5 {
        for c in 0..5 {
            board[r * 5 + c] = CELL_WHITE;
        }
    }
    let hint = find_hint(&clues, &board).unwrap();
    assert_eq!(hint.via, 1);
    // 无法单线推理时如实返回：5x5 每行每列 [1]（排列格局，无候选重合）
    let clues = Clues {
        size: 5,
        rows: vec![vec![1], vec![1], vec![1], vec![1], vec![1]],
        cols: vec![vec![1], vec![1], vec![1], vec![1], vec![1]],
    };
    let board = vec![CELL_UNKNOWN; 25];
    assert!(find_hint(&clues, &board).is_none());
}
