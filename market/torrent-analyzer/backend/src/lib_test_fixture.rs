pub fn build_torrent() -> Vec<u8> {
    let mut out = String::from("d");
    out.push_str("8:announce");
    out.push_str("29:http://tracker.example/announce");
    out.push_str("4:infod");
    out.push_str("6:lengthi11e");
    out.push_str("4:name10:hello.txt");
    out.push_str("12:piece lengthi16384e");
    out.push_str("6:pieces20:01234567890123456789");
    out.push('e');
    out.push('e');
    out.into_bytes()
}
