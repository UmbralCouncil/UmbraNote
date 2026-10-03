use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use std::ops::Range;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Outline {
    pub headings: Vec<String>,
    pub links: usize,
    pub images: usize,
    pub code_blocks: usize,
    pub tables: usize,
    pub task_items: usize,
}

/// A presentation-only rendering of portable Markdown. Byte ranges refer to
/// `text`, so the Floem layer can apply typography without owning or changing
/// the source document.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RenderedMarkdown {
    pub text: String,
    pub spans: Vec<RenderedSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedSpan {
    pub range: Range<usize>,
    pub style: RenderStyle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderStyle {
    Heading(u8),
    Strong,
    Emphasis,
    Code,
    CodeBlock,
    Link,
    Quote,
    Image,
    Table,
}

/// Convert Markdown events into display text and style ranges. The original
/// source remains the canonical representation and is never rewritten here.
pub fn render(source: &str) -> RenderedMarkdown {
    let mut out = RenderedMarkdown::default();
    let mut open = Vec::<(RenderStyle, usize)>::new();
    let mut lists = Vec::<Option<u64>>::new();

    let push = |text: &str, out: &mut RenderedMarkdown| out.text.push_str(text);
    let close = |wanted: fn(&RenderStyle) -> bool,
                 out: &mut RenderedMarkdown,
                 open: &mut Vec<(RenderStyle, usize)>| {
        if let Some(index) = open.iter().rposition(|(style, _)| wanted(style)) {
            let (style, start) = open.remove(index);
            if start < out.text.len() {
                out.spans.push(RenderedSpan {
                    range: start..out.text.len(),
                    style,
                });
            }
        }
    };

    for event in Parser::new_ext(source, Options::all()) {
        match event {
            Event::Start(tag) => match tag {
                Tag::Heading { level, .. } => {
                    open.push((RenderStyle::Heading(level as u8), out.text.len()));
                }
                Tag::Strong => open.push((RenderStyle::Strong, out.text.len())),
                Tag::Emphasis => open.push((RenderStyle::Emphasis, out.text.len())),
                Tag::Link { .. } => open.push((RenderStyle::Link, out.text.len())),
                Tag::Image { .. } => {
                    push("▣ ", &mut out);
                    open.push((RenderStyle::Image, out.text.len() - "▣ ".len()));
                }
                Tag::CodeBlock(_) => open.push((RenderStyle::CodeBlock, out.text.len())),
                Tag::BlockQuote(_) => {
                    push("│ ", &mut out);
                    open.push((RenderStyle::Quote, out.text.len() - "│ ".len()));
                }
                Tag::List(start) => lists.push(start),
                Tag::Item => {
                    if let Some(list) = lists.last_mut() {
                        match list {
                            Some(number) => {
                                push(&format!("{number}. "), &mut out);
                                *number += 1;
                            }
                            None => push("• ", &mut out),
                        }
                    }
                }
                Tag::Table(_) => {
                    open.push((RenderStyle::Table, out.text.len()));
                }
                Tag::TableRow => push("│ ", &mut out),
                _ => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Heading(_) => {
                    close(
                        |s| matches!(s, RenderStyle::Heading(_)),
                        &mut out,
                        &mut open,
                    );
                    push("\n\n", &mut out);
                }
                TagEnd::Paragraph => push("\n\n", &mut out),
                TagEnd::Strong => close(|s| matches!(s, RenderStyle::Strong), &mut out, &mut open),
                TagEnd::Emphasis => {
                    close(|s| matches!(s, RenderStyle::Emphasis), &mut out, &mut open)
                }
                TagEnd::Link => close(|s| matches!(s, RenderStyle::Link), &mut out, &mut open),
                TagEnd::Image => {
                    close(|s| matches!(s, RenderStyle::Image), &mut out, &mut open);
                }
                TagEnd::CodeBlock => {
                    close(|s| matches!(s, RenderStyle::CodeBlock), &mut out, &mut open);
                    push("\n\n", &mut out);
                }
                TagEnd::BlockQuote(_) => {
                    close(|s| matches!(s, RenderStyle::Quote), &mut out, &mut open);
                    push("\n", &mut out);
                }
                TagEnd::Item => push("\n", &mut out),
                TagEnd::List(_) => {
                    lists.pop();
                    push("\n", &mut out);
                }
                TagEnd::TableCell => push(" │ ", &mut out),
                TagEnd::TableRow => push("\n", &mut out),
                TagEnd::Table => {
                    close(|s| matches!(s, RenderStyle::Table), &mut out, &mut open);
                    push("\n", &mut out);
                }
                _ => {}
            },
            Event::Text(text) => push(&text, &mut out),
            Event::Code(text) => {
                let start = out.text.len();
                push(&text, &mut out);
                out.spans.push(RenderedSpan {
                    range: start..out.text.len(),
                    style: RenderStyle::Code,
                });
            }
            Event::SoftBreak => push("\n", &mut out),
            Event::HardBreak => push("\n", &mut out),
            Event::Rule => push("────────────────────────────────\n\n", &mut out),
            Event::TaskListMarker(done) => push(if done { "☑ " } else { "☐ " }, &mut out),
            Event::Html(html) | Event::InlineHtml(html) => push(&html, &mut out),
            Event::FootnoteReference(name) => push(&format!("[{name}]"), &mut out),
            Event::InlineMath(math) => push(&math, &mut out),
            Event::DisplayMath(math) => {
                push(&math, &mut out);
                push("\n\n", &mut out);
            }
        }
    }

    // pulldown-cmark normally balances tags; closing defensively keeps malformed
    // input renderable while users are midway through typing.
    for (style, start) in open {
        if start < out.text.len() {
            out.spans.push(RenderedSpan {
                range: start..out.text.len(),
                style,
            });
        }
    }
    while out.text.ends_with("\n\n\n") {
        out.text.pop();
    }
    out
}

pub fn analyze(source: &str) -> Outline {
    let mut outline = Outline::default();
    let mut heading = None::<String>;
    for event in Parser::new_ext(source, Options::all()) {
        match event {
            Event::Start(Tag::Heading { .. }) => heading = Some(String::new()),
            Event::Text(text) | Event::Code(text) if heading.is_some() => {
                heading.as_mut().unwrap().push_str(&text)
            }
            Event::End(pulldown_cmark::TagEnd::Heading(_)) => {
                if let Some(text) = heading.take() {
                    outline.headings.push(text)
                }
            }
            Event::Start(Tag::Link { .. }) => outline.links += 1,
            Event::Start(Tag::Image { .. }) => outline.images += 1,
            Event::Start(Tag::CodeBlock(_)) => outline.code_blocks += 1,
            Event::Start(Tag::Table(_)) => outline.tables += 1,
            Event::TaskListMarker(_) => outline.task_items += 1,
            _ => {}
        }
    }
    outline
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_structure() {
        let result = analyze(
            "# Hello *world*\n\n[a](https://example.com) ![i](image.png)\n\n- [ ] task\n\n| A | B |\n| - | - |\n| 1 | 2 |\n\n```rs\nfn x() {}\n```",
        );
        assert_eq!(result.headings, ["Hello world"]);
        assert_eq!(result.links, 1);
        assert_eq!(result.images, 1);
        assert_eq!(result.code_blocks, 1);
        assert_eq!(result.tables, 1);
        assert_eq!(result.task_items, 1);
    }

    #[test]
    fn renders_standard_markdown_without_source_markup() {
        let rendered = render(
            "# Title\n\nHello **bold** and *soft* with `code`.\n\n- [x] done\n- [ ] next\n\n> quoted\n\n| A | B |\n| - | - |\n| 1 | 2 |",
        );
        assert!(
            rendered
                .text
                .starts_with("Title\n\nHello bold and soft with code.")
        );
        assert!(rendered.text.contains("• ☑ done"));
        assert!(rendered.text.contains("│ quoted"));
        assert!(!rendered.text.contains("**bold**"));
        assert!(
            rendered
                .spans
                .iter()
                .any(|span| span.style == RenderStyle::Heading(1))
        );
        assert!(
            rendered
                .spans
                .iter()
                .any(|span| span.style == RenderStyle::Strong)
        );
        assert!(
            rendered
                .spans
                .iter()
                .any(|span| span.style == RenderStyle::Table)
        );
    }

    #[test]
    fn rendering_does_not_modify_markdown_source() {
        let source = "![alt](image.png)\n\n```rust\nfn main() {}\n```";
        let before = source.to_string();
        let rendered = render(source);
        assert_eq!(source, before);
        assert!(rendered.text.contains("▣ alt"));
        assert!(rendered.text.contains("fn main() {}"));
        assert!(
            rendered
                .spans
                .iter()
                .any(|span| span.style == RenderStyle::CodeBlock)
        );
    }
}
