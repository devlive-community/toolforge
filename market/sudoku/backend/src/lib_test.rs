use super::*;
use serde_json::json;

#[test]
fn generates_unique_puzzles() {
    for seed in [1u64, 42, 2026] {
        let out = generate(GenerateArgs {
            size: 9,
            holes: 40,
            seed,
        })
        .unwrap();
        assert_eq!(out.puzzle.len(), 81);
        assert_eq!(out.solution.len(), 81);
        let mut probe = out.puzzle.clone();
        let mut steps = 0u64;
        assert_eq!(
            count_solutions(&mut probe, 9, 2, &mut steps),
            1,
            "seed {seed} must be unique"
        );
        // 同一种子结果一致
        let again = generate(GenerateArgs {
            size: 9,
            holes: 40,
            seed,
        })
        .unwrap();
        assert_eq!(again.puzzle, out.puzzle);
    }
}

#[test]
fn generates_4x4_and_validates_params() {
    let out = generate(GenerateArgs {
        size: 4,
        holes: 8,
        seed: 7,
    })
    .unwrap();
    assert_eq!(out.puzzle.iter().filter(|v| **v == 0).count(), 8);
    let err = generate(GenerateArgs {
        size: 6,
        holes: 4,
        seed: 1,
    })
    .unwrap_err();
    assert_eq!(err.code, "sudoku.bad_size");
    let err = generate(GenerateArgs {
        size: 9,
        holes: 65,
        seed: 1,
    })
    .unwrap_err();
    assert_eq!(err.code, "sudoku.bad_holes");
}

#[test]
fn analyze_classifies_grid_statuses() {
    let generated = generate(GenerateArgs {
        size: 4,
        holes: 8,
        seed: 11,
    })
    .unwrap();
    let out = analyze(AnalyzeArgs {
        size: 4,
        grid: generated.puzzle.clone(),
    })
    .unwrap();
    assert_eq!(out.status, "unique");
    let out = analyze(AnalyzeArgs {
        size: 4,
        grid: generated.solution.clone(),
    })
    .unwrap();
    assert_eq!(out.status, "solved");

    // 题面冲突：第一行两个 1
    let mut grid = vec![0u8; 16];
    grid[0] = 1;
    grid[1] = 1;
    let out = analyze(AnalyzeArgs { size: 4, grid }).unwrap();
    assert_eq!(out.status, "conflicts");
    assert!(out.conflicts.iter().any(|c| c.kind == "row"));

    // 无解：r1c1=1、r1c2=2，r2c1 放不下 1/2 组合的经典矛盾（同宫 1 已在 c1 行占用后仍可继续，用列矛盾构造）
    let mut grid = vec![0u8; 16];
    grid[0] = 1; // r1c1
    grid[1] = 2; // r1c2
    grid[2] = 3; // r1c3
    grid[3] = 4; // r1c4
    grid[4] = 2; // r2c1 与 r1c2 同宫？4x4 宫为 2x2：r1-2/c1-2，合法。构造无解：固定 r2c2=1（宫内与 r1c1 冲突）
    grid[5] = 1;
    let out = analyze(AnalyzeArgs { size: 4, grid }).unwrap();
    assert_eq!(out.status, "conflicts");

    let err = analyze(AnalyzeArgs {
        size: 9,
        grid: vec![0; 16],
    })
    .unwrap_err();
    assert_eq!(err.code, "sudoku.invalid_grid");
}

#[test]
fn hint_explains_only_candidate() {
    // 构造 4×4：某空格三面被占，只剩唯一候选
    let mut grid = vec![0u8; 16];
    // r1 = 1 2 3 _（c4 空，行缺 4）
    grid[0] = 1;
    grid[1] = 2;
    grid[2] = 3;
    // c4 列已有 4：r4c4 = 4
    grid[15] = 4;
    let mut solution = grid.clone();
    solution[3] = 4;
    // 修正列冲突：r4c4=4 与行 4 无冲突即可（1,2,3,4 每行一次）
    grid[15] = 4;
    solution = vec![1, 2, 3, 4, 3, 4, 1, 2, 2, 1, 4, 3, 4, 3, 2, 1];
    let mut puzzle = vec![0u8; 16];
    for (i, v) in solution.iter().enumerate() {
        puzzle[i] = if [0usize, 1, 2, 15, 5, 6, 9].contains(&i) {
            *v
        } else {
            0
        };
    }
    let out = hint(HintArgs {
        size: 4,
        grid: puzzle,
        solution,
    })
    .unwrap();
    assert_eq!(out.value, 4, "r1c4 is the only candidate in its row");
    assert!(matches!(
        out.reason,
        "only_candidate" | "row_exhaust" | "column_exhaust" | "box_exhaust"
    ));
}

#[test]
fn hint_rejects_wrong_progress() {
    let generated = generate(GenerateArgs {
        size: 4,
        holes: 8,
        seed: 3,
    })
    .unwrap();
    let mut wrong = generated.puzzle.clone();
    // 在空格里填一个与解不一致的数
    let index = wrong.iter().position(|v| *v == 0).unwrap();
    wrong[index] = ((generated.solution[index] % 4) + 1) as u8;
    let err = hint(HintArgs {
        size: 4,
        grid: wrong,
        solution: generated.solution,
    })
    .unwrap_err();
    assert_eq!(err.code, "sudoku.conflicting_solution");
}

#[test]
fn dispatches_via_plugin() {
    let tool = Sudoku::default();
    let out = tool
        .call("generate", json!({"size": 4, "holes": 6, "seed": 99}))
        .unwrap();
    assert_eq!(out["size"], 4);
    let out = tool
        .call(
            "analyze",
            json!({"size": 4, "grid": [1, 2, 3, 4, 3, 4, 1, 2, 2, 1, 4, 3, 4, 3, 2, 1]}),
        )
        .unwrap();
    assert_eq!(out["status"], "solved");
    assert_eq!(
        tool.call("nope", json!({})).unwrap_err().code,
        "plugin.function_not_found"
    );
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked = tf_plugin_api::interface::check_declared_interfaces(&Sudoku::default()).unwrap();
    assert_eq!(checked, 3);
}
