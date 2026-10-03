use ropey::Rope;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkdownAction {
    Bold,
    Italic,
    Heading(u8),
    UnorderedList,
    OrderedList,
    TaskList,
    Blockquote,
    Link,
    Image,
    InlineCode,
    FencedCode,
    HorizontalRule,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormattedEdit {
    /// Byte offsets into the original UTF-8 source.
    pub start: usize,
    pub end: usize,
    pub replacement: String,
    /// Byte offsets into the source after applying this edit.
    pub selection: std::ops::Range<usize>,
}

/// Builds a portable Markdown edit for a UTF-8 byte selection.
///
/// This function has no UI dependency so desktop commands, tests, and a future
/// collaboration layer can all express formatting through the same operation.
pub fn markdown_edit(
    source: &str,
    selection: std::ops::Range<usize>,
    action: MarkdownAction,
) -> Result<FormattedEdit, EditError> {
    if selection.start > selection.end
        || selection.end > source.len()
        || !source.is_char_boundary(selection.start)
        || !source.is_char_boundary(selection.end)
    {
        return Err(EditError::InvalidRange {
            start: selection.start,
            end: selection.end,
        });
    }

    let selected = &source[selection.clone()];
    let (before, content, after) = match action {
        MarkdownAction::Bold => ("**", fallback(selected, "bold text"), "**"),
        MarkdownAction::Italic => ("_", fallback(selected, "italic text"), "_"),
        MarkdownAction::Link => ("[", fallback(selected, "link text"), "](https://)"),
        MarkdownAction::Image => ("![", fallback(selected, "alt text"), "](image.png)"),
        MarkdownAction::InlineCode => ("`", fallback(selected, "code"), "`"),
        MarkdownAction::FencedCode => ("```\n", fallback(selected, "code"), "\n```"),
        MarkdownAction::HorizontalRule => {
            let (insertion, before) = source[selection.end..]
                .find('\n')
                .map_or((source.len(), !source.is_empty()), |newline| {
                    (selection.end + newline + 1, false)
                });
            let replacement = format!("{}---\n", if before { "\n" } else { "" });
            let cursor = insertion + replacement.len();
            return Ok(FormattedEdit {
                start: insertion,
                end: insertion,
                replacement,
                selection: cursor..cursor,
            });
        }
        MarkdownAction::Heading(level) => {
            let level = level.clamp(1, 6) as usize;
            return line_prefix_edit(
                source,
                selection,
                &format!("{} ", "#".repeat(level)),
                "Heading",
            );
        }
        MarkdownAction::UnorderedList => {
            return line_prefix_edit(source, selection, "- ", "List item");
        }
        MarkdownAction::OrderedList => return numbered_list_edit(source, selection),
        MarkdownAction::TaskList => return line_prefix_edit(source, selection, "- [ ] ", "Task"),
        MarkdownAction::Blockquote => return line_prefix_edit(source, selection, "> ", "Quote"),
    };

    let replacement = format!("{before}{content}{after}");
    let content_start = selection.start + before.len();
    Ok(FormattedEdit {
        start: selection.start,
        end: selection.end,
        selection: content_start..content_start + content.len(),
        replacement,
    })
}

fn fallback<'a>(selected: &'a str, placeholder: &'a str) -> &'a str {
    if selected.is_empty() {
        placeholder
    } else {
        selected
    }
}

fn line_prefix_edit(
    source: &str,
    selection: std::ops::Range<usize>,
    prefix: &str,
    placeholder: &str,
) -> Result<FormattedEdit, EditError> {
    let (start, end) = selected_line_bounds(source, &selection);
    let content = fallback(&source[start..end], placeholder);
    let replacement = content
        .lines()
        .map(|line| format!("{prefix}{line}"))
        .collect::<Vec<_>>()
        .join("\n");
    Ok(FormattedEdit {
        start,
        end,
        selection: start + prefix.len()..start + replacement.len(),
        replacement,
    })
}

fn numbered_list_edit(
    source: &str,
    selection: std::ops::Range<usize>,
) -> Result<FormattedEdit, EditError> {
    let (start, end) = selected_line_bounds(source, &selection);
    let content = fallback(&source[start..end], "List item");
    let replacement = content
        .lines()
        .enumerate()
        .map(|(index, line)| format!("{}. {line}", index + 1))
        .collect::<Vec<_>>()
        .join("\n");
    let prefix_len = "1. ".len();
    Ok(FormattedEdit {
        start,
        end,
        selection: start + prefix_len..start + replacement.len(),
        replacement,
    })
}

fn selected_line_bounds(source: &str, selection: &std::ops::Range<usize>) -> (usize, usize) {
    let start = source[..selection.start]
        .rfind('\n')
        .map_or(0, |newline| newline + 1);
    let end = if selection.end > start && source.as_bytes().get(selection.end - 1) == Some(&b'\n') {
        selection.end - 1
    } else {
        source[selection.end..]
            .find('\n')
            .map_or(source.len(), |newline| selection.end + newline)
    };
    (start, end)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Edit {
    Replace {
        start: usize,
        end: usize,
        text: String,
    },
}

#[derive(Debug, Clone)]
struct Snapshot {
    body: Rope,
    title: String,
}

/// UI-independent Markdown document state. Offsets are Unicode scalar indexes,
/// matching Ropey's native indexing and avoiding byte-boundary mistakes.
#[derive(Debug, Clone)]
pub struct Document {
    pub path: PathBuf,
    pub title: String,
    body: Rope,
    saved_body: Rope,
    saved_title: String,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
}

impl Document {
    pub fn new(path: PathBuf, title: impl Into<String>, body: impl AsRef<str>) -> Self {
        let title = title.into();
        let body = Rope::from_str(body.as_ref());
        Self {
            path,
            saved_body: body.clone(),
            saved_title: title.clone(),
            title,
            body,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    pub fn body(&self) -> String {
        self.body.to_string()
    }

    pub fn is_dirty(&self) -> bool {
        self.title != self.saved_title || self.body != self.saved_body
    }

    pub fn set_body(&mut self, body: impl AsRef<str>) {
        if self.body == Rope::from_str(body.as_ref()) {
            return;
        }
        self.checkpoint();
        self.body = Rope::from_str(body.as_ref());
    }

    pub fn rename(&mut self, title: impl Into<String>) {
        let title = title.into();
        if self.title != title {
            self.checkpoint();
            self.title = title;
        }
    }

    pub fn apply(&mut self, edit: Edit) -> Result<(), EditError> {
        match edit {
            Edit::Replace { start, end, text } => {
                if start > end || end > self.body.len_chars() {
                    return Err(EditError::InvalidRange { start, end });
                }
                self.checkpoint();
                self.body.remove(start..end);
                self.body.insert(start, &text);
            }
        }
        Ok(())
    }

    pub fn format(
        &mut self,
        selection: std::ops::Range<usize>,
        action: MarkdownAction,
    ) -> Result<std::ops::Range<usize>, EditError> {
        if selection.start > selection.end || selection.end > self.body.len_chars() {
            return Err(EditError::InvalidRange {
                start: selection.start,
                end: selection.end,
            });
        }
        let source = self.body();
        let byte_range =
            self.body.char_to_byte(selection.start)..self.body.char_to_byte(selection.end);
        let formatted = markdown_edit(&source, byte_range, action)?;
        let start_char = source[..formatted.start].chars().count();
        let end_char = source[..formatted.end].chars().count();
        let selected_start = source[..formatted.start].chars().count()
            + formatted.replacement[..formatted.selection.start - formatted.start]
                .chars()
                .count();
        let selected_len = formatted.replacement[formatted.selection.start - formatted.start
            ..formatted.selection.end - formatted.start]
            .chars()
            .count();
        self.apply(Edit::Replace {
            start: start_char,
            end: end_char,
            text: formatted.replacement,
        })?;
        Ok(selected_start..selected_start + selected_len)
    }

    pub fn undo(&mut self) -> bool {
        let Some(snapshot) = self.undo.pop() else {
            return false;
        };
        self.redo.push(self.snapshot());
        self.restore(snapshot);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(snapshot) = self.redo.pop() else {
            return false;
        };
        self.undo.push(self.snapshot());
        self.restore(snapshot);
        true
    }

    pub fn mark_saved(&mut self) {
        self.saved_body = self.body.clone();
        self.saved_title.clone_from(&self.title);
    }

    fn checkpoint(&mut self) {
        self.undo.push(self.snapshot());
        self.redo.clear();
        if self.undo.len() > 256 {
            self.undo.remove(0);
        }
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            body: self.body.clone(),
            title: self.title.clone(),
        }
    }

    fn restore(&mut self, snapshot: Snapshot) {
        self.body = snapshot.body;
        self.title = snapshot.title;
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum EditError {
    #[error("invalid edit range {start}..{end}")]
    InvalidRange { start: usize, end: usize },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_unicode_by_character_and_tracks_history() {
        let mut doc = Document::new("note.md".into(), "Note", "a🟣c");
        doc.apply(Edit::Replace {
            start: 1,
            end: 2,
            text: "purple".into(),
        })
        .unwrap();
        assert_eq!(doc.body(), "apurplec");
        assert!(doc.is_dirty());
        assert!(doc.undo());
        assert_eq!(doc.body(), "a🟣c");
        assert!(doc.redo());
        assert_eq!(doc.body(), "apurplec");
    }

    #[test]
    fn saved_state_includes_title_and_body() {
        let mut doc = Document::new("note.md".into(), "Note", "body");
        doc.rename("Renamed");
        assert!(doc.is_dirty());
        doc.mark_saved();
        assert!(!doc.is_dirty());
    }

    #[test]
    fn formatting_produces_standard_markdown_and_preserves_unicode() {
        let mut doc = Document::new("note.md".into(), "Note", "Purple 🟣 idea");
        let selection = doc.format(7..8, MarkdownAction::Bold).unwrap();
        assert_eq!(doc.body(), "Purple **🟣** idea");
        assert_eq!(&doc.body().chars().collect::<Vec<_>>()[selection], &['🟣']);
    }

    #[test]
    fn line_actions_format_each_selected_line() {
        let mut doc = Document::new("note.md".into(), "Note", "one\ntwo");
        doc.format(0..7, MarkdownAction::TaskList).unwrap();
        assert_eq!(doc.body(), "- [ ] one\n- [ ] two");
    }

    #[test]
    fn line_actions_expand_partial_selection_without_damaging_context() {
        let source = "before\nhello world\nafter";
        let start = source.find("world").unwrap();
        let edit = markdown_edit(source, start..start + 5, MarkdownAction::Heading(2)).unwrap();
        let mut result = source.to_string();
        result.replace_range(edit.start..edit.end, &edit.replacement);
        assert_eq!(result, "before\n## hello world\nafter");
    }

    #[test]
    fn horizontal_rule_never_replaces_selected_text() {
        let source = "keep this text";
        let edit = markdown_edit(source, 5..9, MarkdownAction::HorizontalRule).unwrap();
        let mut result = source.to_string();
        result.replace_range(edit.start..edit.end, &edit.replacement);
        assert!(result.contains("keep this text"));
        assert_eq!(result, "keep this text\n---\n");
    }

    #[test]
    fn all_actions_remain_plain_markdown() {
        let cases = [
            (MarkdownAction::Italic, "_text_"),
            (MarkdownAction::Link, "[text](https://)"),
            (MarkdownAction::Image, "![text](image.png)"),
            (MarkdownAction::InlineCode, "`text`"),
            (MarkdownAction::FencedCode, "```\ntext\n```"),
            (MarkdownAction::Heading(2), "## text"),
            (MarkdownAction::UnorderedList, "- text"),
            (MarkdownAction::OrderedList, "1. text"),
            (MarkdownAction::Blockquote, "> text"),
        ];
        for (action, expected) in cases {
            let mut doc = Document::new("note.md".into(), "Note", "text");
            doc.format(0..4, action).unwrap();
            assert_eq!(doc.body(), expected);
        }
    }
}
