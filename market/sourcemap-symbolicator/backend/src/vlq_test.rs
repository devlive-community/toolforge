use super::*;

#[test]
fn decodes_a_single_digit() {
    assert_eq!(decode(b"A", 0), Ok((0, 1)));
    assert_eq!(decode(b"C", 0), Ok((1, 1)));
    assert_eq!(decode(b"D", 0), Ok((-1, 1)));
    assert_eq!(decode(b"E", 0), Ok((2, 1)));
    assert_eq!(decode(b"I", 0), Ok((4, 1)));
    // Q = 16：raw = 16 → 正数 8
    assert_eq!(decode(b"Q", 0), Ok((8, 1)));
    // + = 62：连续位为 1，需要再跟一位组；终止形式的 31 是 "+B"
    assert_eq!(decode(b"+B", 0), Ok((31, 2)));
    // g = 32：连续位为 1 但后面没有字符
    assert_eq!(decode(b"g", 0), Err(VlqError::Truncated));
}

#[test]
fn decodes_multi_digit_values() {
    // gB：第一组 0（续位），第二组 1 → raw = 1 << 5 = 32 → 16
    assert_eq!(decode(b"gB", 0), Ok((16, 2)));
    // hB：第一组 1（续位），第二组 1 → raw = 33 → 负数 -16
    assert_eq!(decode(b"hB", 0), Ok((-16, 2)));
    // ggU：0、0、20 三组 → raw = 20 << 10 = 20480 → 10240
    assert_eq!(decode(b"ggU", 0), Ok((10240, 3)));
}

#[test]
fn rejects_unknown_characters() {
    assert_eq!(decode(b"!", 0), Err(VlqError::InvalidChar(b'!')));
    assert_eq!(decode(b"=", 0), Err(VlqError::InvalidChar(b'=')));
}

#[test]
fn rejects_overflowing_values() {
    let long = vec![b'g'; 14];
    assert_eq!(decode(&long, 0), Err(VlqError::Overflow));
}

#[test]
fn decodes_from_an_offset() {
    assert_eq!(decode(b"xC", 1), Ok((1, 1)));
}
