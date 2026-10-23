use super::*;
use crate::rng::make;

fn seeded(op: &str) -> ChaCha20Rng {
    make(op, Some("test seed"))
}

#[test]
fn draws_numbers_in_range() {
    let values = numbers(&mut seeded("numbers"), 1, 6, 1000, false, false).unwrap();
    assert!(values.iter().all(|v| (1..=6).contains(v)));
    assert!((1..=6).all(|n| values.contains(&n)), "every face appears");
    let unique = numbers(&mut seeded("numbers"), 1, 50, 50, true, true).unwrap();
    assert_eq!(unique, (1..=50).collect::<Vec<_>>());
    let huge = numbers(&mut seeded("numbers"), i64::MIN, i64::MAX, 100, true, false).unwrap();
    assert_eq!(
        huge.iter().collect::<std::collections::HashSet<_>>().len(),
        100
    );
    assert_eq!(
        numbers(&mut seeded("n"), 5, 1, 1, false, false)
            .unwrap_err()
            .code,
        "random.invalid_range"
    );
    assert_eq!(
        numbers(&mut seeded("n"), 1, 3, 4, true, false)
            .unwrap_err()
            .code,
        "random.not_enough_numbers"
    );
    assert_eq!(
        numbers(&mut seeded("n"), 1, 3, 0, false, false)
            .unwrap_err()
            .code,
        "random.invalid_count"
    );
}

/// 固定算法：同一个种子必须一直得到同样的结果，否则以前公布的抽签无法验证
#[test]
fn pins_the_seeded_algorithm() {
    let pinned = numbers(
        &mut make("numbers", Some("ToolForge")),
        1,
        100,
        5,
        false,
        false,
    )
    .unwrap();
    let items = parse_items("Ann\nBob\nCid\nDan\nEve", false).unwrap();
    let winners = draw(&mut make("draw", Some("ToolForge")), &items, 2).unwrap();
    assert_eq!(
        pinned,
        numbers(
            &mut make("numbers", Some("ToolForge")),
            1,
            100,
            5,
            false,
            false
        )
        .unwrap()
    );
    assert_eq!(pinned, PINNED_NUMBERS);
    assert_eq!(winners, PINNED_WINNERS);
}

const PINNED_NUMBERS: [i64; 5] = [40, 6, 24, 55, 84];
const PINNED_WINNERS: [&str; 2] = ["Cid", "Dan"];

#[test]
fn parses_names_and_weights() {
    let items = parse_items("  Ann \n\nBob*3\nC*D\n", true).unwrap_err();
    assert_eq!(items.code, "random.invalid_weight");
    let items = parse_items("Ann\nBob*3\n", true).unwrap();
    assert_eq!(
        items,
        [
            Candidate {
                name: "Ann".into(),
                weight: 1
            },
            Candidate {
                name: "Bob".into(),
                weight: 3
            }
        ]
    );
    let plain = parse_items("Bob*3", false).unwrap();
    assert_eq!(plain[0].name, "Bob*3", "weights only when enabled");
    assert_eq!(
        parse_items(" \n ", false).unwrap_err().code,
        "random.no_items"
    );
}

#[test]
fn draws_without_repeats_and_honours_weights() {
    let items = parse_items("A\nB\nC\nD", false).unwrap();
    let winners = draw(&mut seeded("draw"), &items, 4).unwrap();
    let mut sorted = winners.clone();
    sorted.sort();
    assert_eq!(sorted, ["A", "B", "C", "D"]);
    assert_eq!(
        draw(&mut seeded("draw"), &items, 5).unwrap_err().code,
        "random.not_enough_items"
    );
    // A 的权重是 B 的 9 倍：多次抽一个，A 明显更多
    let weighted = parse_items("A*9\nB", true).unwrap();
    let mut rng = seeded("draw");
    let a = (0..2000)
        .filter(|_| draw(&mut rng, &weighted, 1).unwrap()[0] == "A")
        .count();
    assert!((1700..1900).contains(&a), "{a}");
}

#[test]
fn splits_into_balanced_groups() {
    let items = parse_items(
        &(1..=10)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join("\n"),
        false,
    )
    .unwrap();
    let three = groups(&mut seeded("groups"), &items, Some(3), None, true).unwrap();
    assert_eq!(three.iter().map(Vec::len).collect::<Vec<_>>(), [4, 3, 3]);
    let mut all: Vec<String> = three.into_iter().flatten().collect();
    all.sort_by_key(|s| s.parse::<u32>().unwrap());
    assert_eq!(all.len(), 10);
    let fours = groups(&mut seeded("groups"), &items, None, Some(4), false).unwrap();
    assert_eq!(
        fours,
        [
            vec!["1", "2", "3", "4"],
            vec!["5", "6", "7"],
            vec!["8", "9", "10"]
        ]
    );
    assert_eq!(
        groups(&mut seeded("g"), &items, Some(11), None, true)
            .unwrap_err()
            .code,
        "random.not_enough_items"
    );
}

#[test]
fn rolls_dice() {
    let rolls = dice(&mut seeded("dice"), 600, 6).unwrap();
    assert!(rolls.iter().all(|r| (1..=6).contains(r)));
    let coins = dice(&mut seeded("dice"), 100, 2).unwrap();
    assert!(coins.contains(&1) && coins.contains(&2));
    assert_eq!(
        dice(&mut seeded("dice"), 1, 1).unwrap_err().code,
        "random.invalid_sides"
    );
}
