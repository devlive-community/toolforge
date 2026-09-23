use super::*;

#[test]
fn lists_iana_timezones() {
    let zones = list();
    assert!(zones.all.iter().any(|z| z == "Asia/Shanghai"));
    assert!(zones.all.iter().any(|z| z == "America/New_York"));
    assert!(zones.all.windows(2).all(|w| w[0] < w[1]));
}

#[test]
fn resolves_special_names() {
    assert!(resolve("local").is_ok());
    assert_eq!(resolve("UTC").unwrap(), TimeZone::UTC);
    assert_eq!(
        resolve("Nope/Zone").unwrap_err().code,
        "time.invalid_timezone"
    );
}
