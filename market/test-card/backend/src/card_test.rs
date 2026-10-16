use super::*;

fn make(brand: Brand, count: usize, seed: Option<&str>) -> Generated {
    generate(GenerateArgs {
        brand,
        count,
        seed: seed.map(str::to_owned),
    })
    .unwrap()
}

#[test]
fn generated_cards_follow_brand_rules() {
    for b in brand::ALL {
        let out = make(b, 50, None);
        for c in &out.cards {
            assert_eq!(brand::detect(&c.number), Some(b), "{}", c.number);
            assert!(b.lengths().contains(&c.number.len()));
            assert!(brand::luhn_valid(&c.number));
            assert_eq!(c.cvc.len(), b.cvc_length());
            let (mm, yy) = c.expiry.split_once('/').unwrap();
            assert!((1..=12).contains(&mm.parse::<u32>().unwrap()));
            assert_eq!(yy.len(), 2);
        }
    }
}

#[test]
fn expiry_is_in_the_future() {
    let (year, month) = current_month();
    for c in make(Brand::Visa, 100, Some("future")).cards {
        let (mm, yy) = c.expiry.split_once('/').unwrap();
        let (mm, yy): (i32, i32) = (mm.parse().unwrap(), yy.parse().unwrap());
        let full = year / 100 * 100 + yy + if yy < year % 100 { 100 } else { 0 };
        assert!(full * 12 + mm > year * 12 + month as i32, "{}", c.expiry);
    }
}

#[test]
fn seeds_reproduce_results() {
    let a = make(Brand::Mastercard, 5, Some("demo"));
    let b = make(Brand::Mastercard, 5, Some("demo"));
    assert_eq!(a.cards, b.cards);
    assert!(a.seeded);
    assert_ne!(make(Brand::Mastercard, 5, Some("other")).cards, a.cards);
    assert!(a.csv.starts_with(
        "purpose,mode,brand,number,holder,expiry,cvc\ntest-only,local-format,mastercard,"
    ));
    assert!(a.json.contains("\"mode\": \"local-format\""));
    assert_eq!(
        generate(GenerateArgs {
            brand: Brand::Visa,
            count: 0,
            seed: None
        })
        .unwrap_err()
        .code,
        "card.invalid_count"
    );
}

#[test]
fn validates_numbers_with_reasons() {
    let out = validate(ValidateArgs { input: "4242 4242 4242 4242\n4242424242424241\n\n9999999999999995\n37828224631000\n4242-4242-4242-424x".into() }).unwrap();
    assert_eq!(out.len(), 5);
    assert!(out[0].problems.is_empty());
    assert_eq!(out[0].sandbox.as_ref().unwrap().scenario, "visa");
    assert_eq!(out[1].problems, vec![Problem::Luhn]);
    assert_eq!(out[1].sandbox.as_ref().unwrap().scenario, "incorrectNumber");
    assert_eq!(out[2].problems, vec![Problem::UnknownBrand]);
    assert!(out[3].problems.contains(&Problem::Length));
    assert_eq!(out[3].allowed_lengths, vec![15]);
    assert!(out[4].problems.contains(&Problem::Characters));
    assert_eq!(out[4].line, 6);
}
