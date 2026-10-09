//! 图标模板：文件夹、磁盘、文档、光盘、相框等外形，把用户的图片放进去。
//! 每个模板是 1024×1024 画布上的一段 SVG，图片以 data URI 嵌入，交给 resvg 渲染。

use std::fmt::Write;

use image::RgbaImage;
use resvg::{tiny_skia, usvg};
use serde::Serialize;
use tf_plugin_api::{PluginError, PluginResult};

/// 模板分组（界面按组显示）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Group {
    Folder,
    App,
    Drive,
    Document,
    Media,
    Photo,
    Retro,
}

#[derive(Debug)]
pub struct Template {
    pub id: &'static str,
    pub group: Group,
    draw: fn(&Ctx) -> String,
}

/// 嵌入的图片
pub struct Art<'a> {
    /// data:image/png;base64,…
    pub href: &'a str,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Rect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

const fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect { x, y, w, h }
}

#[derive(Debug, Clone, Copy)]
enum Fit {
    /// 完整放进区域，四周按短边比例留白
    Contain(f32),
    /// 铺满区域（超出部分由裁切路径去掉）
    Cover,
}

struct Ctx<'a> {
    art: &'a Art<'a>,
    /// 用户的缩放（1 为默认大小）
    zoom: f32,
}

impl Ctx<'_> {
    fn place(&self, slot: Rect, fit: Fit) -> Rect {
        let (inner, cover) = match fit {
            Fit::Contain(margin) => {
                let m = slot.w.min(slot.h) * margin;
                (
                    rect(slot.x + m, slot.y + m, slot.w - 2.0 * m, slot.h - 2.0 * m),
                    false,
                )
            }
            Fit::Cover => (slot, true),
        };
        let (sx, sy) = (inner.w / self.art.width, inner.h / self.art.height);
        let scale = if cover { sx.max(sy) } else { sx.min(sy) } * self.zoom;
        let (w, h) = (self.art.width * scale, self.art.height * scale);
        rect(
            inner.x + (inner.w - w) / 2.0,
            inner.y + (inner.h - h) / 2.0,
            w,
            h,
        )
    }

    /// 图片元素；clip 为裁切路径 id，extra 为额外属性（如滤镜）
    fn image(&self, slot: Rect, fit: Fit, clip: Option<&str>, extra: &str) -> String {
        let r = self.place(slot, fit);
        let clip = clip
            .map(|id| format!(r#" clip-path="url(#{id})""#))
            .unwrap_or_default();
        let image = format!(
            r#"<image href="{}" x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" preserveAspectRatio="none" {extra}/>"#,
            self.art.href, r.x, r.y, r.w, r.h
        );
        // 裁切放在外层：图片自己的滤镜（如雕刻）先生效
        if clip.is_empty() {
            image
        } else {
            format!("<g{clip}>{image}</g>")
        }
    }
}

/// 所有模板共用的阴影与雕刻滤镜
const COMMON_DEFS: &str = r##"<filter id="sh" x="-20%" y="-20%" width="140%" height="150%" color-interpolation-filters="sRGB"><feDropShadow dx="0" dy="14" stdDeviation="16" flood-color="#000" flood-opacity="0.28"/></filter><filter id="shs" x="-20%" y="-20%" width="140%" height="150%" color-interpolation-filters="sRGB"><feDropShadow dx="0" dy="4" stdDeviation="5" flood-color="#000" flood-opacity="0.25"/></filter><linearGradient id="rainbow" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#ff7eb3"/><stop offset="0.25" stop-color="#ffd36e"/><stop offset="0.5" stop-color="#7ef0c1"/><stop offset="0.75" stop-color="#7fb2ff"/><stop offset="1" stop-color="#d78bff"/></linearGradient><linearGradient id="silver" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#f4f6f8"/><stop offset="1" stop-color="#b3bcc4"/></linearGradient><linearGradient id="paper" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#ffffff"/><stop offset="1" stop-color="#f1f2f4"/></linearGradient>"##;

/// 把图片按 alpha 压成单色并加一道下方高光，像刻在表面上
fn engrave_filter(id: &str, rgb: (f32, f32, f32), alpha: f32) -> String {
    let (r, g, b) = rgb;
    format!(
        r##"<filter id="{id}" x="-10%" y="-10%" width="120%" height="130%" color-interpolation-filters="sRGB"><feColorMatrix in="SourceAlpha" type="matrix" values="0 0 0 0 {r} 0 0 0 0 {g} 0 0 0 0 {b} 0 0 0 {alpha} 0" result="tint"/><feColorMatrix in="SourceAlpha" type="matrix" values="0 0 0 0 1 0 0 0 0 1 0 0 0 0 1 0 0 0 0.35 0" result="light"/><feOffset in="light" dy="5" result="low"/><feMerge><feMergeNode in="low"/><feMergeNode in="tint"/></feMerge></filter>"##
    )
}

// ───────────────────────────── 文件夹 ─────────────────────────────

const FOLDER_BACK: &str = "M120 236Q120 196 160 196H392Q418 196 434 216L458 246H864Q904 246 904 286V820Q904 860 864 860H160Q120 860 120 820Z";
const FOLDER_FRONT: Rect = rect(120.0, 316.0, 784.0, 544.0);

fn folder(c: &Ctx, dark: bool, engraved: bool) -> String {
    let (back, back2, front, front2, line, tint) = if dark {
        (
            "#2b98e0",
            "#1c80c8",
            "#45b5f6",
            "#2a9be6",
            "#1f86cf",
            (0.10, 0.48, 0.78),
        )
    } else {
        (
            "#5ab6ee",
            "#3b9fe2",
            "#8dd3fb",
            "#62bdf3",
            "#4aa8e8",
            (0.25, 0.60, 0.87),
        )
    };
    let f = FOLDER_FRONT;
    let art = if engraved {
        c.image(f, Fit::Contain(0.12), None, r#"filter="url(#engrave)""#)
    } else {
        c.image(f, Fit::Contain(0.12), None, "")
    };
    format!(
        r##"<defs><linearGradient id="fb" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="{back}"/><stop offset="1" stop-color="{back2}"/></linearGradient><linearGradient id="ff" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="{front}"/><stop offset="1" stop-color="{front2}"/></linearGradient>{engrave}</defs><g filter="url(#sh)"><path d="{FOLDER_BACK}" fill="url(#fb)"/><rect x="{x}" y="{y}" width="{w}" height="{h}" rx="40" fill="url(#ff)"/></g><rect x="{x}" y="{y}" width="{w}" height="6" rx="3" fill="#fff" opacity="0.45"/><rect x="{x}" y="806" width="{w}" height="4" fill="{line}" opacity="0.6"/>{art}"##,
        engrave = engrave_filter("engrave", tint, 0.85),
        x = f.x,
        y = f.y,
        w = f.w,
        h = f.h,
    )
}

// ───────────────────────────── 应用外形 ─────────────────────────────

/// 白底外形 + 图片，shape 是裁切路径的内容
fn tile(c: &Ctx, shape: &str, slot: Rect, fit: Fit) -> String {
    format!(
        r##"<defs><clipPath id="c">{shape}</clipPath></defs><g filter="url(#sh)"><g fill="url(#paper)">{shape}</g></g>{art}"##,
        art = c.image(slot, fit, Some("c"), "")
    )
}

fn big_sur(c: &Ctx) -> String {
    tile(
        c,
        r#"<rect x="100" y="100" width="824" height="824" rx="185"/>"#,
        rect(100.0, 100.0, 824.0, 824.0),
        Fit::Contain(0.1),
    )
}

fn ios(c: &Ctx) -> String {
    tile(
        c,
        r#"<rect x="72" y="72" width="880" height="880" rx="200"/>"#,
        rect(72.0, 72.0, 880.0, 880.0),
        Fit::Contain(0.06),
    )
}

fn android(c: &Ctx) -> String {
    tile(
        c,
        r#"<rect x="72" y="72" width="880" height="880" rx="96"/>"#,
        rect(72.0, 72.0, 880.0, 880.0),
        Fit::Contain(0.06),
    )
}

fn imessage(c: &Ctx) -> String {
    tile(
        c,
        r#"<rect x="72" y="192" width="880" height="640" rx="300"/>"#,
        rect(212.0, 192.0, 600.0, 640.0),
        Fit::Contain(0.08),
    )
}

fn circle_full(c: &Ctx) -> String {
    tile(
        c,
        r#"<circle cx="512" cy="512" r="440"/>"#,
        rect(112.0, 112.0, 800.0, 800.0),
        Fit::Contain(0.04),
    )
}

fn circle_border(c: &Ctx) -> String {
    format!(
        r##"<defs><clipPath id="c"><circle cx="512" cy="512" r="396"/></clipPath><linearGradient id="ring" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#ffffff"/><stop offset="0.5" stop-color="#d9dde1"/><stop offset="1" stop-color="#9aa3ab"/></linearGradient></defs><circle cx="512" cy="512" r="440" fill="url(#ring)" filter="url(#sh)"/><circle cx="512" cy="512" r="400" fill="#8f979e"/><circle cx="512" cy="512" r="396" fill="url(#paper)"/>{art}"##,
        art = c.image(
            rect(146.0, 146.0, 732.0, 732.0),
            Fit::Contain(0.06),
            Some("c"),
            ""
        )
    )
}

fn square(c: &Ctx) -> String {
    tile(
        c,
        r#"<rect x="120" y="120" width="784" height="784" rx="56"/>"#,
        rect(120.0, 120.0, 784.0, 784.0),
        Fit::Contain(0.05),
    )
}

// ───────────────────────────── 磁盘 ─────────────────────────────

fn drive(c: &Ctx, gold: bool) -> String {
    let (top, mid, bottom) = if gold {
        ("#ffd769", "#e9a915", "#b97f06")
    } else {
        ("#f3f6f8", "#cdd5db", "#a1acb5")
    };
    format!(
        r##"<defs><linearGradient id="body" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="{top}"/><stop offset="0.6" stop-color="{mid}"/><stop offset="1" stop-color="{bottom}"/></linearGradient><linearGradient id="side" x1="0" y1="0" x2="1" y2="0"><stop offset="0" stop-color="#fff" stop-opacity="0.45"/><stop offset="0.12" stop-color="#fff" stop-opacity="0"/><stop offset="0.88" stop-color="#000" stop-opacity="0"/><stop offset="1" stop-color="#000" stop-opacity="0.18"/></linearGradient></defs><g filter="url(#sh)"><path d="M262 120H762Q800 120 806 158L866 790H158L218 158Q224 120 262 120Z" fill="url(#body)"/><path d="M262 120H762Q800 120 806 158L866 790H158L218 158Q224 120 262 120Z" fill="url(#side)"/><rect x="140" y="770" width="744" height="130" rx="65" fill="url(#silver)" stroke="#8a939b" stroke-width="5"/></g><rect x="176" y="790" width="672" height="18" rx="9" fill="#fff" opacity="0.6"/><circle cx="820" cy="836" r="14" fill="#38d84a"/><circle cx="816" cy="832" r="5" fill="#d6ffd9"/>{art}"##,
        art = c.image(
            rect(240.0, 170.0, 544.0, 560.0),
            Fit::Contain(0.06),
            None,
            ""
        )
    )
}

fn usb(c: &Ctx) -> String {
    format!(
        r##"<defs><linearGradient id="body" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#454545"/><stop offset="1" stop-color="#141414"/></linearGradient><linearGradient id="plug" x1="0" y1="0" x2="1" y2="0"><stop offset="0" stop-color="#b7bec4"/><stop offset="0.5" stop-color="#eef1f3"/><stop offset="1" stop-color="#a3abb2"/></linearGradient><clipPath id="c"><rect x="226" y="196" width="572" height="420" rx="14"/></clipPath></defs><g filter="url(#sh)"><rect x="262" y="640" width="500" height="250" rx="26" fill="url(#plug)" stroke="#7c858d" stroke-width="6"/><rect x="170" y="140" width="684" height="530" rx="70" fill="url(#body)"/></g><rect x="370" y="736" width="92" height="52" rx="6" fill="#3a3d40"/><rect x="562" y="736" width="92" height="52" rx="6" fill="#3a3d40"/><rect x="226" y="196" width="572" height="420" rx="14" fill="#eeeeee"/>{art}<rect x="190" y="152" width="644" height="10" rx="5" fill="#fff" opacity="0.15"/>"##,
        art = c.image(
            rect(226.0, 196.0, 572.0, 420.0),
            Fit::Contain(0.06),
            Some("c"),
            ""
        )
    )
}

fn sd_card(c: &Ctx) -> String {
    format!(
        r##"<defs><linearGradient id="body" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#4a4a4a"/><stop offset="1" stop-color="#232323"/></linearGradient><clipPath id="c"><rect x="252" y="236" width="520" height="624" rx="14"/></clipPath></defs><path d="M234 100H700L834 234V884Q834 924 794 924H234Q194 924 194 884V140Q194 100 234 100Z" fill="url(#body)" stroke="#111" stroke-width="6" filter="url(#sh)"/><rect x="194" y="330" width="16" height="78" fill="#e1b33c"/><rect x="252" y="236" width="520" height="624" rx="14" fill="#ebebeb"/>{art}"##,
        art = c.image(
            rect(252.0, 236.0, 520.0, 624.0),
            Fit::Contain(0.08),
            Some("c"),
            ""
        )
    )
}

// ───────────────────────────── 文档 ─────────────────────────────

const PAGE: &str = "M240 96H648L828 276V884Q828 928 784 928H240Q196 928 196 884V140Q196 96 240 96Z";
const FOLD: &str = "M648 96V236Q648 276 688 276H828Z";

fn document(c: &Ctx, full: bool) -> String {
    let art = if full {
        c.image(rect(196.0, 96.0, 632.0, 832.0), Fit::Cover, Some("c"), "")
    } else {
        c.image(
            rect(276.0, 330.0, 472.0, 472.0),
            Fit::Contain(0.04),
            None,
            "",
        )
    };
    format!(
        r##"<defs><clipPath id="c"><path d="{PAGE}"/></clipPath></defs><path d="{PAGE}" fill="url(#paper)" filter="url(#sh)"/>{art}<path d="{FOLD}" fill="#e6e7e9" filter="url(#shs)"/>"##
    )
}

fn booklet(c: &Ctx) -> String {
    let mut rings = String::new();
    for i in 0..20 {
        let y = 116 + i * 40;
        let _ = write!(
            rings,
            r##"<rect x="184" y="{y}" width="52" height="14" rx="7" fill="#3b3b3b"/><rect x="196" y="{}" width="28" height="4" rx="2" fill="#bbb"/>"##,
            y + 3
        );
    }
    format!(
        r##"<defs><clipPath id="c"><path d="{PAGE}"/></clipPath></defs><path d="{PAGE}" fill="url(#paper)" filter="url(#sh)"/>{art}<path d="{FOLD}" fill="#e6e7e9" filter="url(#shs)"/><rect x="196" y="96" width="14" height="832" fill="#2a2a2a"/>{rings}"##,
        art = c.image(rect(196.0, 96.0, 632.0, 832.0), Fit::Cover, Some("c"), "")
    )
}

// ───────────────────────────── 光盘与影音 ─────────────────────────────

fn dvd_case(c: &Ctx) -> String {
    format!(
        r##"<defs><linearGradient id="spine" x1="0" y1="0" x2="1" y2="0"><stop offset="0" stop-color="#000"/><stop offset="1" stop-color="#3a3a3a"/></linearGradient><clipPath id="c"><rect x="262" y="98" width="516" height="828"/></clipPath></defs><rect x="232" y="80" width="560" height="864" rx="14" fill="#111" filter="url(#sh)"/><rect x="232" y="80" width="28" height="864" rx="10" fill="url(#spine)"/><rect x="262" y="98" width="516" height="828" fill="#fff"/>{art}"##,
        art = c.image(
            rect(262.0, 98.0, 516.0, 828.0),
            Fit::Contain(0.06),
            Some("c"),
            ""
        )
    )
}

fn movie_case(c: &Ctx) -> String {
    format!(
        r##"<defs><linearGradient id="case" x1="0" y1="0" x2="1" y2="0"><stop offset="0" stop-color="#2d3034"/><stop offset="0.12" stop-color="#5f646a"/><stop offset="1" stop-color="#3d4146"/></linearGradient><clipPath id="c"><rect x="276" y="112" width="532" height="800" rx="4"/></clipPath></defs><rect x="200" y="92" width="628" height="840" rx="22" fill="url(#case)" filter="url(#sh)"/><rect x="214" y="380" width="30" height="270" rx="15" fill="#777c82"/><rect x="276" y="112" width="532" height="800" rx="4" fill="#fff"/>{art}<rect x="276" y="112" width="532" height="800" rx="4" fill="none" stroke="#ccc" stroke-width="3"/>"##,
        art = c.image(
            rect(276.0, 112.0, 532.0, 800.0),
            Fit::Contain(0.07),
            Some("c"),
            ""
        )
    )
}

fn bluray_case(c: &Ctx) -> String {
    format!(
        r##"<defs><linearGradient id="case" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#5bb0f5"/><stop offset="1" stop-color="#1c6cc6"/></linearGradient><clipPath id="c"><rect x="222" y="198" width="580" height="708"/></clipPath></defs><rect x="200" y="96" width="624" height="832" rx="26" fill="url(#case)" filter="url(#sh)"/><ellipse cx="512" cy="146" rx="64" ry="17" fill="#fff" opacity="0.9"/><rect x="222" y="198" width="580" height="708" fill="#fff"/>{art}"##,
        art = c.image(
            rect(222.0, 198.0, 580.0, 708.0),
            Fit::Contain(0.06),
            Some("c"),
            ""
        )
    )
}

fn cd_case(c: &Ctx) -> String {
    format!(
        r##"<defs><clipPath id="c"><rect x="176" y="160" width="742" height="704"/></clipPath><linearGradient id="gloss" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#fff" stop-opacity="0.35"/><stop offset="0.4" stop-color="#fff" stop-opacity="0.05"/><stop offset="1" stop-color="#fff" stop-opacity="0"/></linearGradient></defs><rect x="96" y="150" width="832" height="724" rx="10" fill="#dfe3e6" stroke="#9aa1a7" stroke-width="4" filter="url(#sh)"/><circle cx="560" cy="512" r="340" fill="url(#rainbow)" opacity="0.55"/>{art}<circle cx="560" cy="512" r="340" fill="url(#rainbow)" opacity="0.1" clip-path="url(#c)"/><rect x="96" y="150" width="80" height="724" fill="#1b1b1b"/><rect x="176" y="160" width="742" height="704" fill="url(#gloss)"/>"##,
        art = c.image(
            rect(176.0, 160.0, 742.0, 704.0),
            Fit::Contain(0.06),
            Some("c"),
            r#"opacity="0.92""#
        )
    )
}

fn cd_cover(c: &Ctx) -> String {
    format!(
        r##"<defs><linearGradient id="case" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#5d6268"/><stop offset="1" stop-color="#34383c"/></linearGradient><clipPath id="c"><rect x="196" y="170" width="700" height="684"/></clipPath></defs><rect x="110" y="150" width="804" height="724" rx="12" fill="url(#case)" filter="url(#sh)"/><rect x="124" y="360" width="40" height="300" rx="20" fill="#7d838a"/><rect x="196" y="170" width="700" height="684" fill="#fff"/>{art}"##,
        art = c.image(
            rect(196.0, 170.0, 700.0, 684.0),
            Fit::Contain(0.06),
            Some("c"),
            ""
        )
    )
}

fn cd_disc(c: &Ctx) -> String {
    format!(
        r##"<defs><mask id="hole"><rect width="1024" height="1024" fill="#000"/><circle cx="512" cy="512" r="440" fill="#fff"/><circle cx="512" cy="512" r="58" fill="#000"/></mask><clipPath id="c"><circle cx="512" cy="512" r="432"/></clipPath><radialGradient id="disc"><stop offset="0" stop-color="#e9ecef"/><stop offset="1" stop-color="#c3c8cd"/></radialGradient></defs><g filter="url(#sh)"><g mask="url(#hole)"><circle cx="512" cy="512" r="440" fill="url(#disc)"/>{art}<circle cx="512" cy="512" r="440" fill="url(#rainbow)" opacity="0.16"/><circle cx="512" cy="512" r="170" fill="#9aa0a6" opacity="0.85"/><circle cx="512" cy="512" r="118" fill="#70767c"/><circle cx="512" cy="512" r="440" fill="none" stroke="#a7aeb5" stroke-width="6"/></g></g>"##,
        art = c.image(rect(80.0, 80.0, 864.0, 864.0), Fit::Cover, Some("c"), "")
    )
}

fn vinyl(c: &Ctx) -> String {
    let mut grooves = String::new();
    for r in (150..=340).step_by(16) {
        let _ = write!(
            grooves,
            r##"<circle cx="664" cy="512" r="{r}" fill="none" stroke="#262626" stroke-width="2"/>"##
        );
    }
    format!(
        r##"<defs><clipPath id="c"><rect x="56" y="232" width="560" height="560"/></clipPath></defs><circle cx="664" cy="512" r="356" fill="#0d0d0d" filter="url(#sh)"/>{grooves}<circle cx="664" cy="512" r="110" fill="#2c2c2c"/><circle cx="664" cy="512" r="12" fill="#888"/><rect x="56" y="232" width="560" height="560" fill="#fff" filter="url(#sh)"/>{art}"##,
        art = c.image(rect(56.0, 232.0, 560.0, 560.0), Fit::Cover, Some("c"), "")
    )
}

fn mini_disc(c: &Ctx) -> String {
    format!(
        r##"<defs><radialGradient id="disc"><stop offset="0" stop-color="#f2f4f6"/><stop offset="1" stop-color="#b9c0c6"/></radialGradient><clipPath id="c"><rect x="150" y="230" width="400" height="560" rx="24"/></clipPath></defs><rect x="110" y="150" width="804" height="724" rx="60" fill="#a8b0b7" opacity="0.6" stroke="#cfd4d8" stroke-width="6" filter="url(#sh)"/><circle cx="572" cy="512" r="320" fill="url(#disc)"/><circle cx="572" cy="512" r="320" fill="url(#rainbow)" opacity="0.2"/><circle cx="572" cy="512" r="70" fill="#9aa1a7"/><rect x="660" y="410" width="230" height="200" rx="12" fill="#c9ced2" stroke="#9da4aa" stroke-width="4"/><rect x="150" y="230" width="400" height="560" rx="24" fill="#fff" filter="url(#shs)"/>{art}"##,
        art = c.image(
            rect(150.0, 230.0, 400.0, 560.0),
            Fit::Contain(0.08),
            Some("c"),
            ""
        )
    )
}

fn film_frame(c: &Ctx) -> String {
    let mut holes = String::new();
    for i in 0..14 {
        let x = 62 + i * 66;
        let _ = write!(
            holes,
            r##"<rect x="{x}" y="206" width="38" height="38" rx="6" fill="#707070"/><rect x="{x}" y="780" width="38" height="38" rx="6" fill="#707070"/>"##
        );
    }
    format!(
        r##"<defs><clipPath id="c"><rect x="90" y="306" width="844" height="412"/></clipPath></defs><rect x="40" y="180" width="944" height="664" fill="#1c1c1c" filter="url(#sh)"/>{holes}<rect x="72" y="288" width="880" height="448" fill="#000"/><rect x="90" y="306" width="844" height="412" fill="#fff"/>{art}"##,
        art = c.image(
            rect(90.0, 306.0, 844.0, 412.0),
            Fit::Contain(0.06),
            Some("c"),
            ""
        )
    )
}

fn floppy(c: &Ctx) -> String {
    format!(
        r##"<defs><linearGradient id="body" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#3a3a3a"/><stop offset="1" stop-color="#1e1e1e"/></linearGradient><linearGradient id="shutter" x1="0" y1="0" x2="1" y2="0"><stop offset="0" stop-color="#c3c8cc"/><stop offset="0.5" stop-color="#f3f5f6"/><stop offset="1" stop-color="#b4bac0"/></linearGradient><clipPath id="c"><rect x="212" y="440" width="600" height="474"/></clipPath></defs><path d="M150 110H830L914 194V874Q914 914 874 914H150Q110 914 110 874V150Q110 110 150 110Z" fill="url(#body)" filter="url(#sh)"/><rect x="300" y="110" width="430" height="246" rx="6" fill="url(#shutter)"/><rect x="604" y="142" width="72" height="182" rx="6" fill="#222"/><rect x="212" y="440" width="600" height="474" rx="8" fill="#fff"/>{art}<rect x="140" y="836" width="30" height="30" fill="#111" stroke="#555" stroke-width="3"/><rect x="854" y="836" width="30" height="30" fill="#111" stroke="#555" stroke-width="3"/>"##,
        art = c.image(
            rect(212.0, 440.0, 600.0, 474.0),
            Fit::Contain(0.08),
            Some("c"),
            ""
        )
    )
}

// ───────────────────────────── 相片 ─────────────────────────────

fn slide(c: &Ctx) -> String {
    tile(
        c,
        r#"<rect x="262" y="112" width="500" height="800" rx="18"/>"#,
        rect(262.0, 112.0, 500.0, 800.0),
        Fit::Contain(0.08),
    )
}

/// 白色相纸（带白边），旋转 angle 度
fn print(c: &Ctx, frame: Rect, border: f32, angle: f32, id: &str) -> String {
    let inner = rect(
        frame.x + border,
        frame.y + border,
        frame.w - 2.0 * border,
        frame.h - 2.0 * border,
    );
    format!(
        r##"<defs><clipPath id="{id}"><rect x="{ix}" y="{iy}" width="{iw}" height="{ih}"/></clipPath></defs><g transform="rotate({angle} 512 512)"><rect x="{x}" y="{y}" width="{w}" height="{h}" rx="8" fill="url(#paper)" filter="url(#sh)"/><rect x="{ix}" y="{iy}" width="{iw}" height="{ih}" fill="#fff"/>{art}</g>"##,
        x = frame.x,
        y = frame.y,
        w = frame.w,
        h = frame.h,
        ix = inner.x,
        iy = inner.y,
        iw = inner.w,
        ih = inner.h,
        art = c.image(inner, Fit::Cover, Some(id), "")
    )
}

fn tilted(c: &Ctx) -> String {
    print(c, rect(152.0, 152.0, 720.0, 720.0), 28.0, 8.0, "c")
}

fn stack(c: &Ctx) -> String {
    format!(
        r##"<g transform="rotate(-11 512 512)"><rect x="170" y="150" width="680" height="680" rx="8" fill="#6e7276" opacity="0.85" filter="url(#sh)"/></g>{front}"##,
        front = print(c, rect(176.0, 190.0, 672.0, 672.0), 26.0, 3.0, "c")
    )
}

fn poster(c: &Ctx) -> String {
    format!(
        r##"<defs><clipPath id="c"><rect x="232" y="150" width="560" height="760"/></clipPath></defs><rect x="232" y="150" width="560" height="760" fill="url(#paper)" filter="url(#sh)"/>{art}<rect x="412" y="104" width="200" height="92" fill="#e8e8e8" opacity="0.82" transform="rotate(-3 512 150)" filter="url(#shs)"/>"##,
        art = c.image(
            rect(232.0, 150.0, 560.0, 760.0),
            Fit::Contain(0.08),
            Some("c"),
            ""
        )
    )
}

fn polaroid(c: &Ctx) -> String {
    format!(
        r##"<defs><clipPath id="c"><rect x="236" y="160" width="552" height="560"/></clipPath></defs><rect x="196" y="120" width="632" height="784" rx="22" fill="url(#paper)" filter="url(#sh)"/><rect x="236" y="160" width="552" height="560" fill="#c2c2c2"/>{art}"##,
        art = c.image(
            rect(236.0, 160.0, 552.0, 560.0),
            Fit::Contain(0.03),
            Some("c"),
            ""
        )
    )
}

fn stamp(c: &Ctx) -> String {
    let (x0, y0, w, h) = (212, 152, 600, 720);
    let mut holes = String::new();
    for i in 0..=12 {
        let x = x0 + i * w / 12;
        let _ = write!(
            holes,
            r##"<circle cx="{x}" cy="{y0}" r="18" fill="#000"/><circle cx="{x}" cy="{}" r="18" fill="#000"/>"##,
            y0 + h
        );
    }
    for i in 0..=15 {
        let y = y0 + i * h / 15;
        let _ = write!(
            holes,
            r##"<circle cx="{x0}" cy="{y}" r="18" fill="#000"/><circle cx="{}" cy="{y}" r="18" fill="#000"/>"##,
            x0 + w
        );
    }
    let mut waves = String::new();
    for i in 0..4 {
        let y = 690 + i * 34;
        let _ = write!(
            waves,
            r##"<path d="M560 {y}q40 -22 80 0t80 0t80 0t80 0" fill="none" stroke="#7d848a" stroke-width="7" opacity="0.55"/>"##
        );
    }
    format!(
        r##"<defs><mask id="perf"><rect x="{x0}" y="{y0}" width="{w}" height="{h}" fill="#fff"/>{holes}</mask><clipPath id="c"><rect x="262" y="202" width="500" height="620"/></clipPath></defs><g transform="rotate(-14 512 512)"><g filter="url(#sh)"><g mask="url(#perf)"><rect x="{x0}" y="{y0}" width="{w}" height="{h}" fill="#fff"/></g></g>{art}</g><circle cx="740" cy="660" r="120" fill="none" stroke="#7d848a" stroke-width="8" opacity="0.55"/><circle cx="740" cy="660" r="92" fill="none" stroke="#7d848a" stroke-width="5" opacity="0.45"/>{waves}"##,
        art = c.image(rect(262.0, 202.0, 500.0, 620.0), Fit::Cover, Some("c"), "")
    )
}

// ───────────────────────────── 复古 ─────────────────────────────

fn cartridge(c: &Ctx) -> String {
    let mut ridges = String::new();
    for i in 0..18 {
        let _ = write!(
            ridges,
            r##"<rect x="226" y="{}" width="246" height="13" rx="5" fill="#545454"/>"##,
            166 + i * 36
        );
    }
    format!(
        r##"<defs><linearGradient id="body" x1="0" y1="0" x2="1" y2="0"><stop offset="0" stop-color="#5f5f5f"/><stop offset="0.5" stop-color="#777"/><stop offset="1" stop-color="#5a5a5a"/></linearGradient><clipPath id="c"><rect x="508" y="166" width="286" height="640" rx="6"/></clipPath></defs><rect x="190" y="110" width="644" height="804" rx="18" fill="url(#body)" filter="url(#sh)"/>{ridges}<rect x="190" y="854" width="644" height="60" rx="12" fill="#4f4f4f"/><rect x="508" y="166" width="286" height="640" rx="6" fill="#e6e6e6"/>{art}"##,
        art = c.image(
            rect(508.0, 166.0, 286.0, 640.0),
            Fit::Contain(0.08),
            Some("c"),
            ""
        )
    )
}

fn cartridge2(c: &Ctx) -> String {
    format!(
        r##"<defs><linearGradient id="body" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#cfcfcf"/><stop offset="1" stop-color="#a4a4a4"/></linearGradient><clipPath id="c"><rect x="292" y="354" width="440" height="384"/></clipPath></defs><path d="M200 110H760L824 174V874Q824 914 784 914H240Q200 914 200 874Z" fill="url(#body)" filter="url(#sh)"/><rect x="262" y="150" width="500" height="124" rx="62" fill="#a9a9a9" stroke="#8f8f8f" stroke-width="5"/><rect x="268" y="330" width="488" height="432" rx="16" fill="#6c6c6c"/><rect x="292" y="354" width="440" height="384" fill="#fff"/>{art}<path d="M470 800H554L512 846Z" fill="#6c6c6c"/>"##,
        art = c.image(
            rect(292.0, 354.0, 440.0, 384.0),
            Fit::Contain(0.06),
            Some("c"),
            ""
        )
    )
}

fn vcard(c: &Ctx) -> String {
    format!(
        r##"<defs><clipPath id="c"><rect x="168" y="300" width="324" height="324" rx="10"/></clipPath></defs><g filter="url(#sh)"><rect x="96" y="210" width="832" height="560" rx="40" fill="url(#paper)"/><rect x="288" y="740" width="74" height="96" rx="12" fill="#151515"/><rect x="662" y="740" width="74" height="96" rx="12" fill="#151515"/></g><rect x="168" y="300" width="324" height="324" rx="10" fill="#dbe6f1"/>{art}<g transform="translate(560 316) scale(12.5)" fill="none" stroke="#8a9096" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="4"/><path d="M16 8v5a3 3 0 0 0 6 0v-1a10 10 0 1 0-4 8"/></g>"##,
        art = c.image(
            rect(168.0, 300.0, 324.0, 324.0),
            Fit::Contain(0.04),
            Some("c"),
            ""
        )
    )
}

pub const TEMPLATES: &[Template] = &[
    Template {
        id: "folder",
        group: Group::Folder,
        draw: |c| folder(c, false, false),
    },
    Template {
        id: "folderEngraved",
        group: Group::Folder,
        draw: |c| folder(c, false, true),
    },
    Template {
        id: "folderDark",
        group: Group::Folder,
        draw: |c| folder(c, true, false),
    },
    Template {
        id: "folderDarkEngraved",
        group: Group::Folder,
        draw: |c| folder(c, true, true),
    },
    Template {
        id: "bigSur",
        group: Group::App,
        draw: big_sur,
    },
    Template {
        id: "ios",
        group: Group::App,
        draw: ios,
    },
    Template {
        id: "android",
        group: Group::App,
        draw: android,
    },
    Template {
        id: "imessage",
        group: Group::App,
        draw: imessage,
    },
    Template {
        id: "circle",
        group: Group::App,
        draw: circle_full,
    },
    Template {
        id: "circleBorder",
        group: Group::App,
        draw: circle_border,
    },
    Template {
        id: "square",
        group: Group::App,
        draw: square,
    },
    Template {
        id: "driveGold",
        group: Group::Drive,
        draw: |c| drive(c, true),
    },
    Template {
        id: "driveSilver",
        group: Group::Drive,
        draw: |c| drive(c, false),
    },
    Template {
        id: "usb",
        group: Group::Drive,
        draw: usb,
    },
    Template {
        id: "sdCard",
        group: Group::Drive,
        draw: sd_card,
    },
    Template {
        id: "documentFull",
        group: Group::Document,
        draw: |c| document(c, true),
    },
    Template {
        id: "document",
        group: Group::Document,
        draw: |c| document(c, false),
    },
    Template {
        id: "booklet",
        group: Group::Document,
        draw: booklet,
    },
    Template {
        id: "dvdCase",
        group: Group::Media,
        draw: dvd_case,
    },
    Template {
        id: "movieCase",
        group: Group::Media,
        draw: movie_case,
    },
    Template {
        id: "blurayCase",
        group: Group::Media,
        draw: bluray_case,
    },
    Template {
        id: "cdCase",
        group: Group::Media,
        draw: cd_case,
    },
    Template {
        id: "cdCover",
        group: Group::Media,
        draw: cd_cover,
    },
    Template {
        id: "cdDisc",
        group: Group::Media,
        draw: cd_disc,
    },
    Template {
        id: "vinyl",
        group: Group::Media,
        draw: vinyl,
    },
    Template {
        id: "miniDisc",
        group: Group::Media,
        draw: mini_disc,
    },
    Template {
        id: "filmFrame",
        group: Group::Media,
        draw: film_frame,
    },
    Template {
        id: "floppy",
        group: Group::Media,
        draw: floppy,
    },
    Template {
        id: "slide",
        group: Group::Photo,
        draw: slide,
    },
    Template {
        id: "tilted",
        group: Group::Photo,
        draw: tilted,
    },
    Template {
        id: "stack",
        group: Group::Photo,
        draw: stack,
    },
    Template {
        id: "poster",
        group: Group::Photo,
        draw: poster,
    },
    Template {
        id: "polaroid",
        group: Group::Photo,
        draw: polaroid,
    },
    Template {
        id: "stamp",
        group: Group::Photo,
        draw: stamp,
    },
    Template {
        id: "cartridge",
        group: Group::Retro,
        draw: cartridge,
    },
    Template {
        id: "cartridge2",
        group: Group::Retro,
        draw: cartridge2,
    },
    Template {
        id: "vcard",
        group: Group::Retro,
        draw: vcard,
    },
];

pub fn find(id: &str) -> PluginResult<&'static Template> {
    TEMPLATES
        .iter()
        .find(|t| t.id == id)
        .ok_or_else(|| PluginError::new("icon.unknown_template").with("id", id))
}

pub const MIN_ZOOM: f32 = 0.5;
pub const MAX_ZOOM: f32 = 1.5;

/// 模板的完整 SVG
pub fn svg(template: &Template, art: &Art, zoom: f32) -> String {
    let body = (template.draw)(&Ctx {
        art,
        zoom: zoom.clamp(MIN_ZOOM, MAX_ZOOM),
    });
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024" width="1024" height="1024"><defs>{COMMON_DEFS}</defs>{body}</svg>"#
    )
}

/// 渲染为边长 size 的图片
pub fn render(template: &Template, art: &Art, zoom: f32, size: u32) -> PluginResult<RgbaImage> {
    let failed = |detail: String| PluginError::new("icon.render_failed").with("detail", detail);
    // 模板里没有文字，不需要加载字体
    let tree = usvg::Tree::from_str(&svg(template, art, zoom), &usvg::Options::default())
        .map_err(|e| failed(e.to_string()))?;
    let mut pixmap = tiny_skia::Pixmap::new(size, size).ok_or_else(|| failed("empty".into()))?;
    let scale = size as f32 / 1024.0;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    let mut image = RgbaImage::new(size, size);
    for (pixel, out) in pixmap.pixels().iter().zip(image.pixels_mut()) {
        let c = pixel.demultiply();
        *out = image::Rgba([c.red(), c.green(), c.blue(), c.alpha()]);
    }
    Ok(image)
}

#[cfg(test)]
#[path = "templates_test.rs"]
mod tests;
