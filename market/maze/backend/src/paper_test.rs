use super::*;
use crate::tests::{Context, spec};

#[test]
fn question_and_answer_batches_have_matching_distinct_ids_and_page_boundaries() {
    let ctx = Context::default();
    let s = spec("print");
    let options = Options {
        count: 5,
        per_page: 4,
        sheet: "puzzle".into(),
    };
    let q = plan(&s, &options, &ctx).unwrap();
    let a = plan(
        &s,
        &Options {
            sheet: "answer".into(),
            ..options
        },
        &ctx,
    )
    .unwrap();
    assert_eq!(q.ids, a.ids);
    assert_eq!(q.ids[0], s.id());
    assert_eq!(
        q.ids
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        5
    );
    assert_eq!(q.pages(), 2);
    assert_eq!(q.placements(1).unwrap().len(), 1);
    assert!(
        q.scenes
            .iter()
            .zip(a.scenes.iter())
            .all(|(q, a)| a.lines.len() > q.lines.len())
    );
    assert!(!q.answer);
    assert!(a.answer);
    assert!(q.preview(2).is_err());
}
#[test]
fn vector_pdf_and_png_keep_shared_wall_geometry_and_black_white_output() {
    let ctx = Context::default();
    let maze = generate(spec("render"), &ctx).unwrap();
    let scene = drawing::scene(&maze, false, &ctx).unwrap();
    let svg = drawing::svg(&scene);
    assert!(svg.contains("M1 2L2 2"));
    let png = png(&scene, &ctx).unwrap();
    let image = image::load_from_memory(&png).unwrap().to_rgb8();
    let scale = 1600.0 / scene.width.max(scene.height);
    assert_eq!(
        image
            .get_pixel((1.5 * scale) as u32, (2.0 * scale) as u32)
            .0,
        [0, 0, 0]
    );
    assert!(image.width() <= 1600 && image.height() <= 1600);
    assert!(
        image
            .pixels()
            .all(|p| p.0 == [255, 255, 255] || p.0 == [0, 0, 0])
    );
    let p = plan(
        &maze.spec,
        &Options {
            count: 1,
            per_page: 1,
            sheet: "puzzle".into(),
        },
        &ctx,
    )
    .unwrap();
    let count = p.scenes[0].lines.len();
    let pdf = pdf(&[p], &ctx).unwrap();
    let doc = Document::load_mem(&pdf).unwrap();
    let page = *doc.get_pages().values().next().unwrap();
    let data = doc.get_page_content(page);
    let content = lopdf::content::Content::decode(&data).unwrap();
    assert_eq!(
        content
            .operations
            .iter()
            .filter(|op| op.operator == "S")
            .count(),
        count
    );
}
#[test]
fn batch_and_page_budgets_are_explicit_and_placements_fit_a4() {
    let ctx = Context::default();
    for (count, per_page) in [(0, 1), (13, 1), (1, 3)] {
        assert!(
            plan(
                &spec("x"),
                &Options {
                    count,
                    per_page,
                    sheet: "puzzle".into()
                },
                &ctx
            )
            .is_err()
        );
    }
    for per_page in [1, 2, 4] {
        let p = plan(
            &spec("x"),
            &Options {
                count: 12,
                per_page,
                sheet: "puzzle".into(),
            },
            &ctx,
        )
        .unwrap();
        for page in 0..p.pages() {
            for (id, x, y, scale) in p.placements(page).unwrap() {
                let scene = &p.scenes[id];
                assert!(x >= 28.0 && y >= 28.0);
                assert!(x + scene.width * scale <= 595.276 - 27.0);
                assert!(y + scene.height * scale <= 841.89 - 27.0);
            }
        }
    }
}
