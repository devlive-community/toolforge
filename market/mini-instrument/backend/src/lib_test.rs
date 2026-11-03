use super::*;
use serde_json::json;

#[test]
fn a4_is_440_and_octaves_double() {
    assert!((midi_to_hz(69) - 440.0).abs() < 1e-9);
    assert!((midi_to_hz(81) - 880.0).abs() < 1e-9);
    assert!((midi_to_hz(57) - 220.0).abs() < 1e-9);
}

#[test]
fn keyboard_covers_two_octaves_with_names() {
    let out = keyboard(KeyboardArgs { start_midi: 60 }).unwrap();
    assert_eq!(out.keys.len(), 24);
    assert_eq!(out.keys[0].name, "C4");
    assert_eq!(out.keys[0].solfege, "do");
    assert_eq!(out.keys[12].name, "C5");
    assert!(!out.keys[0].black);
    assert!(out.keys[1].black);
    assert_eq!(out.keys[0].hz, 261.63);
    let err = keyboard(KeyboardArgs { start_midi: 5 }).unwrap_err();
    assert_eq!(err.code, "instrument.bad_range");
}

#[test]
fn chords_have_expected_intervals() {
    let out = chord(ChordArgs {
        root_midi: 60,
        kind: "major".into(),
        arpeggio_ms: None,
    })
    .unwrap();
    assert_eq!(out.name, "C");
    let midis: Vec<i32> = out.notes.iter().map(|n| n.midi).collect();
    assert_eq!(midis, vec![60, 64, 67]);
    let out = chord(ChordArgs {
        root_midi: 60,
        kind: "minor".into(),
        arpeggio_ms: None,
    })
    .unwrap();
    assert_eq!(out.name, "Cm");
    let midis: Vec<i32> = out.notes.iter().map(|n| n.midi).collect();
    assert_eq!(midis, vec![60, 63, 67]);
    let out = chord(ChordArgs {
        root_midi: 60,
        kind: "dom7".into(),
        arpeggio_ms: Some(120),
    })
    .unwrap();
    assert_eq!(out.name, "C7");
    assert_eq!(out.notes.len(), 4);
    let ats: Vec<u64> = out.notes.iter().map(|n| n.at_ms).collect();
    assert_eq!(ats, vec![0, 120, 240, 360]);
    let err = chord(ChordArgs {
        root_midi: 60,
        kind: "sus9".into(),
        arpeggio_ms: None,
    })
    .unwrap_err();
    assert_eq!(err.code, "instrument.bad_chord");
}

#[test]
fn dispatches_via_plugin() {
    let out = MiniInstrument::default()
        .call("keyboard", json!({"startMidi": 60}))
        .unwrap();
    assert_eq!(out["keys"].as_array().unwrap().len(), 24);
    assert_eq!(
        MiniInstrument::default()
            .call("nope", json!({}))
            .unwrap_err()
            .code,
        "plugin.function_not_found"
    );
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&MiniInstrument::default()).unwrap();
    assert_eq!(checked, 2);
}
