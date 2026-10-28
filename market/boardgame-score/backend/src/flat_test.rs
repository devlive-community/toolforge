use super::*;

fn args(players: &[&str], direction: Direction) -> RankArgs {
    RankArgs {
        players: players.iter().map(|s| s.to_string()).collect(),
        direction,
    }
}

fn got(rows: Vec<Row>) -> Vec<(usize, String, i64)> {
    rows.into_iter()
        .map(|r| (r.rank, r.name, r.total))
        .collect()
}

#[test]
fn ties_share_a_rank_and_the_next_rank_is_skipped() {
    let rows = rank(args(&["Ann:10/5", "Bo:15/0", "Cy:-2/-3"], Direction::High)).unwrap();
    assert_eq!(
        got(rows),
        vec![
            (1, "Ann".into(), 15),
            (1, "Bo".into(), 15),
            (3, "Cy".into(), -5)
        ]
    );
}

#[test]
fn low_score_wins_when_asked() {
    let rows = rank(args(&["Ann:3", "Bo:1"], Direction::Low)).unwrap();
    assert_eq!(got(rows)[0], (1, "Bo".into(), 1));
}

#[test]
fn names_may_contain_colons_and_repeat() {
    let rows = rank(args(&["Team: red:4", "Team: red:2"], Direction::High)).unwrap();
    assert_eq!(rows[0].name, "Team: red");
    assert_eq!(rows.len(), 2);
}

#[test]
fn invalid_input_follows_app_rules() {
    assert_eq!(
        rank(args(&["Ann:1"], Direction::High)).unwrap_err().code,
        "boardgame-score.players"
    );
    assert_eq!(
        rank(args(&["Ann:1/2", "Bo:1"], Direction::High))
            .unwrap_err()
            .code,
        "boardgame-score.incomplete"
    );
    assert_eq!(
        rank(args(&["Ann:1.5", "Bo:1"], Direction::High))
            .unwrap_err()
            .code,
        "boardgame-score.score"
    );
    assert_eq!(
        rank(args(&["Ann:9999999", "Bo:1"], Direction::High))
            .unwrap_err()
            .code,
        "boardgame-score.score"
    );
    assert_eq!(
        rank(args(&["Ann", "Bo:1"], Direction::High))
            .unwrap_err()
            .code,
        "boardgame-score.incomplete"
    );
}
