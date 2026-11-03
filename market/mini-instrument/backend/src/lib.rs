//! 桌面小乐器后端：十二平均律音高（A4=440Hz）、和弦组成与琶音顺序。
//! 波形合成与键盘交互在前端完成。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct MiniInstrument {
    manifest: Manifest,
}

impl Default for MiniInstrument {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for MiniInstrument {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "keyboard" => to_value(keyboard(parse_args(args)?)?),
            "chord" => to_value(chord(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(MiniInstrument);

/// 音名（C 起）；黑键共用升号拼写
const NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];
const BLACK: [bool; 12] = [
    false, true, false, true, false, false, true, false, true, false, true, false,
];

fn midi_to_hz(midi: i32) -> f64 {
    440.0 * 2.0f64.powf((midi - 69) as f64 / 12.0)
}

fn midi_name(midi: i32) -> String {
    // MIDI 60 = C4（科学音高记谱）
    let name = NAMES[(midi.rem_euclid(12)) as usize];
    let octave = midi.div_euclid(12) - 1;
    format!("{name}{octave}")
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyboardArgs {
    /// 起始 MIDI 音高（两八度 = 24 键）
    pub start_midi: i32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Key {
    pub midi: i32,
    pub hz: f64,
    /// 音名（如 C4）
    pub name: String,
    /// 唱名（C=D…可空）
    pub solfege: &'static str,
    pub black: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyboardOutput {
    pub keys: Vec<Key>,
    pub start_midi: i32,
}

/// 两八度键盘（24 键），从 start_midi 起
pub fn keyboard(args: KeyboardArgs) -> PluginResult<KeyboardOutput> {
    if !(21..=96).contains(&args.start_midi) {
        return Err(
            PluginError::new("instrument.bad_range").with("startMidi", args.start_midi as i64)
        );
    }
    const SOLFEGES: [&str; 12] = [
        "do", "di", "re", "ri", "mi", "fa", "fi", "sol", "si", "la", "li", "ti",
    ];
    let keys = (0..24)
        .map(|offset| {
            let midi = args.start_midi + offset;
            let degree = midi.rem_euclid(12) as usize;
            Key {
                midi,
                hz: (midi_to_hz(midi) * 100.0).round() / 100.0,
                name: midi_name(midi),
                solfege: SOLFEGES[degree],
                black: BLACK[degree],
            }
        })
        .collect();
    Ok(KeyboardOutput {
        keys,
        start_midi: args.start_midi,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChordArgs {
    /// 根音 MIDI
    pub root_midi: i32,
    /// major / minor / dom7
    pub kind: String,
    /// 琶音每音间隔毫秒（0 表示只返回整组）
    #[serde(default)]
    pub arpeggio_ms: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChordNote {
    pub midi: i32,
    pub hz: f64,
    pub name: String,
    /// 和弦内角色键：root / third / fifth / seventh
    pub role: &'static str,
    /// 琶音播放时刻（毫秒；非琶音为 0）
    pub at_ms: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChordOutput {
    pub kind: String,
    pub name: String,
    pub notes: Vec<ChordNote>,
}

pub fn chord(args: ChordArgs) -> PluginResult<ChordOutput> {
    if !(21..=96).contains(&args.root_midi) {
        return Err(
            PluginError::new("instrument.bad_range").with("rootMidi", args.root_midi as i64)
        );
    }
    let intervals: &[i32] = match args.kind.as_str() {
        "major" => &[0, 4, 7],
        "minor" => &[0, 3, 7],
        "dom7" => &[0, 4, 7, 10],
        other => return Err(PluginError::new("instrument.bad_chord").with("kind", other)),
    };
    let roles = ["root", "third", "fifth", "seventh"];
    let suffix = match args.kind.as_str() {
        "major" => "",
        "minor" => "m",
        _ => "7",
    };
    let root_name = NAMES[args.root_midi.rem_euclid(12) as usize];
    let name = format!("{root_name}{suffix}");
    let arpeggio_ms = args.arpeggio_ms.unwrap_or(0);
    let notes = intervals
        .iter()
        .enumerate()
        .map(|(index, interval)| {
            let midi = args.root_midi + interval;
            ChordNote {
                midi,
                hz: (midi_to_hz(midi) * 100.0).round() / 100.0,
                name: midi_name(midi),
                role: roles[index],
                at_ms: if arpeggio_ms > 0 {
                    (index as u64) * arpeggio_ms
                } else {
                    0
                },
            }
        })
        .collect();
    Ok(ChordOutput {
        kind: args.kind,
        name,
        notes,
    })
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
