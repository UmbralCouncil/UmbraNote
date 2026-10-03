import { syntaxTree } from "@codemirror/language";
import type { Range } from "@codemirror/state";
import {
  Decoration,
  type DecorationSet,
  EditorView,
  ViewPlugin,
  type ViewUpdate,
  WidgetType,
} from "@codemirror/view";

const markClasses: Record<string, string> = {
  ATXHeading1: "cm-live-h1",
  ATXHeading2: "cm-live-h2",
  ATXHeading3: "cm-live-h3",
  ATXHeading4: "cm-live-h4",
  ATXHeading5: "cm-live-h5",
  ATXHeading6: "cm-live-h6",
  StrongEmphasis: "cm-live-strong",
  Emphasis: "cm-live-emphasis",
  InlineCode: "cm-live-inline-code",
  FencedCode: "cm-live-codeblock",
  Blockquote: "cm-live-quote",
  Link: "cm-live-link",
  Image: "cm-live-image",
  Table: "cm-live-table",
};

const hiddenMarks = new Set(["HeaderMark", "EmphasisMark", "CodeMark", "QuoteMark"]);

class CheckboxWidget extends WidgetType {
  constructor(
    readonly checked: boolean,
    readonly from: number,
    readonly to: number,
  ) {
    super();
  }
  eq(other: CheckboxWidget) {
    return other.checked === this.checked && other.from === this.from;
  }
  toDOM(view: EditorView) {
    const checkbox = document.createElement("input");
    checkbox.type = "checkbox";
    checkbox.checked = this.checked;
    checkbox.className = "cm-task-checkbox";
    checkbox.setAttribute("aria-label", this.checked ? "Mark task incomplete" : "Mark task complete");
    checkbox.addEventListener("mousedown", (event) => event.preventDefault());
    checkbox.addEventListener("change", () => {
      view.dispatch({ changes: { from: this.from, to: this.to, insert: this.checked ? "[ ]" : "[x]" } });
      view.focus();
    });
    return checkbox;
  }
  ignoreEvent() {
    return false;
  }
}

class RuleWidget extends WidgetType {
  toDOM() {
    const rule = document.createElement("hr");
    rule.className = "cm-markdown-rule";
    return rule;
  }
}

function intersectsSelection(view: EditorView, from: number, to: number): boolean {
  return view.state.selection.ranges.some((range) => range.from <= to && range.to >= from);
}

function decorations(view: EditorView): DecorationSet {
  const ranges: Range<Decoration>[] = [];
  syntaxTree(view.state).iterate({
    enter(node) {
      const className = markClasses[node.name];
      if (className) {
        ranges.push(Decoration.mark({ class: className }).range(node.from, node.to));
      }
      if (node.name === "TaskMarker" && !intersectsSelection(view, node.from, node.to)) {
        const checked = view.state.doc.sliceString(node.from, node.to).toLocaleLowerCase() === "[x]";
        ranges.push(
          Decoration.replace({ widget: new CheckboxWidget(checked, node.from, node.to) }).range(
            node.from,
            node.to,
          ),
        );
      }
      if (node.name === "HorizontalRule" && !intersectsSelection(view, node.from, node.to)) {
        ranges.push(Decoration.replace({ widget: new RuleWidget(), block: true }).range(node.from, node.to));
      }
      if (hiddenMarks.has(node.name)) {
        const parent = node.node.parent;
        const from = parent?.from ?? node.from;
        const to = parent?.to ?? node.to;
        if (!intersectsSelection(view, from, to)) {
          ranges.push(Decoration.replace({}).range(node.from, node.to));
        }
      }
      if (node.name === "LinkMark" || node.name === "URL") {
        const link = node.node.parent;
        if (link && ["Link", "Image"].includes(link.name) && !intersectsSelection(view, link.from, link.to)) {
          ranges.push(Decoration.replace({}).range(node.from, node.to));
        }
      }
    },
  });
  return Decoration.set(ranges, true);
}

export const liveMarkdownPreview = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    constructor(view: EditorView) {
      this.decorations = decorations(view);
    }
    update(update: ViewUpdate) {
      if (update.docChanged || update.selectionSet || update.viewportChanged) {
        this.decorations = decorations(update.view);
      }
    }
  },
  { decorations: (plugin) => plugin.decorations },
);
