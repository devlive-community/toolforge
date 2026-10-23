use super::*;

#[test]
fn read_to_end_reads_whole_files_within_the_limit() {
    let path = std::env::temp_dir().join(format!("tf-guest-read-{}", std::process::id()));
    let data: Vec<u8> = (0..(READ_CHUNK as usize + 10))
        .map(|i| (i % 251) as u8)
        .collect();
    std::fs::write(&path, &data).unwrap();
    let path_str = path.to_string_lossy().into_owned();
    assert_eq!(
        read_to_end(&path_str, u64::MAX).unwrap(),
        data,
        "spans several chunks"
    );
    let err = read_to_end(&path_str, 1024).unwrap_err();
    assert_eq!(err.code, "fs.too_large");
    std::fs::remove_file(path).unwrap();
    assert!(read_to_end(&path_str, u64::MAX).is_err());
}
