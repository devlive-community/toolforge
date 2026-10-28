use super::*;
use crate::tests::Context;

fn args(seed: &str) -> DrawArgs {
    DrawArgs {
        rows: 6,
        cols: 8,
        seed: Some(seed.into()),
        entrance: Side::Left,
        exit: Side::Right,
        answer: false,
    }
}

#[test]
fn same_seed_draws_the_same_maze_and_code() {
    let a = draw(args("cli"), &Context::default()).unwrap();
    let b = draw(args("cli"), &Context::default()).unwrap();
    assert_eq!(a, b);
    assert!(a.code.starts_with("M1.6.8."));
    // 7 行墙线 + 6 行格子，每行 8 格 × 4 字符 + 1
    let lines: Vec<&str> = a.text.lines().collect();
    assert_eq!(lines.len(), 13);
    assert!(lines.iter().all(|l| l.chars().count() == 33));
    assert_eq!(a.text.matches('S').count(), 1);
    assert_eq!(a.text.matches('E').count(), 1);
}

#[test]
fn the_code_reopens_the_same_maze_in_the_app() {
    let out = draw(args("round-trip"), &Context::default()).unwrap();
    let spec = Spec::from_code(&out.code).unwrap();
    assert_eq!(spec.seed, "round-trip");
    assert_eq!(spec.id(), out.id);
}

#[test]
fn answer_marks_the_route_between_start_and_exit() {
    let mut a = args("route");
    a.answer = true;
    let out = draw(a, &Context::default()).unwrap();
    assert_eq!(out.text.matches('·').count(), out.route_cells - 2);
}

#[test]
fn entrance_and_exit_are_open_to_the_outside() {
    let out = draw(args("doors"), &Context::default()).unwrap();
    let lines: Vec<&str> = out.text.lines().collect();
    // 入口在左边第一行，出口在右边最后一行
    assert!(lines[1].starts_with(' '));
    assert!(lines[11].ends_with(' '));
}

#[test]
fn size_limits_and_default_seed() {
    let mut a = args("x");
    a.rows = 1;
    assert_eq!(draw(a, &Context::default()).unwrap_err().code, "maze.size");
    let mut a = args("x");
    a.seed = None;
    assert!(
        draw(a, &Context::default())
            .unwrap()
            .seed
            .starts_with("cli-")
    );
    let mut a = args("x");
    a.entrance = Side::Right;
    a.rows = 2;
    a.cols = 2;
    a.exit = Side::Right;
    // 右边第一格与最后一格不同，可以生成
    assert!(draw(a, &Context::default()).is_ok());
}
