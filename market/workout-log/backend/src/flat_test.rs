use super::*;

fn args(sets: &[&str]) -> VolumeArgs {
    VolumeArgs {
        sets: sets.iter().map(|s| s.to_string()).collect(),
        unit: Unit::Kg,
    }
}

fn row(sets: &[&str]) -> VolumeRow {
    volume(args(sets)).unwrap().summary.remove(0)
}

#[test]
fn volume_sums_reps_times_kilograms() {
    let r = row(&["8x60kg", "8x60", "6 × 70 kg"]);
    assert_eq!((r.sets, r.reps), (3, 22));
    assert_eq!(r.volume_kg, "1380.000");
    assert_eq!(r.heaviest_kg, "70.000");
}

#[test]
fn pounds_are_converted_like_the_app() {
    let r = row(&["5x135lb", "5x135lbs"]);
    // 135 lb = 61.234970 kg
    assert_eq!(r.heaviest_kg, "61.235");
    assert_eq!(r.volume_kg, "612.350");
}

#[test]
fn bodyweight_sets_count_reps_but_no_volume() {
    let r = row(&["12", "10"]);
    assert_eq!(r.reps, 22);
    assert_eq!(r.volume_kg, "0.000");
    assert_eq!(r.heaviest_kg, "");
}

#[test]
fn malformed_sets_are_rejected() {
    assert!(volume(args(&["x60"])).is_err());
    assert!(volume(args(&["8x-5"])).is_err());
    assert!(volume(args(&["eightx60"])).is_err());
    assert!(volume(args(&[])).is_err());
}
