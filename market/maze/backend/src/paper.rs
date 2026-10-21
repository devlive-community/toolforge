use crate::{
    drawing::{self, Scene},
    maze::*,
};
use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use lopdf::{Dictionary, Document, Object, Stream};
use serde::Deserialize;
use std::io::Cursor;
use std::time::Instant;
use tf_plugin_api::{PluginResult, TaskContext};

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Options {
    pub count: usize,
    pub per_page: usize,
    pub sheet: String,
}
pub struct Plan {
    pub scenes: Vec<Scene>,
    pub ids: Vec<String>,
    pub per_page: usize,
    pub answer: bool,
}
impl Plan {
    pub fn pages(&self) -> usize {
        self.scenes.len().div_ceil(self.per_page)
    }
    pub fn placements(&self, page: usize) -> PluginResult<Vec<(usize, f32, f32, f32)>> {
        if page >= self.pages() {
            return Err(error("page"));
        }
        let columns = if self.per_page == 4 { 2 } else { 1 };
        let rows = if self.per_page == 1 { 1 } else { 2 };
        let w = (595.276 - 56.0) / columns as f32;
        let h = (841.89 - 56.0) / rows as f32;
        let mut out = Vec::new();
        for id in page * self.per_page..((page + 1) * self.per_page).min(self.scenes.len()) {
            let slot = id % self.per_page;
            let scene = &self.scenes[id];
            let scale = ((w - 16.0) / scene.width).min((h - 16.0) / scene.height);
            let x = 28.0 + (slot % columns) as f32 * w + (w - scene.width * scale) / 2.0;
            let y = 28.0 + (slot / columns) as f32 * h + (h - scene.height * scale) / 2.0;
            out.push((id, x, y, scale));
        }
        Ok(out)
    }
    pub fn preview(&self, page: usize) -> PluginResult<String> {
        let mut output="<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 595.276 841.89\" width=\"595\" height=\"842\"><rect width=\"100%\" height=\"100%\" fill=\"white\"/>".to_owned();
        for (id, x, y, scale) in self.placements(page)? {
            output.push_str(&format!("<g transform=\"translate({x} {y}) scale({scale})\" stroke=\"black\" stroke-linecap=\"square\">"));
            for (line, width) in &self.scenes[id].lines {
                output.push_str(&format!(
                    "<path d=\"M{} {}L{} {}\" stroke-width=\"{}\"/>",
                    line[0], line[1], line[2], line[3], width
                ));
            }
            output.push_str("</g>");
        }
        output.push_str("</svg>");
        Ok(output)
    }
}
pub fn plan(spec: &Spec, options: &Options, ctx: &dyn TaskContext) -> PluginResult<Plan> {
    if !(1..=12).contains(&options.count)
        || ![1, 2, 4].contains(&options.per_page)
        || !["puzzle", "answer"].contains(&options.sheet.as_str())
    {
        return Err(error("paper"));
    }
    let started = Instant::now();
    let mut scenes = Vec::new();
    let mut ids = Vec::new();
    for index in 0..options.count {
        check(ctx)?;
        if started.elapsed().as_secs_f64() > 10.0 {
            return Err(error("time"));
        }
        let mut next = spec.clone();
        if index > 0 {
            next.seed = format!("B-{}-{}", spec.id(), index + 1);
        }
        let maze = generate(next, ctx)?;
        ids.push(maze.spec.id());
        scenes.push(drawing::scene(&maze, options.sheet == "answer", ctx)?);
        ctx.progress(index as u64 + 1, options.count as u64);
    }
    Ok(Plan {
        scenes,
        ids,
        per_page: options.per_page,
        answer: options.sheet == "answer",
    })
}
pub fn png(scene: &Scene, ctx: &dyn TaskContext) -> PluginResult<Vec<u8>> {
    let scale = 1600.0 / scene.width.max(scene.height);
    let width = (scene.width * scale).ceil() as u32;
    let height = (scene.height * scale).ceil() as u32;
    let mut image = RgbImage::from_pixel(width, height, Rgb([255, 255, 255]));
    for (line, stroke) in &scene.lines {
        check(ctx)?;
        let radius = stroke * scale / 2.0;
        let x0 = (line[0].min(line[2]) * scale - radius).floor().max(0.0) as u32;
        let x1 = (line[0].max(line[2]) * scale + radius)
            .ceil()
            .min(width as f32) as u32;
        let y0 = (line[1].min(line[3]) * scale - radius).floor().max(0.0) as u32;
        let y1 = (line[1].max(line[3]) * scale + radius)
            .ceil()
            .min(height as f32) as u32;
        for y in y0..y1 {
            for x in x0..x1 {
                image.put_pixel(x, y, Rgb([0, 0, 0]));
            }
        }
    }
    let mut data = Cursor::new(Vec::new());
    DynamicImage::ImageRgb8(image)
        .write_to(&mut data, ImageFormat::Png)
        .map_err(|_| error("export"))?;
    check(ctx)?;
    Ok(data.into_inner())
}
pub fn pdf(plans: &[Plan], ctx: &dyn TaskContext) -> PluginResult<Vec<u8>> {
    let pages_count: usize = plans.iter().map(Plan::pages).sum();
    if pages_count == 0 || pages_count > 24 {
        return Err(error("paper"));
    }
    let mut document = Document::with_version("1.7");
    let pages_id = document.new_object_id();
    let mut kids = Vec::new();
    for plan in plans {
        for page in 0..plan.pages() {
            check(ctx)?;
            let mut content = "0 G 2 J\n".to_owned();
            for (id, x, y, scale) in plan.placements(page)? {
                for (line, width) in &plan.scenes[id].lines {
                    check(ctx)?;
                    content.push_str(&format!(
                        "{} w {} {} m {} {} l S\n",
                        width * scale,
                        x + line[0] * scale,
                        841.89 - y - line[1] * scale,
                        x + line[2] * scale,
                        841.89 - y - line[3] * scale
                    ));
                }
            }
            let stream = document.add_object(Stream::new(Dictionary::new(), content.into_bytes()));
            let mut page_dict = Dictionary::new();
            page_dict.set("Type", Object::Name(b"Page".to_vec()));
            page_dict.set("Parent", pages_id);
            page_dict.set(
                "MediaBox",
                vec![
                    Object::Integer(0),
                    Object::Integer(0),
                    Object::Real(595.276),
                    Object::Real(841.89),
                ],
            );
            page_dict.set("Resources", Dictionary::new());
            page_dict.set("Contents", stream);
            kids.push(Object::Reference(document.add_object(page_dict)));
        }
    }
    let mut pages = Dictionary::new();
    pages.set("Type", Object::Name(b"Pages".to_vec()));
    pages.set("Kids", Object::Array(kids));
    pages.set("Count", pages_count as i64);
    document.objects.insert(pages_id, Object::Dictionary(pages));
    let mut catalog = Dictionary::new();
    catalog.set("Type", Object::Name(b"Catalog".to_vec()));
    catalog.set("Pages", pages_id);
    let root = document.add_object(catalog);
    document.trailer.set("Root", root);
    document.compress();
    let mut data = Vec::new();
    document.save_to(&mut data).map_err(|_| error("export"))?;
    check(ctx)?;
    Ok(data)
}

#[cfg(test)]
#[path = "paper_test.rs"]
mod tests;
