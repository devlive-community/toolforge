//! 应用图标生成插件后端：由一张图片（PNG / JPEG / WebP / SVG 等）生成 Web、iOS、Android、
//! macOS 与 Windows 的整套图标，支持留白、背景色与圆角；也能套用文件夹、磁盘、光盘等模板，
//! 导出图标或直接设置到文件夹上。

mod apply;
mod compose;
mod presets;
mod source;
mod templates;

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
/// 模板缩略图边长与嵌入的图片边长
const THUMB: u32 = 160;
const THUMB_ART: u32 = 320;
/// 没有选择图片时用来展示模板的示例图
const SAMPLE: &[u8] = include_bytes!("sample.svg");

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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TemplatesArgs {
    #[serde(default)]
    path: Option<String>,
    #[serde(default = "one")]
    zoom: f32,
}

fn one() -> f32 {
    1.0
}

fn default_size() -> u32 {
    512
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TemplateArgs {
    #[serde(default)]
    path: Option<String>,
    template: String,
    #[serde(default = "one")]
    zoom: f32,
    #[serde(default = "default_size")]
    size: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ExportFormat {
    Png,
    Icns,
    Ico,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportArgs {
    #[serde(default)]
    path: Option<String>,
    template: String,
    #[serde(default = "one")]
    zoom: f32,
    formats: Vec<ExportFormat>,
    output_dir: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApplyArgs {
    #[serde(default)]
    path: Option<String>,
    template: String,
    #[serde(default = "one")]
    zoom: f32,
    targets: Vec<String>,
}

#[derive(Deserialize)]
struct ClearArgs {
    targets: Vec<String>,
}

type Key = (String, Option<SystemTime>);

/// 嵌入模板的图片（PNG data URI）及其尺寸
struct ArtData {
    href: String,
    width: f32,
    height: f32,
}

impl ArtData {
    fn new(image: &image::RgbaImage, longest: u32) -> Self {
        let (w, h) = image.dimensions();
        let scale = (longest as f32 / w.max(h) as f32).min(1.0);
        let resized = if scale < 1.0 {
            image::imageops::resize(
                image,
                ((w as f32 * scale).round() as u32).max(1),
                ((h as f32 * scale).round() as u32).max(1),
                image::imageops::FilterType::Lanczos3,
            )
        } else {
            image.clone()
        };
        Self {
            href: data_uri(&resized),
            width: resized.width() as f32,
            height: resized.height() as f32,
        }
    }

    fn art(&self) -> templates::Art<'_> {
        templates::Art {
            href: &self.href,
            width: self.width,
            height: self.height,
        }
    }
}

/// 不覆盖已有文件：name.ext → name (1).ext
fn unique_file(path: PathBuf) -> PathBuf {
    if !path.exists() {
        return path;
    }
    let stem = path
        .file_stem()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_default();
    (1..)
        .map(|n| path.with_file_name(format!("{stem} ({n}).{ext}")))
        .find(|p| !p.exists())
        .unwrap_or(path)
}

pub struct IconGenerator {
    manifest: Manifest,
    /// 最近读取的源图（SVG 渲染需要加载系统字体，较慢）
    last: Mutex<Option<(Key, Arc<Source>)>>,
    /// 最近一次模板缩略图用的图片（编码 PNG 也要时间）
    thumb_art: Mutex<Option<(Option<Key>, Arc<ArtData>)>>,
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

    /// 模板用的图片：选了图片用图片，否则用示例图
    fn template_source(&self, path: Option<&str>) -> PluginResult<(Option<Key>, Arc<Source>)> {
        match path.filter(|p| !p.is_empty()) {
            Some(path) => {
                let source = self.source(path)?;
                let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok();
                Ok((Some((path.to_owned(), modified)), source))
            }
            None => {
                static SAMPLE_SOURCE: std::sync::OnceLock<Arc<Source>> = std::sync::OnceLock::new();
                let source = SAMPLE_SOURCE.get_or_init(|| {
                    Arc::new(source::render_svg(SAMPLE, MASTER).expect("sample svg"))
                });
                Ok((None, source.clone()))
            }
        }
    }

    fn thumb_art(&self, path: Option<&str>) -> PluginResult<Arc<ArtData>> {
        let (key, source) = self.template_source(path)?;
        let mut cached = self.thumb_art.lock().unwrap();
        if let Some((k, art)) = cached.as_ref()
            && *k == key
        {
            return Ok(art.clone());
        }
        let art = Arc::new(ArtData::new(&source.image, THUMB_ART));
        *cached = Some((key, art.clone()));
        Ok(art)
    }

    fn full_art(&self, path: Option<&str>) -> PluginResult<ArtData> {
        let (_, source) = self.template_source(path)?;
        Ok(ArtData::new(&source.image, MASTER))
    }

    fn templates(&self, args: TemplatesArgs) -> PluginResult<Value> {
        let art = self.thumb_art(args.path.as_deref())?;
        // 模板之间互不依赖，分给多个线程渲染
        let threads = std::thread::available_parallelism()
            .map_or(4, |n| n.get())
            .min(8);
        let chunk = templates::TEMPLATES.len().div_ceil(threads);
        let parts: Vec<PluginResult<Vec<Value>>> = std::thread::scope(|scope| {
            let handles: Vec<_> = templates::TEMPLATES
                .chunks(chunk)
                .map(|part| {
                    let art = &art;
                    scope.spawn(move || {
                        part.iter()
                            .map(|t| {
                                let image = templates::render(t, &art.art(), args.zoom, THUMB)?;
                                Ok(json!({ "id": t.id, "group": t.group, "image": data_uri(&image) }))
                            })
                            .collect()
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|h| h.join().expect("render thread"))
                .collect()
        });
        let mut list = Vec::with_capacity(templates::TEMPLATES.len());
        for part in parts {
            list.extend(part?);
        }
        Ok(Value::Array(list))
    }

    fn render_template(&self, args: TemplateArgs) -> PluginResult<Value> {
        let template = templates::find(&args.template)?;
        let size = args.size.clamp(16, MASTER);
        // 大图预览用全尺寸图片，小图用缩略图即可
        let image = if size > THUMB_ART {
            templates::render(
                template,
                &self.full_art(args.path.as_deref())?.art(),
                args.zoom,
                size,
            )?
        } else {
            templates::render(
                template,
                &self.thumb_art(args.path.as_deref())?.art(),
                args.zoom,
                size,
            )?
        };
        Ok(data_uri(&image).into())
    }

    fn template_master(
        &self,
        path: Option<&str>,
        id: &str,
        zoom: f32,
    ) -> PluginResult<(image::RgbaImage, &'static templates::Template)> {
        let template = templates::find(id)?;
        let art = self.full_art(path)?;
        Ok((
            templates::render(template, &art.art(), zoom, MASTER)?,
            template,
        ))
    }

    fn export_template(&self, args: ExportArgs, ctx: &dyn TaskContext) -> PluginResult<Value> {
        if args.formats.is_empty() {
            return Err(PluginError::new("icon.no_formats"));
        }
        if !Path::new(&args.output_dir).is_dir() {
            return Err(
                PluginError::new("icon.output_dir_missing").with("path", args.output_dir.as_str())
            );
        }
        ctx.stage("icon.render");
        let (master, template) =
            self.template_master(args.path.as_deref(), &args.template, args.zoom)?;
        let base = format!(
            "{}-{}",
            args.path.as_deref().map_or_else(|| "icon".to_owned(), stem),
            template.id
        );
        ctx.stage("icon.generate");
        let mut files = Vec::new();
        for (i, format) in args.formats.iter().enumerate() {
            if ctx.is_cancelled() {
                return Err(cancelled());
            }
            let (ext, bytes) = match format {
                ExportFormat::Png => ("png", presets::png_bytes(&master)),
                ExportFormat::Icns => (
                    "icns",
                    presets::icns(&mut |size| presets::png_bytes(&compose::scaled(&master, size))),
                ),
                ExportFormat::Ico => {
                    let sizes: Vec<_> = presets::WINDOWS_SIZES
                        .iter()
                        .map(|s| compose::scaled(&master, *s))
                        .collect();
                    ("ico", presets::ico(&sizes)?)
                }
            };
            let path = unique_file(Path::new(&args.output_dir).join(format!("{base}.{ext}")));
            std::fs::write(&path, bytes).map_err(|e| {
                PluginError::new("icon.write_failed")
                    .with("path", path.to_string_lossy().as_ref())
                    .with("detail", e.to_string())
            })?;
            ctx.log(
                LogLevel::Info,
                "icon.file",
                json!({ "path": path.to_string_lossy() }),
            );
            files.push(path.to_string_lossy().into_owned());
            ctx.progress(i as u64 + 1, args.formats.len() as u64);
        }
        Ok(json!({ "files": files }))
    }

    fn apply_icon(&self, args: ApplyArgs, ctx: &dyn TaskContext) -> PluginResult<Value> {
        if args.targets.is_empty() {
            return Err(PluginError::new("icon.no_targets"));
        }
        ctx.stage("icon.render");
        let (master, _) = self.template_master(args.path.as_deref(), &args.template, args.zoom)?;
        ctx.stage("icon.apply");
        let mut applied = 0;
        for (i, target) in args.targets.iter().enumerate() {
            if ctx.is_cancelled() {
                return Err(cancelled());
            }
            match apply::set(Path::new(target), &master) {
                Ok(()) => {
                    applied += 1;
                    ctx.log(LogLevel::Info, "icon.applied", json!({ "path": target }));
                }
                Err(e) => ctx.log(LogLevel::Error, &e.code, json!(e.params)),
            }
            ctx.progress(i as u64 + 1, args.targets.len() as u64);
        }
        Ok(json!({ "applied": applied, "total": args.targets.len() }))
    }

    fn clear_icon(&self, args: ClearArgs) -> PluginResult<Value> {
        for target in &args.targets {
            apply::clear(Path::new(target))?;
        }
        Ok(json!({ "cleared": args.targets.len() }))
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
            thumb_art: Mutex::new(None),
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
            "templates" => self.templates(parse_args(args)?),
            "renderTemplate" => self.render_template(parse_args(args)?),
            "clearIcon" => self.clear_icon(parse_args(args)?),
            // 只有 macOS 能给普通文件设置图标，其他系统只支持文件夹
            "applySupport" => Ok(json!({ "files": cfg!(target_os = "macos") })),
            other => Err(unknown_function(other)),
        }
    }

    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        match function {
            "generate" => self.generate(parse_args(args)?, ctx),
            "exportTemplate" => self.export_template(parse_args(args)?, ctx),
            "applyIcon" => self.apply_icon(parse_args(args)?, ctx),
            _ => self.call(function, args),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
