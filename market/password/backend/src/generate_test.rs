use rand::SeedableRng;
use rand::rngs::StdRng;

use super::*;

fn args(length: usize) -> Args {
    Args {
        length,
        count: 1,
        upper: true,
        lower: true,
        digits: true,
        symbols: false,
        exclude_ambiguous: false,
        exclude: String::new(),
        require_each: true,
    }
}

#[test]
fn generates_requested_length_and_count() {
    let mut rng = StdRng::seed_from_u64(7);
    let mut a = args(24);
    a.count = 50;
    let out = generate_with(a, &mut rng).unwrap();
    assert_eq!(out.passwords.len(), 50);
    assert!(out.passwords.iter().all(|p| p.chars().count() == 24));
    assert_eq!(out.pool_size, 62);
    assert_eq!(out.entropy, 142.9);
}

#[test]
fn every_class_is_present_when_required() {
    let mut rng = StdRng::seed_from_u64(1);
    let mut a = args(4);
    a.symbols = true;
    a.count = 200;
    for password in generate_with(a, &mut rng).unwrap().passwords {
        assert!(password.chars().any(|c| UPPER.contains(c)), "{password}");
        assert!(password.chars().any(|c| LOWER.contains(c)), "{password}");
        assert!(password.chars().any(|c| DIGITS.contains(c)), "{password}");
        assert!(password.chars().any(|c| SYMBOLS.contains(c)), "{password}");
    }
}

#[test]
fn exclusions_are_respected() {
    let mut rng = StdRng::seed_from_u64(3);
    let mut a = args(64);
    a.exclude_ambiguous = true;
    a.exclude = "abcXYZ".into();
    a.count = 20;
    for password in generate_with(a, &mut rng).unwrap().passwords {
        assert!(
            !password
                .chars()
                .any(|c| AMBIGUOUS.contains(c) || "abcXYZ".contains(c))
        );
    }
}

#[test]
fn validates_options() {
    let mut rng = StdRng::seed_from_u64(0);
    assert_eq!(
        generate_with(args(0), &mut rng).unwrap_err().code,
        "password.invalid_length"
    );
    let mut none = args(8);
    none.upper = false;
    none.lower = false;
    none.digits = false;
    assert_eq!(
        generate_with(none, &mut rng).unwrap_err().code,
        "password.no_charset"
    );
    let mut excluded = args(8);
    excluded.upper = false;
    excluded.lower = false;
    excluded.exclude = DIGITS.into();
    assert_eq!(
        generate_with(excluded, &mut rng).unwrap_err().code,
        "password.no_charset"
    );
    assert_eq!(
        generate_with(args(2), &mut rng).unwrap_err().code,
        "password.too_short_for_classes"
    );
    let mut many = args(8);
    many.count = 501;
    assert_eq!(
        generate_with(many, &mut rng).unwrap_err().code,
        "password.invalid_count"
    );
}

#[test]
fn default_rng_produces_distinct_passwords() {
    let mut a = args(32);
    a.count = 10;
    let out = generate(a).unwrap();
    let mut unique = out.passwords.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), 10);
}
