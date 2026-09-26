use super::*;

#[test]
fn keeps_input_multiple_of_32() {
    assert_eq!(input_size(900, 360), (896, 352));
    assert_eq!(input_size(2880, 1800), (1920, 1216));
    assert_eq!(input_size(10, 10), (32, 32));
}

fn map(width: usize, height: usize, rects: &[(usize, usize, usize, usize, f32)]) -> Vec<f32> {
    let mut prob = vec![0.0; width * height];
    for &(x, y, w, h, v) in rects {
        for yy in y..y + h {
            for xx in x..x + w {
                prob[yy * width + xx] = v;
            }
        }
    }
    prob
}

#[test]
fn finds_expands_and_orders_regions() {
    // 两行：第一行有左右两段，第二行一段；另有一个低分区域与一个过小区域
    let prob = map(
        100,
        60,
        &[
            (50, 10, 30, 6, 0.9),
            (5, 11, 30, 6, 0.9),
            (5, 35, 60, 8, 0.8),
            (80, 50, 10, 5, 0.4),
            (90, 40, 2, 2, 0.9),
        ],
    );
    let found = regions(&prob, 100, 60, 200, 120);
    assert_eq!(found.len(), 3, "{found:?}");
    // 阅读顺序：第一行左、第一行右、第二行
    assert!(found[0].x < found[1].x && same_row(&found[0], &found[1]));
    assert!(found[2].y > found[0].y && !same_row(&found[0], &found[2]));
    // 映射回原图（×2）并向外扩展：30×6 的区域扩展 1.5×180/72 = 3.75
    let first = found[0];
    assert!((first.x - (5.0 - 3.75) * 2.0).abs() < 0.01, "{first:?}");
    assert!((first.w - (30.0 + 7.5) * 2.0).abs() < 0.01, "{first:?}");
}

#[test]
fn clamps_to_the_image() {
    let prob = map(40, 20, &[(0, 0, 40, 20, 0.9)]);
    let found = regions(&prob, 40, 20, 40, 20);
    assert_eq!(found.len(), 1);
    assert_eq!(
        (found[0].x, found[0].y, found[0].w, found[0].h),
        (0.0, 0.0, 40.0, 20.0)
    );
}
