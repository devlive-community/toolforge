use crate::{game::Game, maze::*};
use serde::Serialize;
use std::collections::BTreeSet;
use tf_plugin_api::{PluginResult, TaskContext};

pub type Line = [f32; 4];
#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Point {
    pub x: f32,
    pub y: f32,
}
pub fn center(maze: &Maze, cell: usize) -> Point {
    Point {
        x: (cell % maze.spec.cols) as f32 + 0.5,
        y: (cell / maze.spec.cols) as f32 + 0.5,
    }
}
pub fn walls(maze: &Maze) -> Vec<Line> {
    let mut lines = Vec::new();
    for (cell, &bits) in maze.passages.iter().enumerate() {
        let x = (cell % maze.spec.cols) as f32;
        let y = (cell / maze.spec.cols) as f32;
        if bits & 1 == 0 {
            lines.push([x, y, x + 1.0, y]);
        }
        if bits & 8 == 0 {
            lines.push([x, y, x, y + 1.0]);
        }
        if cell / maze.spec.cols + 1 == maze.spec.rows && bits & 4 == 0 {
            lines.push([x, y + 1.0, x + 1.0, y + 1.0]);
        }
        if cell % maze.spec.cols + 1 == maze.spec.cols && bits & 2 == 0 {
            lines.push([x + 1.0, y, x + 1.0, y + 1.0]);
        }
    }
    lines
}
pub fn path(lines: &[Line]) -> String {
    lines
        .iter()
        .map(|l| format!("M{} {}L{} {}", l[0], l[1], l[2], l[3]))
        .collect()
}
pub fn route_lines(maze: &Maze, route: &[usize]) -> Vec<Line> {
    route
        .windows(2)
        .map(|p| {
            let a = center(maze, p[0]);
            let b = center(maze, p[1]);
            [a.x, a.y, b.x, b.y]
        })
        .collect()
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cell {
    pub id: usize,
    pub x: usize,
    pub y: usize,
    pub adjacent: bool,
}
pub fn view(game: &Game, maze: &Maze, ctx: &dyn TaskContext) -> PluginResult<serde_json::Value> {
    let at = *game.history.last().unwrap();
    let mut edges = BTreeSet::new();
    for p in game.history.windows(2) {
        check(ctx)?;
        edges.insert((p[0].min(p[1]), p[0].max(p[1])));
    }
    let trail = if game.show_trail {
        edges
            .into_iter()
            .map(|(a, b)| {
                let a = center(maze, a);
                let b = center(maze, b);
                [a.x, a.y, b.x, b.y]
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let route = maze.route(maze.start, ctx)?;
    let solution = if game.answer_visible {
        Some(path(&route_lines(maze, &route)))
    } else {
        None
    };
    let visited = game.history.iter().copied().collect::<BTreeSet<_>>().len();
    let cells = (0..maze.passages.len())
        .map(|id| Cell {
            id,
            x: id % maze.spec.cols,
            y: id / maze.spec.cols,
            adjacent: maze.legal(at, id),
        })
        .collect::<Vec<_>>();
    Ok(
        serde_json::json!({"id":game.id,"code":game.spec.code(),"spec":game.spec,"cells":cells,"wallPath":path(&walls(maze)),"trailPath":path(&trail),"answerPath":solution,"player":center(maze,at),"start":center(maze,maze.start),"exit":center(maze,maze.exit),"hint":game.hint_target.map(|id|center(maze,id)),"position":at,"steps":game.history.len()-1,"visited":visited,"routeLength":route.len()-1,"cellCount":maze.passages.len(),"hints":game.hints,"revealed":game.revealed,"showTrail":game.show_trail,"answerVisible":game.answer_visible,"finished":at==maze.exit,"completion":game.completion}),
    )
}

pub struct Scene {
    pub width: f32,
    pub height: f32,
    pub lines: Vec<(Line, f32)>,
}
fn glyph(ch: char) -> [u8; 7] {
    match ch {
        '0' => [14, 17, 19, 21, 25, 17, 14],
        '1' => [4, 12, 4, 4, 4, 4, 14],
        '2' => [14, 17, 1, 2, 4, 8, 31],
        '3' => [30, 1, 1, 14, 1, 1, 30],
        '4' => [2, 6, 10, 18, 31, 2, 2],
        '5' => [31, 16, 16, 30, 1, 1, 30],
        '6' => [14, 16, 16, 30, 17, 17, 14],
        '7' => [31, 1, 2, 4, 8, 8, 8],
        '8' => [14, 17, 17, 14, 17, 17, 14],
        '9' => [14, 17, 17, 15, 1, 1, 14],
        'A' => [14, 17, 17, 31, 17, 17, 17],
        'B' => [30, 17, 17, 30, 17, 17, 30],
        'C' => [15, 16, 16, 16, 16, 16, 15],
        'D' => [30, 17, 17, 17, 17, 17, 30],
        'E' => [31, 16, 16, 30, 16, 16, 31],
        'F' => [31, 16, 16, 30, 16, 16, 16],
        'M' => [17, 27, 21, 21, 17, 17, 17],
        'N' => [17, 25, 21, 19, 17, 17, 17],
        'R' => [30, 17, 17, 30, 20, 18, 17],
        'S' => [15, 16, 16, 14, 1, 1, 30],
        'T' => [31, 4, 4, 4, 4, 4, 4],
        'U' => [17, 17, 17, 17, 17, 17, 14],
        'Z' => [31, 1, 2, 4, 8, 16, 31],
        'W' => [17, 17, 17, 21, 21, 27, 17],
        'X' => [17, 17, 10, 4, 10, 17, 17],
        '-' => [0, 0, 0, 31, 0, 0, 0],
        _ => [0; 7],
    }
}
fn text(scene: &mut Scene, value: &str, x: f32, y: f32, height: f32) {
    let step = height / 7.0;
    for (index, ch) in value.chars().enumerate() {
        for (row, bits) in glyph(ch).iter().enumerate() {
            for col in 0..5 {
                if bits & (1 << (4 - col)) != 0 {
                    let x = x + index as f32 * step * 6.0 + col as f32 * step;
                    let y = y + row as f32 * step + step / 2.0;
                    scene
                        .lines
                        .push(([x + step * 0.4, y, x + step * 0.6, y], step * 0.8));
                }
            }
        }
    }
}
pub fn scene(maze: &Maze, answer: bool, ctx: &dyn TaskContext) -> PluginResult<Scene> {
    let width = maze.spec.cols as f32 + 2.0;
    let height = maze.spec.rows as f32 + 3.0;
    let mut scene = Scene {
        width,
        height,
        lines: Vec::new(),
    };
    for line in walls(maze) {
        scene.lines.push((
            [line[0] + 1.0, line[1] + 2.0, line[2] + 1.0, line[3] + 2.0],
            0.065,
        ));
    }
    if answer {
        let route = route_lines(maze, &maze.route(maze.start, ctx)?);
        for (index, line) in route.iter().enumerate() {
            // Dashed route in the same grid coordinates, distinct in B&W.
            for (a, b) in [(0.08, 0.23), (0.43, 0.58), (0.78, 0.93)] {
                if (index == 0 && a < 0.3) || (index + 1 == route.len() && a > 0.7) {
                    continue;
                }
                scene.lines.push((
                    [
                        line[0] + (line[2] - line[0]) * a + 1.0,
                        line[1] + (line[3] - line[1]) * a + 2.0,
                        line[0] + (line[2] - line[0]) * b + 1.0,
                        line[1] + (line[3] - line[1]) * b + 2.0,
                    ],
                    0.08,
                ));
            }
        }
    }
    let heading = format!(
        "{} {}",
        if answer { "ANSWER" } else { "MAZE" },
        maze.spec.id()
    );
    let font = 0.5f32.min((width - 1.0) * 7.0 / (heading.len() as f32 * 6.0));
    text(&mut scene, &heading, 0.5, 0.25, font);
    text(
        &mut scene,
        &format!("{}X{}  S START  E END", maze.spec.cols, maze.spec.rows),
        0.5,
        1.05,
        font.min(0.4),
    );
    for (cell, label) in [(maze.start, "S"), (maze.exit, "E")] {
        let point = center(maze, cell);
        text(
            &mut scene,
            label,
            point.x + 1.0 - 0.16,
            point.y + 2.0 - 0.21,
            0.42,
        );
    }
    check(ctx)?;
    Ok(scene)
}
pub fn svg(scene: &Scene) -> String {
    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\" width=\"1200\" height=\"{}\"><rect width=\"100%\" height=\"100%\" fill=\"white\"/><g stroke=\"black\" stroke-linecap=\"square\">",
        scene.width,
        scene.height,
        (1200.0 * scene.height / scene.width).round()
    );
    for (line, width) in &scene.lines {
        out.push_str(&format!(
            "<path d=\"M{} {}L{} {}\" stroke-width=\"{}\"/>",
            line[0], line[1], line[2], line[3], width
        ));
    }
    out.push_str("</g></svg>");
    out
}

#[cfg(test)]
#[path = "drawing_test.rs"]
mod tests;
