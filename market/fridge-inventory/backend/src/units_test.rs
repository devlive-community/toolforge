use super::*;

#[test]
fn converts_within_dimension() {
    assert_eq!(convert(1.5, "kg", "g"), Ok(1500.0));
    assert_eq!(convert(250.0, "g", "kg"), Ok(0.25));
    assert_eq!(convert(2.0, "l", "ml"), Ok(2000.0));
    assert_eq!(convert(1.0, "g", "g"), Ok(1.0));
}

#[test]
fn rejects_cross_dimension_and_count_guesses() {
    // 重量与体积不换
    assert_eq!(convert(1.0, "g", "ml"), Err(UnitError::Mismatch));
    // 包与个不猜换算
    assert_eq!(convert(1.0, "pack", "piece"), Err(UnitError::Mismatch));
    assert_eq!(convert(1.0, "kg", "pack"), Err(UnitError::Mismatch));
    // 未知单位
    assert_eq!(convert(1.0, "cup", "g"), Err(UnitError::Unknown));
    assert_eq!(to_base(1.0, "cup"), Err(UnitError::Unknown));
}

#[test]
fn base_roundtrip_is_stable() {
    let base = to_base(0.75, "kg").unwrap();
    let back = from_base(base, "kg").unwrap();
    assert!((back - 0.75).abs() < 1e-12);
    assert!(known("ml"));
    assert!(!known("spoon"));
}
