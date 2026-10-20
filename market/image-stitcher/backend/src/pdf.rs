//! PDF 组装（lopdf）：缩略图以 DCTDecode 原样嵌入；标签文字优先按用户字体
//! 的字形轮廓以矢量路径绘制（支持任意语言，含中文，无需嵌入字体对象），
//! 未提供字体时回退 Helvetica 标准字体（仅 ASCII/Latin-1），无法显示的字符
//! 收集进缺字清单。坐标：布局为毫米（页面左上原点），此处统一换算为点（左下原点）。

use lopdf::{Dictionary, Document, Object, Stream};
use tf_plugin_api::{PluginError, PluginResult};
use ttf_parser::OutlineBuilder;

use crate::contact::{MM_TO_PT, PlanOut};

/// 无字体时 Helvetica 的保守平均字符宽度（em），仅用于截断估算
const FALLBACK_ADVANCE: f64 = 0.6;
/// 文本颜色：黑
const INK: &str = "0 0 0 rg\n";
/// 标签行距（相对字号）
const LINE_SPACING: f64 = 1.35;
/// 单元格左右内边距（mm），与布局常量一致
const CELL_PADDING: f64 = 1.2;
/// 省略号
const ELLIPSIS: char = '…';

fn errp(code: &str, key: &str, value: impl Into<serde_json::Value>) -> PluginError {
    PluginError::new(code).with(key, value)
}

fn name(s: &str) -> Object {
    Object::Name(s.as_bytes().to_vec())
}

/// 内容流坐标格式化（两位小数，避免浮点噪声膨胀体积）
fn num(v: f64) -> String {
    let v = if v.abs() < 0.005 { 0.0 } else { v };
    format!("{v:.2}")
}

/// 用户字体：解析字形轮廓与步进宽度
pub struct FontFace<'a> {
    face: ttf_parser::Face<'a>,
    units_per_em: f64,
}

impl<'a> FontFace<'a> {
    pub fn parse(data: &'a [u8]) -> PluginResult<Self> {
        let face = ttf_parser::Face::parse(data, 0)
            .map_err(|e| errp("sheet.font_failed", "detail", format!("{e:?}")))?;
        let units_per_em = f64::from(face.units_per_em().max(1));
        Ok(Self { face, units_per_em })
    }

    fn units_per_em(&self) -> f64 {
        self.units_per_em
    }

    /// 字形索引；notdef（0）视为缺字（空格除外）
    fn glyph(&self, ch: char) -> Option<ttf_parser::GlyphId> {
        match self.face.glyph_index(ch) {
            Some(gid) if gid.0 != 0 || ch == ' ' => Some(gid),
            _ => None,
        }
    }

    /// 步进宽度（pt）；缺字用保守估算
    fn advance(&self, ch: char, size: f64) -> f64 {
        let Some(gid) = self.glyph(ch) else {
            return FALLBACK_ADVANCE * size;
        };
        f64::from(self.face.glyph_hor_advance(gid).unwrap_or(600)) / self.units_per_em() * size
    }

    /// 文本宽度（pt）
    pub fn measure(&self, text: &str, size: f64) -> f64 {
        text.chars().map(|ch| self.advance(ch, size)).sum()
    }
}

/// 轮廓收集器：跟踪当前点，把 TrueType 的二次曲线转为三次贝塞尔后写入内容流。
/// 坐标变换：pt = 原点 + 单位坐标 / upem × 字号（字形 y 向上，PDF y 也向上，无需翻转）。
struct Pen<'a> {
    scale: f64,
    ox: f64,
    oy: f64,
    cur_x: f64,
    cur_y: f64,
    out: &'a mut String,
    /// 是否有未填充的子路径
    open: bool,
}

impl<'a> Pen<'a> {
    fn new(size: f64, units_per_em: f64, ox: f64, oy: f64, out: &'a mut String) -> Self {
        Self {
            scale: size / units_per_em,
            ox,
            oy,
            cur_x: 0.0,
            cur_y: 0.0,
            out,
            open: false,
        }
    }

    fn x(&self, v: f32) -> f64 {
        self.ox + f64::from(v) * self.scale
    }

    fn y(&self, v: f32) -> f64 {
        self.oy + f64::from(v) * self.scale
    }

    fn finish(&mut self) {
        if self.open {
            self.out.push_str("f\n");
        }
    }
}

impl OutlineBuilder for Pen<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.finish();
        self.cur_x = self.x(x);
        self.cur_y = self.y(y);
        self.out
            .push_str(&format!("{} {} m\n", num(self.cur_x), num(self.cur_y)));
        self.open = true;
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.cur_x = self.x(x);
        self.cur_y = self.y(y);
        self.out
            .push_str(&format!("{} {} l\n", num(self.cur_x), num(self.cur_y)));
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        // 二次转三次：c1 = p0 + 2/3 (q - p0)，c2 = p2 + 2/3 (q - p2)
        let (p0x, p0y) = (self.cur_x, self.cur_y);
        let (qx, qy) = (self.x(x1), self.y(y1));
        let (p2x, p2y) = (self.x(x), self.y(y));
        let c1x = p0x + 2.0 / 3.0 * (qx - p0x);
        let c1y = p0y + 2.0 / 3.0 * (qy - p0y);
        let c2x = p2x + 2.0 / 3.0 * (qx - p2x);
        let c2y = p2y + 2.0 / 3.0 * (qy - p2y);
        self.out.push_str(&format!(
            "{} {} {} {} {} {} c\n",
            num(c1x),
            num(c1y),
            num(c2x),
            num(c2y),
            num(p2x),
            num(p2y)
        ));
        self.cur_x = p2x;
        self.cur_y = p2y;
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.out.push_str(&format!(
            "{} {} {} {} {} {} c\n",
            num(self.x(x1)),
            num(self.y(y1)),
            num(self.x(x2)),
            num(self.y(y2)),
            num(self.x(x)),
            num(self.y(y))
        ));
        self.cur_x = self.x(x);
        self.cur_y = self.y(y);
    }

    fn close(&mut self) {
        self.out.push_str("h\n");
    }
}

/// 把文本截断到可用宽度内（尾部省略号）
fn truncate(text: &str, max_w: f64, size: f64, face: Option<&FontFace<'_>>) -> String {
    let width = |s: &str| match face {
        Some(f) => f.measure(s, size),
        None => s.chars().count() as f64 * FALLBACK_ADVANCE * size,
    };
    if width(text) <= max_w {
        return text.to_owned();
    }
    let mut chars: Vec<char> = text.chars().collect();
    while !chars.is_empty()
        && width(&(chars.iter().collect::<String>() + &ELLIPSIS.to_string())) > max_w
    {
        chars.pop();
    }
    let mut result: String = chars.into_iter().collect();
    result.push(ELLIPSIS);
    result
}

/// WinAnsi 兼容字节：ASCII 与 Latin-1 高半区直通，其余记缺字并替换为 ?
fn winansi(text: &str, missing: &mut Vec<char>) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        let code = u32::from(ch);
        if (0x20..=0x7E).contains(&code) || (0xA0..=0xFF).contains(&code) {
            let escaped = match ch {
                '\\' | '(' | ')' => format!("\\{ch}"),
                _ => ch.to_string(),
            };
            out.push_str(&escaped);
        } else {
            missing.push(ch);
            out.push('?');
        }
    }
    out
}

/// 无字体时以 Helvetica 绘制一行（基线原点）
fn draw_helvetica_line(
    text: &str,
    size: f64,
    x: f64,
    baseline: f64,
    out: &mut String,
    missing: &mut Vec<char>,
) {
    if text.is_empty() {
        return;
    }
    let encoded = winansi(text, missing);
    out.push_str(&format!(
        "BT /F1 {} Tf {} {} Td ({}) Tj ET\n",
        num(size),
        num(x),
        num(baseline),
        encoded
    ));
}

/// 有字体时以字形轮廓绘制一行；无法显示的字符记入缺字清单
fn draw_face_line(
    face: &FontFace<'_>,
    text: &str,
    size: f64,
    x: f64,
    baseline: f64,
    out: &mut String,
    missing: &mut Vec<char>,
) {
    let mut pen_x = x;
    for ch in text.chars() {
        match face.glyph(ch) {
            Some(gid) => {
                let mut pen = Pen::new(size, face.units_per_em(), pen_x, baseline, out);
                face.face.outline_glyph(gid, &mut pen);
                pen.finish();
            }
            None => missing.push(ch),
        }
        pen_x += face.advance(ch, size);
    }
}

/// 组装多页 PDF：thumbs 与布局的格子一一对应（顺序 = 编号顺序）
pub fn build(
    layout: &PlanOut,
    thumbs: &[(u32, u32, Vec<u8>)],
    font_data: Option<&[u8]>,
) -> PluginResult<(Vec<u8>, Vec<char>)> {
    if thumbs.len() != layout.pages.iter().map(|p| p.cells.len()).sum::<usize>() {
        return Err(PluginError::new("sheet.layout_mismatch"));
    }
    let face = font_data.map(FontFace::parse).transpose()?;
    let [page_w_mm, page_h_mm] = layout.page_size;
    let page_w = page_w_mm * MM_TO_PT;
    let page_h = page_h_mm * MM_TO_PT;
    let label_size = layout.label_font_size;

    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();
    let helv_id = doc.add_object({
        let mut font = Dictionary::new();
        font.set("Type", name("Font"));
        font.set("Subtype", name("Type1"));
        font.set("BaseFont", name("Helvetica"));
        font.set("Encoding", name("WinAnsiEncoding"));
        font
    });
    let mut missing = Vec::new();
    let mut kids = Vec::new();
    let mut thumbs = thumbs.iter();
    let mut image_index = 0usize;

    for page in &layout.pages {
        let mut content = String::new();
        let mut xobjects = Dictionary::new();
        for cell in &page.cells {
            let (width, height, jpeg) = thumbs
                .next()
                .ok_or_else(|| PluginError::new("sheet.layout_mismatch"))?;
            let id = doc.add_object(Stream::new(
                {
                    let mut dict = Dictionary::new();
                    dict.set("Type", name("XObject"));
                    dict.set("Subtype", name("Image"));
                    dict.set("Width", i64::from(*width));
                    dict.set("Height", i64::from(*height));
                    dict.set("ColorSpace", name("DeviceRGB"));
                    dict.set("BitsPerComponent", Object::Integer(8));
                    dict.set("Filter", name("DCTDecode"));
                    dict
                },
                jpeg.clone(),
            ));
            image_index += 1;
            xobjects.set(format!("Im{image_index}"), id);
            let x = cell.image_x * MM_TO_PT;
            let y = page_h - (cell.image_y + cell.image_h) * MM_TO_PT;
            content.push_str(&format!(
                "q {} 0 0 {} {} {} cm /Im{image_index} Do Q\n",
                num(cell.image_w * MM_TO_PT),
                num(cell.image_h * MM_TO_PT),
                num(x),
                num(y)
            ));
        }

        // 标签：格子底部标签区逐行绘制（先图后字，文字在上层）
        content.push_str(INK);
        let line_height = label_size * LINE_SPACING * MM_TO_PT;
        for cell in &page.cells {
            let label_x = (cell.x + CELL_PADDING) * MM_TO_PT;
            let max_w = (cell.w - CELL_PADDING * 2.0) * MM_TO_PT;
            let label_top = cell.y + cell.h;
            for (row, line) in cell.lines.iter().enumerate() {
                let truncated = truncate(line, max_w, label_size, face.as_ref());
                let baseline = page_h - (label_top - (row as f64 + 0.78) * line_height) * MM_TO_PT;
                match &face {
                    Some(face) => draw_face_line(
                        face,
                        &truncated,
                        label_size,
                        label_x,
                        baseline,
                        &mut content,
                        &mut missing,
                    ),
                    None => draw_helvetica_line(
                        &truncated,
                        label_size,
                        label_x,
                        baseline,
                        &mut content,
                        &mut missing,
                    ),
                }
            }
        }

        // 页码：底部居中
        if let Some(size) = layout.page_font_size {
            let text = format!("{}/{}", page.number, layout.pages.len());
            let width = match &face {
                Some(face) => face.measure(&text, size),
                None => text.chars().count() as f64 * FALLBACK_ADVANCE * size,
            };
            let baseline = page_h - (page_h_mm - 4.0) * MM_TO_PT;
            match &face {
                Some(face) => draw_face_line(
                    face,
                    &text,
                    size,
                    (page_w - width) / 2.0,
                    baseline,
                    &mut content,
                    &mut missing,
                ),
                None => draw_helvetica_line(
                    &text,
                    size,
                    (page_w - width) / 2.0,
                    baseline,
                    &mut content,
                    &mut missing,
                ),
            }
        }

        // 标题：首页顶部居中（每页重复标题，保持翻页可读）
        if let (Some(title), Some(size)) = (&layout.title, layout.title_font_size) {
            let truncated = truncate(title, page_w_mm - 20.0, size, face.as_ref());
            let width = match &face {
                Some(face) => face.measure(&truncated, size),
                None => truncated.chars().count() as f64 * FALLBACK_ADVANCE * size,
            };
            let baseline = page_h - (4.0 + size * 0.78 * MM_TO_PT) * MM_TO_PT;
            match &face {
                Some(face) => draw_face_line(
                    face,
                    &truncated,
                    size,
                    (page_w - width) / 2.0,
                    baseline,
                    &mut content,
                    &mut missing,
                ),
                None => draw_helvetica_line(
                    &truncated,
                    size,
                    (page_w - width) / 2.0,
                    baseline,
                    &mut content,
                    &mut missing,
                ),
            }
        }

        let mut resources = Dictionary::new();
        if !xobjects.is_empty() {
            resources.set("XObject", xobjects);
        }
        let mut fonts = Dictionary::new();
        fonts.set("F1", helv_id);
        resources.set("Font", fonts);
        let content_id = doc.add_object(Stream::new(Dictionary::new(), content.into_bytes()));
        let mut page_dict = Dictionary::new();
        page_dict.set("Type", name("Page"));
        page_dict.set("Parent", pages_id);
        page_dict.set(
            "MediaBox",
            vec![
                Object::Real(0.0),
                Object::Real(0.0),
                Object::Real(page_w as f32),
                Object::Real(page_h as f32),
            ],
        );
        page_dict.set("Resources", resources);
        page_dict.set("Contents", content_id);
        kids.push(Object::Reference(doc.add_object(page_dict)));
    }

    let mut pages = Dictionary::new();
    pages.set("Type", name("Pages"));
    pages.set("Kids", Object::Array(kids));
    pages.set("Count", Object::Integer(layout.pages.len() as i64));
    doc.objects.insert(pages_id, Object::Dictionary(pages));
    let mut catalog = Dictionary::new();
    catalog.set("Type", name("Catalog"));
    catalog.set("Pages", pages_id);
    let catalog_id = doc.add_object(catalog);
    doc.trailer.set("Root", catalog_id);

    let mut bytes = Vec::new();
    doc.save_to(&mut bytes)
        .map_err(|e| errp("sheet.pdf_failed", "detail", e.to_string()))?;
    Ok((bytes, missing))
}
