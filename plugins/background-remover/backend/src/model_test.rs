use super::*;

#[test]
fn models_deserialize_from_resource_ids() {
    let model: Model = serde_json::from_str("\"isnet-general-use\"").unwrap();
    assert_eq!(model, Model::Isnet);
    assert_eq!(
        serde_json::from_str::<Model>("\"u2netp\"").unwrap(),
        Model::U2netp
    );
    assert_eq!(
        (Model::U2netp.input_size(), Model::Isnet.input_size()),
        (320, 1024)
    );
}

#[test]
fn preprocess_resizes_and_normalizes() {
    let image = RgbImage::from_pixel(64, 32, image::Rgb([255, 128, 0]));
    let tensor = preprocess(&image, Model::Isnet);
    assert_eq!(tensor.shape(), &[1, 3, 1024, 1024]);
    let view = tensor.to_plain_array_view::<f32>().unwrap();
    // IS-Net：(x / max - 0.5) / 1.0，max 为 255
    assert!((view[[0, 0, 0, 0]] - 0.5).abs() < 1e-3);
    assert!((view[[0, 2, 10, 10]] + 0.5).abs() < 1e-3);

    let tensor = preprocess(&image, Model::U2netp);
    assert_eq!(tensor.shape(), &[1, 3, 320, 320]);
}

#[test]
fn output_is_min_max_normalized() {
    let data: Vec<f32> = (0..16).map(|i| i as f32 * 0.1 - 0.3).collect();
    let tensor = Tensor::from_shape(&[1, 1, 4, 4], &data).unwrap();
    let mask = mask_from_output(&tensor, 4).unwrap();
    assert_eq!(mask.get_pixel(0, 0).0[0], 0);
    assert_eq!(mask.get_pixel(3, 3).0[0], 255);
    assert!(mask_from_output(&tensor, 8).is_err());
}

#[test]
fn invalid_model_file_is_reported() {
    let path = std::env::temp_dir().join(format!("tfp-bg-bad-{}.onnx", std::process::id()));
    std::fs::write(&path, b"not a model").unwrap();
    let err = Sessions::default().get(Model::U2netp, &path).err().unwrap();
    assert_eq!(err.code, "bg.model_invalid");
}
