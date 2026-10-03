import { describe, expect, it } from "vitest";
import { markdownFormat } from "./format";

function apply(source: string, from: number, to: number, action: Parameters<typeof markdownFormat>[3]) {
  const edit = markdownFormat(source, from, to, action);
  return source.slice(0, edit.from) + edit.insert + source.slice(edit.to);
}

describe("portable Markdown formatting", () => {
  it("bolds only the selected text", () => {
    expect(apply("hello world", 6, 11, "bold")).toBe("hello **world**");
  });

  it("formats the complete line for structural actions", () => {
    expect(apply("before\nhello world\nafter", 13, 18, "heading")).toBe(
      "before\n## hello world\nafter",
    );
  });

  it("preserves Unicode selections", () => {
    expect(apply("Purple 🟣 idea", 7, 9, "italic")).toBe("Purple _🟣_ idea");
  });
});
