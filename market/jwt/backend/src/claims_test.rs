use super::*;
use serde_json::json;

#[test]
fn reports_expiry_and_not_before() {
    let payload = json!({"exp": 1000, "nbf": 3000, "iat": 500, "sub": "x"});
    let status = status(&payload, 2000);
    assert_eq!(status.expired, Some(true));
    assert_eq!(status.not_yet_valid, Some(true));
    let exp = status.times.iter().find(|t| t.name == "exp").unwrap();
    assert_eq!(exp.relative_seconds, -1000);
    assert_eq!(exp.iso.as_deref(), Some("1970-01-01T00:16:40Z"));
}

#[test]
fn missing_claims_are_unknown() {
    let status = status(&json!({"sub": "x"}), 0);
    assert!(status.times.is_empty());
    assert_eq!((status.expired, status.not_yet_valid), (None, None));
}

#[test]
fn accepts_fractional_numeric_dates() {
    let status = status(&json!({"exp": 1516239022.5}), 0);
    assert_eq!(status.times[0].value, 1516239022);
}
