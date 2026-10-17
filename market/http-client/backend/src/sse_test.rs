use super::*;

fn parse_chunks(chunks: &[&[u8]]) -> (Vec<Item>, Parser) {
    let mut parser = Parser::default();
    let mut out = Vec::new();
    for chunk in chunks {
        parser.feed(chunk, &mut out);
    }
    (out, parser)
}

fn events(items: &[Item]) -> Vec<&Event> {
    items
        .iter()
        .filter_map(|item| match item {
            Item::Event(event) => Some(event),
            Item::Comment => None,
        })
        .collect()
}

fn event(kind: Option<&str>, id: Option<&str>, data: &str) -> Event {
    Event {
        event: kind.map(str::to_owned),
        id: id.map(str::to_owned),
        data: data.to_owned(),
        retry: None,
        truncated: false,
    }
}

#[test]
fn parses_fields_and_multiline_data() {
    let text = b"event: update\nid: 7\ndata: first\ndata:second\ndata\n\ndata: plain\n\n";
    let (items, parser) = parse_chunks(&[text]);
    assert_eq!(
        events(&items),
        [
            &event(Some("update"), Some("7"), "first\nsecond\n"),
            // id 延续到之后的事件，event 每次重置
            &event(None, Some("7"), "plain"),
        ]
    );
    assert_eq!(parser.last_id(), Some("7"));
}

#[test]
fn handles_every_line_ending_and_chunk_boundary() {
    let text = "data: a\r\n\r\ndata: b\r\rdata: c\n\n";
    let expected = [
        event(None, None, "a"),
        event(None, None, "b"),
        event(None, None, "c"),
    ];
    // 在每一个字节位置切块，结果都一样（包括 CR 与 LF 被切开）
    for split in 0..=text.len() {
        let (a, b) = text.as_bytes().split_at(split);
        let (items, _) = parse_chunks(&[a, b]);
        assert_eq!(
            events(&items),
            expected.iter().collect::<Vec<_>>(),
            "split at {split}"
        );
    }
    let bytes: Vec<&[u8]> = text.as_bytes().chunks(1).collect();
    assert_eq!(events(&parse_chunks(&bytes).0).len(), 3);
}

#[test]
fn decodes_utf8_split_across_chunks() {
    let text = "data: 你好，世界 🌏\n\n".as_bytes();
    for split in 0..=text.len() {
        let (a, b) = text.split_at(split);
        let (items, _) = parse_chunks(&[a, b]);
        assert_eq!(events(&items)[0].data, "你好，世界 🌏", "split at {split}");
    }
}

#[test]
fn follows_the_specification_details() {
    let text = "\u{feff}: heartbeat\nretry: 3000\nretry: soon\nid: a\0b\nfoo: ignored\nevent:\ndata\n\n\
                id\ndata: x\n\nevent: only\n\ndata:  two spaces\n\n";
    let (items, parser) = parse_chunks(&[text.as_bytes()]);
    assert_eq!(items[0], Item::Comment);
    let list = events(&items);
    // BOM 去掉；非数字 retry 与含 NUL 的 id 被忽略；空 event 视为默认
    assert_eq!(list[0].data, "");
    assert_eq!(list[0].retry, Some(3000));
    assert_eq!(list[0].event, None);
    assert_eq!(list[0].id, None);
    // 空 id 重置 last event id；没有 data 的事件不派发
    assert_eq!(list[1].id, None);
    assert_eq!(list[1].retry, None);
    // 只去掉冒号后的一个空格
    assert_eq!(list[2].data, " two spaces");
    assert_eq!(list.len(), 3);
    assert_eq!(parser.retry(), Some(3000));
}

#[test]
fn discards_unterminated_event_at_end() {
    let (items, mut parser) = parse_chunks(&[b"data: done\n\ndata: partial\n"]);
    assert_eq!(events(&items).len(), 1);
    assert!(parser.finish());
    assert!(!Parser::default().finish());
}

#[test]
fn truncates_oversized_lines_and_data() {
    let mut parser = Parser::default();
    let mut out = Vec::new();
    let big = vec![b'x'; MAX_DATA + 10];
    parser.feed(b"data: ", &mut out);
    parser.feed(&big, &mut out);
    parser.feed(b"\n\n", &mut out);
    let list = events(&out);
    assert_eq!(list[0].data.len(), MAX_DATA);
    assert!(list[0].truncated);
    // 超长行被截断后，后续事件照常解析
    let line = vec![b'y'; MAX_LINE + 5];
    parser.feed(b"data: ", &mut out);
    parser.feed(&line, &mut out);
    parser.feed(b"\n\ndata: next\n\n", &mut out);
    let list = events(&out);
    assert!(list[1].truncated);
    assert_eq!(list[2].data, "next");
    assert!(!list[2].truncated);
}

#[test]
fn decodes_raw_chunks_incrementally() {
    let text = "流式 chunk 🌏".as_bytes();
    for split in 0..=text.len() {
        let mut decoder = Utf8Chunks::default();
        let (a, b) = text.split_at(split);
        let joined = decoder.feed(a) + &decoder.feed(b) + &decoder.finish();
        assert_eq!(joined, "流式 chunk 🌏", "split at {split}");
    }
    let mut decoder = Utf8Chunks::default();
    // 非法字节不会被一直扣留
    assert_eq!(decoder.feed(&[b'a', 0xff]), "a\u{fffd}");
    assert_eq!(decoder.feed(&[0xe4, 0xbd]), "");
    assert_eq!(decoder.finish(), "\u{fffd}");
}
