use super::*;
use serde_json::json;

#[test]
fn deals_unique_undercover_and_renames_duplicates() {
    let out = deal(DealArgs {
        players: vec!["张三".into(), "张三".into(), "李四".into(), "王五".into()],
        category: Some("food".into()),
        custom_pair: None,
        seed: 5,
    })
    .unwrap();
    assert_eq!(out.players.len(), 4);
    assert_eq!(out.players[0].name, "张三");
    assert_eq!(out.players[1].name, "张三(2)");
    assert_eq!(out.category, "food");
    assert_eq!(out.pair, ("火锅".into(), "麻辣烫".into()));
    let undercover: Vec<&PlayerWord> = out
        .players
        .iter()
        .filter(|p| p.role == "undercover")
        .collect();
    assert_eq!(undercover.len(), 1);
    for player in &out.players {
        let expected = if player.role == "undercover" {
            &out.pair.1
        } else {
            &out.pair.0
        };
        assert_eq!(&player.word, expected);
    }
    // 同一种子分配一致
    let again = deal(DealArgs {
        players: vec!["张三".into(), "张三".into(), "李四".into(), "王五".into()],
        category: Some("food".into()),
        custom_pair: None,
        seed: 5,
    })
    .unwrap();
    assert_eq!(again.players[0].role, out.players[0].role);
}

#[test]
fn deal_validates_players_and_custom_pair() {
    let err = deal(DealArgs {
        players: vec!["a".into(), "b".into(), "c".into()],
        category: None,
        custom_pair: None,
        seed: 1,
    })
    .unwrap_err();
    assert_eq!(err.code, "undercover.bad_player_count");
    let err = deal(DealArgs {
        players: vec!["a".into(), " ".into(), "c".into(), "d".into()],
        category: None,
        custom_pair: None,
        seed: 1,
    })
    .unwrap_err();
    assert_eq!(err.code, "undercover.empty_name");
    let err = deal(DealArgs {
        players: vec!["a".into(), "b".into(), "c".into(), "d".into()],
        category: Some("nope".into()),
        custom_pair: None,
        seed: 1,
    })
    .unwrap_err();
    assert_eq!(err.code, "undercover.bad_category");
    let out = deal(DealArgs {
        players: vec!["a".into(), "b".into(), "c".into(), "d".into()],
        category: None,
        custom_pair: Some(("词甲".into(), "词乙".into())),
        seed: 1,
    })
    .unwrap();
    assert_eq!(out.category, "custom");
    assert!(out.players.iter().any(|p| p.word == "词乙"));
}

#[test]
fn tally_majority_eliminates_and_checks_victory() {
    // 5 人存活，卧底 3 号；4 票集中 3 号 → 出局 + 卧底剩 2 人（存活 4）→ 继续
    let out = tally(TallyArgs {
        alive: vec![0, 1, 2, 3, 4],
        votes: vec![Some(3), Some(3), Some(3), Some(0), None],
        runoff: false,
        runoff_candidates: vec![],
        undercover: 3,
    })
    .unwrap();
    assert_eq!(
        out.status, "victory_civilian",
        "the undercover was voted out"
    );
    assert_eq!(out.eliminated, Some(3));
    assert_eq!(out.alive, vec![0, 1, 2, 4]);
    // 4 人存活出 4 号（非卧底）→ 存活 3 且卧底仍在 → 卧底胜
    let out = tally(TallyArgs {
        alive: vec![0, 1, 2, 4],
        votes: vec![Some(4), Some(4), Some(4), Some(0)],
        runoff: false,
        runoff_candidates: vec![],
        undercover: 3,
    })
    .unwrap();
    assert_eq!(out.status, "victory_undercover");
    // 卧底自己被票出 → 平民胜
    let out = tally(TallyArgs {
        alive: vec![0, 1, 2, 3],
        votes: vec![Some(3), Some(3), Some(3), None],
        runoff: false,
        runoff_candidates: vec![],
        undercover: 3,
    })
    .unwrap();
    assert_eq!(out.status, "victory_civilian");
}

#[test]
fn tally_tie_then_runoff_stalemate() {
    // 平票 → tie + 候选
    let out = tally(TallyArgs {
        alive: vec![0, 1, 2, 3],
        votes: vec![Some(1), Some(0), None, None],
        runoff: false,
        runoff_candidates: vec![],
        undercover: 2,
    })
    .unwrap();
    assert_eq!(out.status, "tie");
    assert_eq!(out.runoff_candidates, vec![0, 1]);
    // 重投仍平 → 无人出局
    let out = tally(TallyArgs {
        alive: vec![0, 1, 2, 3],
        votes: vec![Some(1), Some(0), None, None],
        runoff: true,
        runoff_candidates: vec![0, 1],
        undercover: 2,
    })
    .unwrap();
    assert_eq!(out.status, "runoff_stalemate");
    assert_eq!(out.eliminated, None);
    assert_eq!(out.alive, vec![0, 1, 2, 3]);
}

#[test]
fn tally_rejects_self_and_bad_targets() {
    let err = tally(TallyArgs {
        alive: vec![0, 1, 2, 3],
        votes: vec![Some(0), Some(2), None, None],
        runoff: false,
        runoff_candidates: vec![],
        undercover: 2,
    })
    .unwrap_err();
    assert_eq!(err.code, "undercover.self_vote");
    let err = tally(TallyArgs {
        alive: vec![0, 1, 2, 3],
        votes: vec![Some(9), None, None, None],
        runoff: false,
        runoff_candidates: vec![],
        undercover: 2,
    })
    .unwrap_err();
    assert_eq!(err.code, "undercover.bad_target");
    // 重投轮只投候选 0：一票 vs 零票 → 0 出局（存活 3 且卧底 2 仍在 → 卧底胜）
    let out = tally(TallyArgs {
        alive: vec![0, 1, 2, 3],
        votes: vec![Some(0), None, None, None],
        runoff: true,
        runoff_candidates: vec![0, 1],
        undercover: 2,
    })
    .unwrap();
    assert_eq!(out.status, "victory_undercover");
}

#[test]
fn dispatches_via_plugin() {
    let tool = UndercoverHost::default();
    let out = tool
        .call(
            "deal",
            json!({"players": ["a", "b", "c", "d"], "category": "tech", "seed": 3}),
        )
        .unwrap();
    assert_eq!(out["pair"].as_array().unwrap().len(), 2);
    assert_eq!(
        tool.call("nope", json!({})).unwrap_err().code,
        "plugin.function_not_found"
    );
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&UndercoverHost::default()).unwrap();
    assert_eq!(checked, 2);
}
