use super::*;

#[test]
fn knows_tracking_parameters() {
    for key in [
        "utm_source",
        "UTM_Campaign",
        "fbclid",
        "gclid",
        "spm",
        "vd_source",
    ] {
        assert!(is_tracking(key), "{key}");
    }
    for key in ["id", "page", "q", "utm", "ref"] {
        assert!(!is_tracking(key), "{key}");
    }
}
