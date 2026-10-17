use super::*;

fn make(brand: Brand, count: usize, seed: Option<&str>) -> Generated {
    generate(GenerateArgs {
        brand,
        bank: None,
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
            bank: None,
            count: 0,
            seed: None
        })
        .unwrap_err()
        .code,
        "card.invalid_count"
    );
}

#[test]
fn generic_generation_keeps_the_original_seed_sequence() {
    // Captured from the pre-bank generator with this seed and fixed UTC month.
    let expected = [
        (
            Brand::Visa,
            "4995058863933591",
            "NOAH PLACEHOLDER",
            "09/31",
            "429",
        ),
        (
            Brand::Mastercard,
            "2404995058863936",
            "QUINN EXAMPLE",
            "08/28",
            "599",
        ),
        (
            Brand::Amex,
            "349950588639332",
            "MIA FIXTURE",
            "09/29",
            "9942",
        ),
        (
            Brand::Discover,
            "6459950588639335",
            "CASEY DUMMY",
            "09/29",
            "994",
        ),
        (
            Brand::Jcb,
            "3551995058863935",
            "QUINN EXAMPLE",
            "08/28",
            "599",
        ),
        (
            Brand::Unionpay,
            "6299505886393359",
            "MIA FIXTURE",
            "05/31",
            "942",
        ),
        (Brand::Diners, "30099505886396", "MIA MOCK", "10/28", "359"),
    ];
    for (brand, number, holder, expiry, cvc) in expected {
        let generated = card(
            brand,
            None,
            &mut rng(Some("legacy-stable-seed")),
            (2026, 10),
        );
        assert_eq!(generated.number, number);
        assert_eq!(generated.holder, holder);
        assert_eq!(generated.expiry, expiry);
        assert_eq!(generated.cvc, cvc);
        assert_eq!(generated.bank, None);
        assert_eq!(generated.bin, None);
    }
    let exported: serde_json::Value =
        serde_json::from_str(&make(Brand::Visa, 1, Some("x")).json).unwrap();
    assert_eq!(exported["cards"][0].as_object().unwrap().len(), 5);
    assert!(exported.get("bankSource").is_none());
}

#[test]
fn domestic_banks_generate_only_curated_credit_card_shapes() {
    let now = current_month();
    for bank in bank::ALL {
        let args = || GenerateArgs {
            brand: Brand::Unionpay,
            bank: Some(bank),
            count: 100,
            seed: Some("domestic-banks".into()),
        };
        let first = generate(args()).unwrap();
        let repeated = generate(args()).unwrap();
        assert_eq!(first.cards, repeated.cards);
        assert!(first.seeded);
        let mut generated_prefixes = std::collections::BTreeSet::new();
        for generated in first.cards {
            let bin = generated.bin.as_ref().unwrap();
            assert_eq!(generated.bank, Some(bank));
            assert_eq!(generated.brand, Brand::Unionpay);
            assert_eq!(brand::detect(&generated.number), Some(Brand::Unionpay));
            assert_eq!(generated.number.len(), 16);
            assert!(generated.number.starts_with(bin));
            assert!(
                bank.prefixes()
                    .iter()
                    .any(|prefix| prefix.bin == bin && prefix.length == 16)
            );
            assert!(brand::luhn_valid(&generated.number));
            assert_eq!(generated.cvc.len(), 3);
            assert!(generated.cvc.bytes().all(|byte| byte.is_ascii_digit()));
            let (month, year) = generated.expiry.split_once('/').unwrap();
            let total = 2000 + year.parse::<i32>().unwrap();
            assert!(total * 12 + month.parse::<i32>().unwrap() > now.0 * 12 + now.1 as i32);
            assert_eq!(bank::detect(&generated.number), Some((bank, bin.as_str())));
            generated_prefixes.insert(bin.clone());
        }
        assert_eq!(generated_prefixes.len(), bank.prefixes().len());
    }
}

#[test]
fn domestic_exports_preserve_bank_bin_and_reference_license() {
    for bank in bank::ALL {
        let generated = generate(GenerateArgs {
            brand: Brand::Unionpay,
            bank: Some(bank),
            count: 3,
            seed: Some("export".into()),
        })
        .unwrap();
        let exported: serde_json::Value = serde_json::from_str(&generated.json).unwrap();
        assert_eq!(exported["purpose"], "test-only");
        assert_eq!(exported["mode"], "local-format");
        assert_eq!(
            exported["bankSource"]["revision"],
            bank::SOURCE.reference.revision
        );
        assert_eq!(exported["bankSource"]["license"], "MIT");
        assert!(
            exported["bankSource"]["licenseText"]
                .as_str()
                .unwrap()
                .contains("Runrioter")
        );
        assert!(
            exported["bankSource"]["comparedWith"]["licenseText"]
                .as_str()
                .unwrap()
                .contains("caijf")
        );
        let mut lines = generated.csv.lines();
        assert_eq!(
            lines.next(),
            Some("purpose,mode,brand,bank,bin,number,holder,expiry,cvc")
        );
        for (index, (line, card)) in lines.zip(&generated.cards).enumerate() {
            let fields: Vec<_> = line.split(',').collect();
            assert_eq!(fields.len(), 9);
            assert_eq!(
                &fields[..4],
                &["test-only", "local-format", "unionpay", bank.id()]
            );
            assert_eq!(fields[4], card.bin.as_deref().unwrap());
            assert_eq!(fields[5], card.number);
            assert_eq!(exported["cards"][index]["bank"], bank.id());
            assert_eq!(exported["cards"][index]["bin"], fields[4]);
            assert_eq!(exported["cards"][index]["number"], fields[5]);
        }
    }
}

#[test]
fn bank_selection_requires_unionpay_and_retains_count_limits() {
    for brand in brand::ALL
        .into_iter()
        .filter(|brand| *brand != Brand::Unionpay)
    {
        let error = generate(GenerateArgs {
            brand,
            bank: Some(Bank::Cmb),
            count: 1,
            seed: None,
        })
        .unwrap_err();
        assert_eq!(error.code, "card.bank_brand_mismatch");
        assert_eq!(error.params["brand"], "unionpay");
    }
    assert_eq!(
        generate(GenerateArgs {
            brand: Brand::Unionpay,
            bank: Some(Bank::Ccb),
            count: MAX_COUNT + 1,
            seed: None,
        })
        .unwrap_err()
        .code,
        "card.invalid_count"
    );
    assert_eq!(
        generate(GenerateArgs {
            brand: Brand::Unionpay,
            bank: Some(Bank::Ccb),
            count: MAX_COUNT,
            seed: Some("maximum".into()),
        })
        .unwrap()
        .cards
        .len(),
        MAX_COUNT
    );
}

#[test]
fn validator_reports_only_matching_reference_rows() {
    for bank in bank::ALL {
        let generated = generate(GenerateArgs {
            brand: Brand::Unionpay,
            bank: Some(bank),
            count: 1,
            seed: Some("validation".into()),
        })
        .unwrap();
        let card = &generated.cards[0];
        let formatted = card
            .number
            .as_bytes()
            .chunks(4)
            .map(|part| std::str::from_utf8(part).unwrap())
            .collect::<Vec<_>>()
            .join("-");
        let checked = validate(ValidateArgs { input: formatted }).unwrap();
        assert!(checked[0].problems.is_empty());
        assert_eq!(checked[0].bank, Some(bank));
        assert_eq!(checked[0].bin, card.bin.as_deref());
        assert!(checked[0].sandbox.is_none());
        let mut bad_check_digit = card.number.clone();
        let last = bad_check_digit.pop().unwrap() as u8;
        bad_check_digit.push(char::from(b'0' + (last - b'0' + 1) % 10));
        let invalid_luhn = validate(ValidateArgs {
            input: bad_check_digit,
        })
        .unwrap();
        assert_eq!(invalid_luhn[0].bank, Some(bank));
        assert_eq!(invalid_luhn[0].problems, vec![Problem::Luhn]);
        let invalid_character = validate(ValidateArgs {
            input: format!("{}x", card.number),
        })
        .unwrap();
        assert_eq!(invalid_character[0].bank, None);
        assert_eq!(invalid_character[0].bin, None);
    }
    for number in [
        "6200000000000005",
        "6225880000000000",
        "6221680000000000000",
        "4242424242424242",
    ] {
        let checked = validate(ValidateArgs {
            input: number.into(),
        })
        .unwrap();
        assert_eq!(checked[0].bank, None);
        assert_eq!(checked[0].bin, None);
        let value = serde_json::to_value(&checked[0]).unwrap();
        assert!(value.get("bank").is_none());
        assert!(value.get("bin").is_none());
    }
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
