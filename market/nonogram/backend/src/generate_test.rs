use super::*;
use crate::solver::{MAX_NODES, SolveResult, extract_clues};

#[test]
fn generates_unique_for_all_sizes() {
    for size in SIZES {
        let puzzle = generate(GenerateArgs {
            size,
            seed: Some(42),
        })
        .unwrap();
        assert_eq!(puzzle.size, size);
        // 唯一性：求解结果与交付图案一致
        assert_eq!(
            solver::solve(&puzzle.clues, MAX_NODES),
            SolveResult::Solved(puzzle.solution.clone())
        );
        // 线索与图案自洽
        assert_eq!(extract_clues(&puzzle.solution, size), puzzle.clues);
    }
}

#[test]
fn seed_reproduces_puzzle() {
    let a = generate(GenerateArgs {
        size: 5,
        seed: Some(7),
    })
    .unwrap();
    let b = generate(GenerateArgs {
        size: 5,
        seed: Some(7),
    })
    .unwrap();
    assert_eq!(a.clues, b.clues);
    assert_eq!(a.solution, b.solution);
}

#[test]
fn rejects_bad_size() {
    let err = generate(GenerateArgs {
        size: 7,
        seed: None,
    })
    .unwrap_err();
    assert_eq!(err.code, "puz.bad_size");
}
