import type { EditorView } from "@codemirror/view";

export type FormatAction =
  | "bold"
  | "italic"
  | "heading"
  | "list"
  | "quote"
  | "link"
  | "code"
  | "codeblock";

export interface TextEdit {
  from: number;
  to: number;
  insert: string;
  anchor: number;
  head: number;
}

export function markdownFormat(
  source: string,
  from: number,
  to: number,
  action: FormatAction,
): TextEdit {
  const selected = source.slice(from, to);
  const wrap = (before: string, fallback: string, after = before): TextEdit => {
    const content = selected || fallback;
    return {
      from,
      to,
      insert: `${before}${content}${after}`,
      anchor: from + before.length,
      head: from + before.length + content.length,
    };
  };

  if (action === "bold") return wrap("**", "bold text");
  if (action === "italic") return wrap("_", "italic text");
  if (action === "link") return wrap("[", "link text", "](https://)");
  if (action === "code") return wrap("`", "code");
  if (action === "codeblock") return wrap("```\n", "code", "\n```");

  const lineStart = source.lastIndexOf("\n", Math.max(0, from - 1)) + 1;
  const nextBreak = source.indexOf("\n", to);
  const lineEnd = nextBreak < 0 ? source.length : nextBreak;
  const content = source.slice(lineStart, lineEnd) || (action === "heading" ? "Heading" : "List item");
  const prefix = action === "heading" ? "## " : action === "quote" ? "> " : "- ";
  const insert = content
    .split("\n")
    .map((line) => `${prefix}${line}`)
    .join("\n");
  return {
    from: lineStart,
    to: lineEnd,
    insert,
    anchor: lineStart + prefix.length,
    head: lineStart + insert.length,
  };
}

export function applyFormat(view: EditorView, action: FormatAction): void {
  const selection = view.state.selection.main;
  const edit = markdownFormat(view.state.doc.toString(), selection.from, selection.to, action);
  view.dispatch({
    changes: { from: edit.from, to: edit.to, insert: edit.insert },
    selection: { anchor: edit.anchor, head: edit.head },
    scrollIntoView: true,
  });
  view.focus();
}
