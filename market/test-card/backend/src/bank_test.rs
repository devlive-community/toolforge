use super::*;

#[test]
fn registry_contains_only_unique_unionpay_credit_prefixes() {
    let catalog = catalog();
    assert_eq!(catalog.banks.len(), 18);
    let mut ids = std::collections::BTreeSet::new();
    let mut bins = std::collections::BTreeSet::new();
    for definition in catalog.banks {
        assert!(ids.insert(definition.id.id()));
        assert_eq!(definition.brand, Brand::Unionpay);
        assert_eq!(definition.card_type, "credit");
        assert!(!definition.prefixes.is_empty());
        assert!(definition.prefixes.len() <= 3);
        for prefix in definition.prefixes {
            assert_eq!(prefix.bin.len(), 6);
            assert!(prefix.bin.starts_with("62"));
            assert!(prefix.bin.bytes().all(|byte| byte.is_ascii_digit()));
            assert_eq!(prefix.length, 16);
            assert!(bins.insert(prefix.bin));
            let sample = format!("{}0000000000", prefix.bin);
            assert_eq!(detect(&sample), Some((definition.id, prefix.bin)));
        }
        assert_eq!(
            serde_json::to_value(definition.id).unwrap(),
            definition.id.id()
        );
        assert_eq!(
            serde_json::from_value::<Bank>(serde_json::json!(definition.id.id())).unwrap(),
            definition.id
        );
    }
}

#[test]
fn registry_does_not_infer_unsupported_lengths_or_debit_bins() {
    for input in [
        "622168",
        "622168000000000",
        "62216800000000000",
        "6225880000000000",
        "6221660000000000",
        "6200000000000005",
        "622168000000000x",
        "招行6225750000000000",
    ] {
        assert_eq!(detect(input), None, "{input}");
    }
}

#[test]
fn sources_are_fixed_and_both_licenses_ship_in_the_catalog() {
    let serialized = serde_json::to_value(catalog()).unwrap();
    let source = &serialized["source"];
    assert_eq!(source["revision"].as_str().unwrap().len(), 40);
    assert_eq!(
        source["comparedWith"]["revision"].as_str().unwrap().len(),
        40
    );
    assert_eq!(source["license"], "MIT");
    assert_eq!(source["checkedAt"], "2026-10-16");
    for license in [
        &source["licenseText"],
        &source["comparedWith"]["licenseText"],
    ] {
        let text = license.as_str().unwrap();
        assert!(text.contains("Permission is hereby granted, free of charge"));
        assert!(text.contains("THE SOFTWARE IS PROVIDED"));
    }
}
