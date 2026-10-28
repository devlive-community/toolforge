use std::cell::Cell;
use std::collections::BTreeMap;

use super::*;
use image::Rgba;
use tf_plugin_api::LogLevel;

struct Ctx {
    cancel_after: Option<u64>,
    progress: std::sync::Mutex<u64>,
}

impl Ctx {
    fn new() -> Self {
        Self {
            cancel_after: None,
            progress: std::sync::Mutex::new(0),
        }
    }
}

impl TaskContext for Ctx {
    fn log(&self, _: LogLevel, _: &str, _: serde_json::Value) {}
    fn progress(&self, done: u64, _: u64) {
        *self.progress.lock().unwrap() = done;
    }
    fn stage(&self, _: &str) {}
    fn is_cancelled(&self) -> bool {
        self.cancel_after
            .is_some_and(|n| *self.progress.lock().unwrap() >= n)
    }
    fn open_file(&self, _: &str) -> PluginResult<Box<dyn std::io::Read + Send>> {
        unreachable!()
    }
    fn file_size(&self, _: &str) -> PluginResult<u64> {
        unreachable!()
    }
}

#[derive(Default)]
struct Memory {
    files: BTreeMap<String, Vec<u8>>,
    writes: Cell<usize>,
}

impl Sink for Memory {
    fn write(&mut self, path: &str, data: &[u8]) -> PluginResult<()> {
        self.writes.set(self.writes.get() + 1);
        self.files.insert(path.to_string(), data.to_vec());
        Ok(())
    }
}

/// 第一帧左上角一块红色，第二帧全透明
fn frames() -> Vec<String> {
    let mut first = RgbaImage::new(240, 180);
    for y in 0..20 {
        for x in 0..20 {
            first.put_pixel(x, y, Rgba([255, 0, 0, 255]));
        }
    }
    vec![frame::encode(&first).unwrap(), String::new()]
}

fn request(format: Format, path: &str) -> Request {
    Request {
        format,
        width: 240,
        height: 180,
        delay_ms: 150,
        background: "#ffffff".into(),
        frames: frames(),
        path: path.into(),
        name: "ball".into(),
    }
}

#[test]
fn gif_loops_forever_with_the_preview_timing_and_no_leftovers() {
    let mut sink = Memory::default();
    let out = run(&request(Format::Gif, "/tmp/a.gif"), &Ctx::new(), &mut sink).unwrap();
    assert_eq!(out.frames, 2);
    let bytes = &sink.files["/tmp/a.gif"];
    let mut options = gif::DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::RGBA);
    let mut decoder = options.read_info(bytes.as_slice()).unwrap();
    assert_eq!((decoder.width(), decoder.height()), (240, 180));
    assert_eq!(decoder.repeat(), gif::Repeat::Infinite);
    let mut seen = Vec::new();
    while let Some(frame) = decoder.read_next_frame().unwrap() {
        assert_eq!(frame.delay, 15, "150 ms = 15 个 GIF 时间单位");
        assert_eq!((frame.width, frame.height), (240, 180), "每帧整幅覆盖");
        seen.push(frame.buffer.to_vec());
    }
    assert_eq!(seen.len(), 2);
    // 第一帧左上角为红色；第二帧同一位置是底色白色，不残留上一帧
    assert_eq!(&seen[0][0..3], &[255, 0, 0]);
    assert_eq!(&seen[1][0..4], &[255, 255, 255, 255]);
}

#[test]
fn png_sequence_keeps_transparency_and_numbers_files_in_order() {
    let mut sink = Memory::default();
    let out = run(&request(Format::Png, "/tmp/out"), &Ctx::new(), &mut sink).unwrap();
    assert_eq!(out.files, 2);
    let names: Vec<_> = sink.files.keys().cloned().collect();
    assert_eq!(
        names,
        vec!["/tmp/out/ball-001.png", "/tmp/out/ball-002.png"]
    );
    let second = image::load_from_memory(&sink.files["/tmp/out/ball-002.png"])
        .unwrap()
        .into_rgba8();
    assert_eq!(second.dimensions(), (240, 180));
    assert_eq!(second.get_pixel(0, 0)[3], 0, "PNG 序列保留透明");
}

#[test]
fn cancelling_a_gif_export_writes_nothing() {
    let mut sink = Memory::default();
    let ctx = Ctx {
        cancel_after: Some(1),
        progress: std::sync::Mutex::new(0),
    };
    let err = run(&request(Format::Gif, "/tmp/a.gif"), &ctx, &mut sink).unwrap_err();
    assert_eq!(err.code, tf_plugin_api::cancelled().code);
    assert_eq!(sink.writes.get(), 0);
}

#[test]
fn a_broken_frame_fails_before_any_file_is_written() {
    let mut sink = Memory::default();
    let mut req = request(Format::Png, "/tmp/out");
    req.frames[1] = "%%%".into();
    assert!(run(&req, &Ctx::new(), &mut sink).is_err());
    assert_eq!(sink.writes.get(), 0);
}

#[test]
fn invalid_delay_is_rejected() {
    let mut req = request(Format::Gif, "/tmp/a.gif");
    req.delay_ms = 125;
    assert_eq!(
        run(&req, &Ctx::new(), &mut Memory::default())
            .unwrap_err()
            .code,
        "flipbook.bad_delay"
    );
}

#[test]
fn file_names_are_sanitized_and_paths_joined_per_platform() {
    assert_eq!(file_stem("../a/b:c"), "..abc".trim_matches('.'));
    assert_eq!(file_stem("   "), "frame");
    assert_eq!(join("C:\\out", "a.png"), "C:\\out\\a.png");
    assert_eq!(join("/tmp/out/", "a.png"), "/tmp/out/a.png");
}
