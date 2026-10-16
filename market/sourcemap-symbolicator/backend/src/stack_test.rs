use super::*;

const MAX: usize = 200;

#[test]
fn parses_chrome_frames() {
    let stack = parse(
        "TypeError: boom\n    at doWork (http://x/assets/app.min.js:1:1234)\n    at http://x/assets/app.min.js:2:9",
        MAX,
    );
    assert_eq!(stack.frames.len(), 3);
    assert!(!stack.frames[0].is_frame);
    assert_eq!(stack.frames[0].raw, "TypeError: boom");

    let work = &stack.frames[1];
    assert!(work.is_frame);
    assert_eq!(work.function.as_deref(), Some("doWork"));
    assert_eq!(work.url.as_deref(), Some("http://x/assets/app.min.js"));
    assert_eq!(work.line, Some(1));
    assert_eq!(work.column, Some(1234));

    let anon = &stack.frames[2];
    assert!(anon.is_frame);
    assert_eq!(anon.function, None);
    assert_eq!(anon.line, Some(2));
    assert_eq!(anon.column, Some(9));
}

#[test]
fn parses_firefox_frames() {
    let stack = parse("doWork@http://x/assets/app.min.js:1:1234", MAX);
    let frame = &stack.frames[0];
    assert!(frame.is_frame);
    assert_eq!(frame.function.as_deref(), Some("doWork"));
    assert_eq!(frame.url.as_deref(), Some("http://x/assets/app.min.js"));
    assert_eq!(frame.line, Some(1));
    assert_eq!(frame.column, Some(1234));
}

#[test]
fn keeps_frames_without_column() {
    let stack = parse("doWork@http://x/app.min.js:12", MAX);
    let frame = &stack.frames[0];
    assert!(frame.is_frame);
    assert_eq!(frame.line, Some(12));
    assert_eq!(frame.column, None);
}

#[test]
fn takes_the_innermost_parenthesized_position() {
    let stack = parse(
        "    at bar (eval at foo (app.min.js:1:100), <anonymous>:1:5)",
        MAX,
    );
    let frame = &stack.frames[0];
    assert_eq!(frame.url.as_deref(), Some("<anonymous>"));
    assert_eq!(frame.line, Some(1));
    assert_eq!(frame.column, Some(5));
    assert_eq!(frame.function.as_deref(), Some("bar"));
}

#[test]
fn keeps_urls_with_ports_and_paths() {
    let stack = parse("at f (https://host:8080/a/b.js:3:4)", MAX);
    let frame = &stack.frames[0];
    assert_eq!(frame.url.as_deref(), Some("https://host:8080/a/b.js"));
    assert_eq!(frame.line, Some(3));
    assert_eq!(frame.column, Some(4));
}

#[test]
fn keeps_message_lines_untouched() {
    let stack = parse("Uncaught RangeError: Invalid array length", MAX);
    let frame = &stack.frames[0];
    assert!(!frame.is_frame);
    assert_eq!(frame.url, None);
}

#[test]
fn handles_webpack_and_scheme_urls() {
    let stack = parse("at Module.x (webpack:///./src/app.js:10:20)", MAX);
    let frame = &stack.frames[0];
    assert_eq!(frame.url.as_deref(), Some("webpack:///./src/app.js"));
    assert_eq!(frame.line, Some(10));
}

#[test]
fn skips_empty_lines_and_truncates() {
    let text = "\n\na\n\nb\n\nc\n";
    let stack = parse(text, 2);
    assert!(stack.truncated);
    assert_eq!(stack.frames.len(), 2);
    let stack = parse(text, MAX);
    assert!(!stack.truncated);
    assert_eq!(stack.frames.len(), 3);
}
