use super::*;

fn html(source: &str) -> String {
    render(Args {
        source: source.to_owned(),
    })
    .unwrap()
    .html
}

#[test]
fn renders_gfm_extensions() {
    let out = html(
        "| a | b |\n|---|---|\n| 1 | 2 |\n\n- [x] done\n- [ ] todo\n\n~~old~~ text[^1]\n\n[^1]: note\n",
    );
    assert!(out.contains("<table>"), "{out}");
    assert!(
        out.contains("<input disabled=\"\" type=\"checkbox\" checked=\"\"/>"),
        "{out}"
    );
    assert!(out.contains("<del>old</del>"), "{out}");
    assert!(out.contains("class=\"footnote-definition\""), "{out}");
    let alert = html("> [!WARNING]\n> careful\n");
    assert!(alert.contains("markdown-alert-warning"), "{alert}");
}

#[test]
fn raw_html_is_shown_as_text() {
    let out = html("<script>alert(1)</script>\n\nhi <img src=x onerror=alert(1)>\n");
    assert!(!out.contains("<script"), "{out}");
    assert!(!out.contains("<img"), "{out}");
    assert!(out.contains("&lt;script&gt;"), "{out}");
}

#[test]
fn unsafe_links_are_removed() {
    let out = html(
        "[a](javascript:alert(1)) [b](java\tscript:x) ![c](data:image/svg+xml,x) ![d](data:image/png;base64,AA==) [e](https://x.dev) [f](#top) [g](docs/a.md)",
    );
    assert!(!out.contains("javascript"), "{out}");
    assert!(!out.contains("svg"), "{out}");
    for kept in [
        "data:image/png;base64,AA==",
        "https://x.dev",
        "href=\"#top\"",
        "docs/a.md",
    ] {
        assert!(out.contains(kept), "{kept} missing in {out}");
    }
    assert!(safe_url("mailto:a@b.c", false));
    assert!(!safe_url("vbscript:x", false));
    assert!(!safe_url(" JAVASCRIPT:x", false));
    assert!(safe_url("path/with:colon", false));
}

#[test]
fn builds_outline_with_unique_anchors() {
    let rendered = render(Args {
        source: "---\ntitle: meta words here\n---\n# Intro\n\ntext\n\n## Usage `cli`\n\n## Usage `cli`\n\n### 中文 标题！\n\n## Custom {#my-id}\n".into(),
    })
    .unwrap();
    let ids: Vec<_> = rendered
        .outline
        .iter()
        .map(|h| (h.level, h.id.as_str(), h.line))
        .collect();
    assert_eq!(
        ids,
        vec![
            (1, "intro", 4),
            (2, "usage-cli", 8),
            (2, "usage-cli-1", 10),
            (3, "中文-标题", 12),
            (2, "my-id", 14)
        ]
    );
    assert_eq!(rendered.outline[1].text, "Usage cli");
    assert_eq!(rendered.title.as_deref(), Some("Intro"));
    assert!(
        rendered.html.contains("<h2 id=\"usage-cli-1\">"),
        "{}",
        rendered.html
    );
    // 元数据块不渲染也不计入字数
    assert!(!rendered.html.contains("meta words"));
}

#[test]
fn counts_stats() {
    let rendered = render(Args {
        source: "# Title\n\nHello world, it's 中文字数。\n\n- [x] a\n- [ ] b\n\n```rust\nfn main() {}\n```\n\n[l](https://a.b) ![i](x.png)\n".into(),
    })
    .unwrap();
    let stats = rendered.stats;
    assert_eq!((stats.tasks, stats.tasks_done), (2, 1));
    assert_eq!((stats.links, stats.images, stats.code_blocks), (1, 1, 1));
    // 与编辑器一致：末尾换行后还有一个空行
    assert_eq!(stats.lines, 13);
    assert_eq!(stats.reading_minutes, 1);
    // Title Hello world it's a b fn main l i = 10 个词，另有 4 个汉字
    assert_eq!(stats.words, 14);
    assert_eq!(count_words("don't stop"), (2, 0));
    assert_eq!(count_words("  "), (0, 0));
    assert_eq!(
        render(Args {
            source: String::new()
        })
        .unwrap()
        .stats,
        Stats::default()
    );
}

#[test]
fn rejects_huge_documents() {
    let big = "a".repeat(MAX_SOURCE + 1);
    assert_eq!(
        render(Args { source: big }).unwrap_err().code,
        "md.too_large"
    );
}
