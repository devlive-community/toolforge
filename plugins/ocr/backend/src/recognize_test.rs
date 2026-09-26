use super::*;

#[test]
fn loads_the_dictionary() {
    let keys = characters();
    assert_eq!(keys.len(), 6625);
    assert_eq!(keys[0], "");
    assert_eq!(keys[keys.len() - 1], " ");
}

#[test]
fn picks_width_buckets() {
    assert_eq!(bucket(10), 160);
    assert_eq!(bucket(321), 480);
    assert_eq!(bucket(9999), 2560);
}

#[test]
fn pads_lines_to_the_bucket() {
    let line = RgbImage::from_pixel(100, 24, image::Rgb([255, 255, 255]));
    let (tensor, bucket) = input(&line);
    assert_eq!(bucket, 320);
    assert_eq!(tensor.shape(), &[1, 3, 48, 320]);
    let view = tensor.to_plain_array_view::<f32>().unwrap();
    assert_eq!(view[[0, 0, 10, 10]], 1.0, "white becomes 1");
    assert_eq!(view[[0, 0, 10, 300]], 0.0, "padding stays 0");
}

#[test]
fn decodes_ctc_output() {
    let keys = ["", "a", "b", "c"];
    // 每步 4 类：a a 空 a b b c
    let steps = [1usize, 1, 0, 1, 2, 2, 3];
    let mut probs = vec![0.0f32; steps.len() * 4];
    for (i, &k) in steps.iter().enumerate() {
        probs[i * 4 + k] = 0.8;
    }
    let (text, score) = decode(&probs, steps.len(), 4, &keys);
    assert_eq!(text, "aabc");
    assert!((score - 0.8).abs() < 1e-6);
    assert_eq!(
        decode(&[0.9, 0.1, 0.0, 0.0], 1, 4, &keys),
        (String::new(), 0.0)
    );
}
