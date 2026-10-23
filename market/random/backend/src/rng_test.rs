use rand::Rng;

use super::*;

#[test]
fn seeds_are_reproducible_and_separate_per_operation() {
    let a: Vec<u32> = (0..5)
        .map(|_| make("numbers", Some("2026-10-09")).random())
        .collect();
    assert!(
        a.windows(2).all(|w| w[0] == w[1]),
        "same seed, same first value"
    );
    let mut x = make("numbers", Some("2026-10-09"));
    let mut y = make("numbers", Some(" 2026-10-09 "));
    assert_eq!(
        x.random::<u64>(),
        y.random::<u64>(),
        "surrounding spaces are ignored"
    );
    let mut other = make("draw", Some("2026-10-09"));
    assert_ne!(
        make("numbers", Some("2026-10-09")).random::<u64>(),
        other.random::<u64>()
    );
    // 没有种子时每次都不同
    assert_ne!(
        make("numbers", None).random::<u128>(),
        make("numbers", Some("")).random::<u128>()
    );
}
