use serde_json::json;

use super::*;
use crate::model::build;
use crate::schema::infer;

fn code(value: serde_json::Value, language: Language) -> String {
    let model = build(&infer(&value), "Root");
    emit(
        &model,
        language,
        &Options {
            root: "Root".into(),
        },
    )
}

fn sample() -> serde_json::Value {
    json!({
        "id": 1,
        "full-name": "Ada",
        "type": "admin",
        "score": 9.5,
        "tags": ["a"],
        "manager": null,
        "posts": [{"title": "x", "draft": true}, {"title": "y"}]
    })
}

#[test]
fn typescript_output() {
    assert_eq!(
        code(sample(), Language::Typescript),
        "export interface Root {\n  id: number;\n  \"full-name\": string;\n  type: string;\n  score: number;\n  tags: string[];\n  manager: unknown;\n  posts: Post[];\n}\n\nexport interface Post {\n  title: string;\n  draft?: boolean;\n}\n"
    );
}

#[test]
fn rust_output() {
    let out = code(sample(), Language::Rust);
    assert!(out.starts_with("use serde::{Deserialize, Serialize};"));
    assert!(out.contains("    #[serde(rename = \"full-name\")]\n    pub full_name: String,"));
    assert!(out.contains("    pub r#type: String,"));
    assert!(out.contains("    pub manager: serde_json::Value,"));
    assert!(out.contains("    #[serde(default, skip_serializing_if = \"Option::is_none\")]\n    pub draft: Option<bool>,"));
    assert!(out.contains("    pub posts: Vec<Post>,"));
}

#[test]
fn go_output() {
    let out = code(sample(), Language::Go);
    let root: Vec<&str> = out.lines().skip(1).take(7).collect();
    assert_eq!(root[0], "\tID       int64    `json:\"id\"`");
    assert_eq!(root[1], "\tFullName string   `json:\"full-name\"`");
    assert_eq!(root[5], "\tManager  any      `json:\"manager\"`");
    assert_eq!(root[6], "\tPosts    []Post   `json:\"posts\"`");
    assert!(
        out.contains("\tDraft *bool  `json:\"draft,omitempty\"`")
            || out.contains("\tDraft bool   `json:\"draft,omitempty\"`"),
        "{out}"
    );
    assert_eq!(go_name("user_url"), "UserURL");
    assert_eq!(go_name("api-key"), "APIKey");
}

#[test]
fn java_kotlin_python_csharp_outputs() {
    let java = code(sample(), Language::Java);
    assert!(java.contains("@JsonProperty(\"full-name\") String fullName"));
    // type 不是 Java 关键字，可直接作为字段名
    assert!(java.contains("    String type,"));
    assert!(
        code(json!({"class": 1}), Language::Java).contains("@JsonProperty(\"class\") long class_")
    );
    assert!(java.contains("Boolean draft"));
    assert!(java.contains("long id"));

    let kotlin = code(sample(), Language::Kotlin);
    assert!(kotlin.contains("@SerialName(\"full-name\") val fullName: String"));
    assert!(kotlin.contains("val draft: Boolean? = null"));
    assert!(kotlin.contains("val manager: JsonElement?"));

    let python = code(sample(), Language::Python);
    // 子类型先于父类型定义
    assert!(
        python.find("class Post(BaseModel)").unwrap()
            < python.find("class Root(BaseModel)").unwrap()
    );
    assert!(python.contains("full_name: str = Field(alias=\"full-name\")"));
    assert!(python.contains("draft: bool | None = Field(default=None)"));
    assert!(python.contains("    type: str\n"));
    assert!(
        code(json!({"from": 1}), Language::Python).contains("from_: int = Field(alias=\"from\")")
    );

    let csharp = code(sample(), Language::Csharp);
    assert!(csharp.contains("[JsonPropertyName(\"full-name\")]\n    public string FullName { get; set; } = string.Empty;"));
    assert!(csharp.contains("public bool? Draft { get; set; }"));
    assert!(csharp.contains("public List<Post> Posts { get; set; } = new();"));
    assert!(
        code(json!({"a": {"b": 1}}), Language::Csharp)
            .contains("public A A { get; set; } = new();")
    );
}

#[test]
fn root_arrays_get_an_alias() {
    let ts = code(json!([{"a": 1}]), Language::Typescript);
    assert!(ts.starts_with("export type Root = RootItem[];"));
    let py = code(json!([1, 2]), Language::Python);
    assert!(py.trim_end().ends_with("Root = list[int]"));
}
