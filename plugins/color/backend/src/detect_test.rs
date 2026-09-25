use super::*;

#[test]
fn detects_colors() {
    assert_eq!(
        detect("#16A34A").unwrap(),
        Detection::new(92, "color").with("hex", "#16a34a")
    );
    assert_eq!(detect("rgb(255 0 0)").unwrap().params["hex"], "#ff0000");
    assert!(detect("#zzz").is_none());
    assert!(detect("#1").is_none());
    assert!(detect("red").is_none());
    assert!(detect("0xff0000").is_none());
}
