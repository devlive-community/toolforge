use super::*;
use crate::tests::{Context, created, spec};

#[test]
fn each_wall_is_unique_and_matches_exactly_one_closed_grid_edge() {
    let maze = generate(spec("geometry"), &Context::default()).unwrap();
    let lines = walls(&maze);
    let keys = lines
        .iter()
        .map(|l| l.map(|x| x as i32))
        .collect::<BTreeSet<_>>();
    assert_eq!(keys.len(), lines.len());
    for cell in 0..maze.passages.len() {
        let x = (cell % maze.spec.cols) as i32;
        let y = (cell / maze.spec.cols) as i32;
        for (side, line) in [
            (Side::Top, [x, y, x + 1, y]),
            (Side::Right, [x + 1, y, x + 1, y + 1]),
            (Side::Bottom, [x, y + 1, x + 1, y + 1]),
            (Side::Left, [x, y, x, y + 1]),
        ] {
            assert_eq!(keys.contains(&line), maze.passages[cell] & side.bit() == 0);
        }
    }
}
#[test]
fn play_view_withholds_solutions_and_deduplicates_only_visual_trail_edges() {
    let ctx = Context::default();
    let mut state = created();
    let maze = state.validate(&ctx).unwrap().unwrap();
    let route = maze.route(maze.start, &ctx).unwrap();
    let g = state.game.as_mut().unwrap();
    g.history = vec![maze.start, route[1], maze.start, route[1]];
    let output = view(g, &maze, &ctx).unwrap();
    assert!(output["answerPath"].is_null());
    assert_eq!(output["steps"], 3);
    assert_eq!(output["visited"], 2);
    assert_eq!(
        output["trailPath"].as_str().unwrap().matches('M').count(),
        1
    );
    g.show_trail = false;
    assert_eq!(view(g, &maze, &ctx).unwrap()["trailPath"], "");
}
