use super::*;

fn conv(category: &str, from: &str, value: &str) -> Output {
    convert(Args {
        category: category.into(),
        from: from.into(),
        value: value.into(),
        precision: 10,
    })
    .unwrap()
}

fn get<'a>(out: &'a Output, unit: &str) -> &'a str {
    &out.results.iter().find(|r| r.unit == unit).unwrap().value
}

#[test]
fn length_and_chinese_units() {
    let out = conv("length", "mi", "1");
    assert_eq!(get(&out, "m"), "1609.344");
    assert_eq!(get(&out, "km"), "1.609344");
    assert_eq!(get(&out, "ft"), "5280");
    let out = conv("length", "chi", "3");
    assert_eq!(get(&out, "m"), "1");
    assert_eq!(get(&out, "cun"), "30");
}

#[test]
fn temperature_is_affine() {
    let out = conv("temperature", "c", "100");
    assert_eq!(
        (get(&out, "f"), get(&out, "k"), get(&out, "r")),
        ("212", "373.15", "671.67")
    );
    assert_eq!(get(&conv("temperature", "f", "-40"), "c"), "-40");
    assert_eq!(get(&conv("temperature", "k", "0"), "c"), "-273.15");
    let err = convert(Args {
        category: "temperature".into(),
        from: "k".into(),
        value: "-1".into(),
        precision: 10,
    })
    .unwrap_err();
    assert_eq!(err.code, "unit.below_absolute_zero");
}

#[test]
fn data_sizes_decimal_and_binary() {
    let out = conv("data", "gib", "1");
    assert_eq!(get(&out, "mib"), "1024");
    assert_eq!(get(&out, "byte"), "1073741824");
    assert_eq!(get(&out, "gb"), "1.073741824");
    assert_eq!(get(&conv("datarate", "mbps", "100"), "mbyteps"), "12.5");
}

#[test]
fn other_categories() {
    assert_eq!(get(&conv("mass", "jin", "1"), "g"), "500");
    assert_eq!(get(&conv("area", "mu", "15"), "ha"), "1");
    assert_eq!(get(&conv("volume", "gal", "1"), "l"), "3.785411784");
    assert_eq!(get(&conv("speed", "kmh", "36"), "mps"), "10");
    assert_eq!(get(&conv("time", "d", "1"), "min"), "1440");
    assert_eq!(get(&conv("pressure", "atm", "1"), "kpa"), "101.325");
    assert_eq!(get(&conv("energy", "kwh", "1"), "kj"), "3600");
    assert_eq!(get(&conv("angle", "turn", "0.5"), "rad"), "3.141592654");
    assert_eq!(get(&conv("frequency", "rpm", "120"), "hz"), "2");
    assert_eq!(get(&conv("power", "kw", "1"), "w"), "1000");
}

#[test]
fn parses_and_formats_numbers() {
    assert_eq!(parse_value("1,234.5").unwrap(), 1234.5);
    assert_eq!(parse_value(" 2e3 ").unwrap(), 2000.0);
    assert_eq!(parse_value("").unwrap_err().code, "unit.empty");
    assert_eq!(parse_value("abc").unwrap_err().code, "unit.invalid_number");
    assert_eq!(parse_value("inf").unwrap_err().code, "unit.invalid_number");

    assert_eq!(format(0.1 + 0.2, 10), "0.3");
    assert_eq!(format(1.0 / 3.0, 4), "0.3333");
    assert_eq!(format(123_456.789, 4), "123457");
    assert_eq!(format(1.602_176_634e-19, 6), "1.60218e-19");
    assert_eq!(format(6.02e23, 3), "6.02e23");
    assert_eq!(format(-0.000_000_01, 3), "-1e-8");
    assert_eq!(format(0.0, 5), "0");
}

#[test]
fn every_unit_round_trips() {
    for category in CATEGORIES {
        for unit in category.units {
            let out = conv(category.id, unit.id, "1");
            assert_eq!(get(&out, unit.id), "1", "{}:{}", category.id, unit.id);
            assert_eq!(out.results.len(), category.units.len());
        }
        assert!(
            category.units.iter().any(|u| u.id == category.base),
            "{}",
            category.id
        );
    }
    assert_eq!(catalog().len(), CATEGORIES.len());
}

#[test]
fn rejects_unknown_ids() {
    let err = |category: &str, from: &str| {
        convert(Args {
            category: category.into(),
            from: from.into(),
            value: "1".into(),
            precision: 10,
        })
        .unwrap_err()
        .code
    };
    assert_eq!(err("nope", "m"), "unit.unknown_category");
    assert_eq!(err("length", "kg"), "unit.unknown_unit");
}
