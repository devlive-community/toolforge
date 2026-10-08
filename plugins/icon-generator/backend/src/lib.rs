//! 应用图标生成插件后端：由一张图片（PNG / JPEG / WebP / SVG 等）生成 Web、iOS、Android、
//! macOS 与 Windows 的整套图标，支持留白、背景色与圆角。

mod compose;
mod presets;
mod source;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use data_encoding::BASE64;
use serde::Deserialize;
use serde_json::{Value, json};
use tf_plugin_api::{
    LogLevel, Manifest, PluginError, PluginResult, TaskContext, ToolPlugin, cancelled, parse_args,
    unknown_function,
};

use compose::{MASTER, Shape, Style};
use presets::{Masters, Preset, Writer};
use source::Source;

const MANIFEST: &str = include_str!("../../manifest.json");
/// 小于这个尺寸的位图放大后会模糊
const SMALL: u32 = 512;
const PREVIEW: u32 = 256;

#[derive(Deserialize)]
struct PathArgs {
    path: String,
}

#[derive(Deserialize)]
struct RenderArgs {
    path: String,
    #[serde(default)]
    style: Style,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GenerateArgs {
    path: String,
    #[serde(default)]
    style: Style,
    presets: Vec<Preset>,
    output_dir: String,
    /// 写入 webmanifest 的应用名称；为空时用文件名
    #[serde(default)]
    name: Option<String>,
}

type Key = (String, Option<SystemTime>);

pub struct IconGenerator {
    manifest: Manifest,
    /// 最近读取的源图（SVG 渲染需要加载系统字体，较慢）
    last: Mutex<Option<(Key, Arc<Source>)>>,
}

fn stem(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "icon".to_owned())
}

fn data_uri(image: &image::RgbaImage) -> String {
    format!(
        "data:image/png;base64,{}",
        BASE64.encode(&presets::png_bytes(image))
    )
}

/// 不覆盖已有文件夹：name → name (1)
fn unique_dir(path: PathBuf) -> PathBuf {
    if !path.exists() {
        return path;
    }
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    (1..)
        .map(|n| path.with_file_name(format!("{name} ({n})")))
        .find(|p| !p.exists())
        .unwrap_or(path)
}

impl IconGenerator {
    fn source(&self, path: &str) -> PluginResult<Arc<Source>> {
        let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok();
        let key = (path.to_owned(), modified);
        let mut last = self.last.lock().unwrap();
        if let Some((cached, source)) = last.as_ref()
            && *cached == key
        {
            return Ok(source.clone());
        }
        let source = Arc::new(source::load(path, MASTER)?);
        *last = Some((key, source.clone()));
        Ok(source)
    }

    fn inspect(&self, args: PathArgs) -> PluginResult<Value> {
        let source = self.source(&args.path)?;
        let opaque = source.image.pixels().all(|p| p[3] == 255);
        Ok(json!({
            "path": args.path,
            "name": stem(&args.path),
            "width": source.width,
            "height": source.height,
            "svg": source.svg,
            "square": source.width == source.height,
            "transparent": !opaque,
            "small": !source.svg && source.width.max(source.height) < SMALL,
        }))
    }

    fn render(&self, args: RenderArgs) -> PluginResult<Value> {
        args.style.validate()?;
        let source = self.source(&args.path)?;
        let shapes = [
            ("rounded", Shape::Rounded),
            ("opaque", Shape::Opaque),
            ("circle", Shape::Circle),
            ("mac", Shape::Mac),
        ];
        let mut out = serde_json::Map::new();
        for (name, shape) in shapes {
            let image = compose::master(&source.image, &args.style, shape, PREVIEW);
            out.insert(name.to_owned(), data_uri(&image).into());
        }
        Ok(Value::Object(out))
    }

    fn generate(&self, args: GenerateArgs, ctx: &dyn TaskContext) -> PluginResult<Value> {
        args.style.validate()?;
        if args.presets.is_empty() {
            return Err(PluginError::new("icon.no_presets"));
        }
        if !Path::new(&args.output_dir).is_dir() {
            return Err(
                PluginError::new("icon.output_dir_missing").with("path", args.output_dir.as_str())
            );
        }
        ctx.stage("icon.load");
        let source = self.source(&args.path)?;
        let name = args
            .name
            .clone()
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| stem(&args.path));
        let dir =
            unique_dir(Path::new(&args.output_dir).join(format!("{}-icons", stem(&args.path))));
        let mut presets = args.presets.clone();
        presets.sort();
        presets.dedup();

        ctx.stage("icon.generate");
        let mut masters = Masters::new(&source.image, args.style);
        let mut on_file = |path: &Path| {
            let relative = path.strip_prefix(&dir).unwrap_or(path);
            ctx.log(
                LogLevel::Debug,
                "icon.file",
                json!({ "path": relative.to_string_lossy() }),
            );
        };
        let mut writer = Writer {
            written: Vec::new(),
            on_file: &mut on_file,
        };
        for (i, preset) in presets.iter().enumerate() {
            if ctx.is_cancelled() {
                return Err(cancelled());
            }
            let before = writer.written.len();
            presets::write(*preset, &dir, &name, &mut masters, &mut writer)?;
            let preset_name = serde_json::to_value(preset).unwrap();
            ctx.log(
                LogLevel::Info,
                "icon.preset_done",
                json!({ "preset": preset_name, "count": writer.written.len() - before }),
            );
            ctx.progress(i as u64 + 1, presets.len() as u64);
        }
        let files = writer.written.len();
        ctx.log(
            LogLevel::Info,
            "icon.done",
            json!({ "count": files, "path": dir.to_string_lossy() }),
        );
        Ok(json!({
            "dir": dir.to_string_lossy(),
            "files": files,
            "snippet": presets.contains(&Preset::Web).then_some(presets::WEB_SNIPPET),
        }))
    }
}

impl Default for IconGenerator {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
            last: Mutex::new(None),
        }
    }
}

impl ToolPlugin for IconGenerator {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "inspect" => self.inspect(parse_args(args)?),
            "render" => self.render(parse_args(args)?),
            other => Err(unknown_function(other)),
        }
    }

    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        match function {
            "generate" => self.generate(parse_args(args)?, ctx),
            _ => self.call(function, args),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
