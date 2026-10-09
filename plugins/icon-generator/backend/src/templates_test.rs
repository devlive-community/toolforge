use super::*;
use crate::presets::png_bytes;
use data_encoding::BASE64;

fn art_href(width: u32, height: u32) -> String {
    let image = RgbaImage::from_pixel(width, height, image::Rgba([230, 40, 40, 255]));
    format!(
        "data:image/png;base64,{}",
        BASE64.encode(&png_bytes(&image))
    )
}

fn is_red(p: &image::Rgba<u8>) -> bool {
    p[0] > 180 && p[1] < 90 && p[2] < 90 && p[3] > 200
}

#[test]
fn every_template_renders_the_art_inside() {
    let href = art_href(64, 64);
    let art = Art {
        href: &href,
        width: 64.0,
        height: 64.0,
    };
    let mut ids = std::collections::HashSet::new();
    for template in TEMPLATES {
        assert!(ids.insert(template.id), "duplicate {}", template.id);
        let image = render(template, &art, 1.0, 128).unwrap();
        let red = image.pixels().filter(|p| is_red(p)).count();
        // 雕刻模板把图片压成单色，看不到原色
        if template.id.contains("Engraved") {
            assert_eq!(red, 0, "{}", template.id);
        } else {
            assert!(
                red > 128 * 128 / 50,
                "{} shows only {red} art pixels",
                template.id
            );
        }
        // 四角留给阴影或透明，不应画满整张图
        assert!(image.get_pixel(0, 0)[3] < 255, "{}", template.id);
    }
    assert_eq!(TEMPLATES.len(), 37);
}

#[test]
fn zoom_and_fit_change_the_art_size() {
    let href = art_href(200, 100);
    let art = Art {
        href: &href,
        width: 200.0,
        height: 100.0,
    };
    let template = find("bigSur").unwrap();
    let count = |zoom| {
        render(template, &art, zoom, 128)
            .unwrap()
            .pixels()
            .filter(|p| is_red(p))
            .count()
    };
    let (small, normal, big) = (count(0.5), count(1.0), count(1.5));
    assert!(small < normal && normal < big, "{small} {normal} {big}");
    // 超出范围的缩放会被限制
    assert_eq!(count(9.0), big);
    assert_eq!(find("nope").unwrap_err().code, "icon.unknown_template");
}

#[test]
fn engraved_folders_tint_the_art_shape() {
    let href = art_href(64, 64);
    let art = Art {
        href: &href,
        width: 64.0,
        height: 64.0,
    };
    let plain = render(find("folder").unwrap(), &art, 1.0, 128).unwrap();
    let engraved = render(find("folderEngraved").unwrap(), &art, 1.0, 128).unwrap();
    let center = (64, 74);
    assert!(is_red(plain.get_pixel(center.0, center.1)));
    let p = engraved.get_pixel(center.0, center.1);
    assert!(p[2] > p[0] && p[3] == 255, "{p:?}");
}
