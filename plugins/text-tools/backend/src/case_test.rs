use super::*;

fn style(input: &str, name: &str) -> String {
    convert(input, name)
}

#[test]
fn converts_between_programming_cases() {
    let input = "hello world-fooBar";
    assert_eq!(style(input, "camel"), "helloWorldFooBar");
    assert_eq!(style(input, "pascal"), "HelloWorldFooBar");
    assert_eq!(style(input, "snake"), "hello_world_foo_bar");
    assert_eq!(style(input, "kebab"), "hello-world-foo-bar");
    assert_eq!(style(input, "constant"), "HELLO_WORLD_FOO_BAR");
    assert_eq!(style(input, "train"), "Hello-World-Foo-Bar");
    assert_eq!(style(input, "dot"), "hello.world.foo.bar");
    assert_eq!(style(input, "path"), "hello/world/foo/bar");
    assert_eq!(style(input, "title"), "Hello World Foo Bar");
}

#[test]
fn text_cases() {
    assert_eq!(style("hELLO World", "sentence"), "Hello world");
    assert_eq!(style("Straße", "upper"), "STRASSE");
    assert_eq!(style("Hello 世界", "swap"), "hELLO 世界");
}

#[test]
fn all_styles_preserve_lines() {
    let out = all(Args {
        input: "user id\r\norder total".into(),
    });
    assert_eq!(out.len(), STYLES.len());
    let snake = out.iter().find(|c| c.style == "snake").unwrap();
    assert_eq!(snake.output, "user_id\norder_total");
}
