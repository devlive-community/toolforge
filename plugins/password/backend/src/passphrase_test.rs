use std::collections::HashSet;

use rand::SeedableRng;
use rand::rngs::StdRng;

use super::*;

fn args(words: usize) -> Args {
    Args {
        words,
        count: 1,
        wordlist: Wordlist::En,
        separator: "-".into(),
        case: Case::Lower,
        append_digit: false,
    }
}

#[test]
fn wordlists_are_unique_and_clean() {
    let en = words(Wordlist::En);
    assert_eq!(en.len(), 7772);
    assert!(
        en.iter()
            .all(|(w, _)| !w.is_empty() && w.bytes().all(|b| b.is_ascii_lowercase()))
    );
    assert_eq!(
        en.iter().map(|(w, _)| w).collect::<HashSet<_>>().len(),
        en.len()
    );

    let zh = words(Wordlist::Zh);
    assert!(zh.len() >= 512);
    assert!(zh.iter().all(|(p, hint)| {
        p.bytes().all(|b| b.is_ascii_lowercase()) && hint.is_some_and(|h| !h.is_ascii())
    }));
    assert_eq!(
        zh.iter().map(|(p, _)| p).collect::<HashSet<_>>().len(),
        zh.len()
    );
}

#[test]
fn builds_words_with_separator() {
    let mut rng = StdRng::seed_from_u64(1);
    let mut a = args(5);
    a.count = 20;
    a.separator = "_".into();
    let out = generate_with(a, &mut rng).unwrap();
    assert_eq!(out.passwords.len(), 20);
    let list: HashSet<_> = words(Wordlist::En).iter().map(|(w, _)| *w).collect();
    for password in &out.passwords {
        let parts: Vec<_> = password.split('_').collect();
        assert_eq!(parts.len(), 5);
        assert!(parts.iter().all(|p| list.contains(p)));
    }
    assert!(out.hints.is_none());
    // log2(7772) × 5 ≈ 64.6
    assert_eq!(out.entropy, 64.6);
}

#[test]
fn title_case_and_digit() {
    let mut rng = StdRng::seed_from_u64(2);
    let mut a = args(4);
    a.case = Case::Title;
    a.append_digit = true;
    a.separator = " ".into();
    let out = generate_with(a, &mut rng).unwrap();
    let parts: Vec<_> = out.passwords[0].split(' ').collect();
    assert!(
        parts
            .iter()
            .all(|p| p.chars().next().unwrap().is_ascii_uppercase())
    );
    assert_eq!(
        parts
            .iter()
            .filter(|p| p.chars().last().unwrap().is_ascii_digit())
            .count(),
        1
    );
    assert!(out.entropy > entropy(7772, 4, false, Case::Title));
}

#[test]
fn chinese_list_returns_pinyin_and_hints() {
    let mut rng = StdRng::seed_from_u64(3);
    let mut a = args(4);
    a.wordlist = Wordlist::Zh;
    a.count = 3;
    let out = generate_with(a, &mut rng).unwrap();
    let hints = out.hints.unwrap();
    assert_eq!(hints.len(), 3);
    assert!(out.passwords[0].is_ascii());
    assert_eq!(hints[0].split(' ').count(), 4);
}

#[test]
fn validates_options() {
    let mut rng = StdRng::seed_from_u64(4);
    assert_eq!(
        generate_with(args(2), &mut rng).unwrap_err().code,
        "password.invalid_words"
    );
    let mut a = args(4);
    a.separator = "0123456789".into();
    assert_eq!(
        generate_with(a, &mut rng).unwrap_err().code,
        "password.invalid_separator"
    );
}
