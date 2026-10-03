import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { languages } from "@codemirror/language-data";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { tags } from "@lezer/highlight";
import { GFM } from "@lezer/markdown";
import { Compartment, EditorState } from "@codemirror/state";
import { EditorView, drawSelection, keymap } from "@codemirror/view";
import { createEffect, onCleanup, onMount } from "solid-js";
import { applyFormat, type FormatAction } from "./format";
import { liveMarkdownPreview } from "./livePreview";

const umbraCodeHighlight = HighlightStyle.define([
  { tag: tags.keyword, color: "#c4adff", fontWeight: "600" },
  { tag: [tags.typeName, tags.className, tags.namespace], color: "#a9c7ff" },
  { tag: [tags.function(tags.variableName), tags.definition(tags.function(tags.variableName))], color: "#d7c7ff" },
  { tag: [tags.string, tags.special(tags.string)], color: "#b8d8b0" },
  { tag: [tags.number, tags.bool, tags.null], color: "#e6b98d" },
  { tag: [tags.comment, tags.docComment], color: "#786f83", fontStyle: "italic" },
  { tag: [tags.operator, tags.punctuation], color: "#a49aac" },
  { tag: [tags.propertyName, tags.attributeName], color: "#d8a7d5" },
  { tag: [tags.regexp, tags.escape], color: "#e2a6b8" },
  { tag: [tags.meta, tags.annotation], color: "#8fbec4" },
  { tag: tags.invalid, color: "#ff8297", textDecoration: "underline wavy" },
]);

export interface EditorController {
  format(action: FormatAction): void;
  focus(): void;
}

export function MarkdownEditor(props: {
  path: string;
  content: string;
  onChange(content: string): void;
  onSave(): void;
  onReady(controller: EditorController): void;
}) {
  let host!: HTMLDivElement;
  let view: EditorView | undefined;
  const documentSlot = new Compartment();

  onMount(() => {
    view = new EditorView({
      parent: host,
      state: EditorState.create({
        doc: props.content,
        extensions: [
          documentSlot.of([]),
          history(),
          markdown({ base: markdownLanguage, extensions: [GFM], codeLanguages: languages }),
          syntaxHighlighting(umbraCodeHighlight),
          liveMarkdownPreview,
          drawSelection({ cursorBlinkRate: 0 }),
          EditorView.lineWrapping,
          EditorView.contentAttributes.of({ spellcheck: "true", "aria-label": "Markdown editor" }),
          keymap.of([
            { key: "Mod-s", run: () => (props.onSave(), true) },
            { key: "Mod-b", run: (editor) => (applyFormat(editor, "bold"), true) },
            { key: "Mod-i", run: (editor) => (applyFormat(editor, "italic"), true) },
            indentWithTab,
            ...defaultKeymap,
            ...historyKeymap,
          ]),
          EditorView.updateListener.of((update) => {
            if (update.docChanged) props.onChange(update.state.doc.toString());
          }),
        ],
      }),
    });
    view.dom.dataset.path = props.path;
    props.onReady({
      format: (action) => view && applyFormat(view, action),
      focus: () => view?.focus(),
    });
  });

  createEffect(() => {
    const path = props.path;
    if (!view) return;
    const currentPath = view.dom.dataset.path;
    if (currentPath !== path) {
      view.dom.dataset.path = path;
      view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: props.content } });
      view.dispatch({ selection: { anchor: 0 } });
    }
  });

  onCleanup(() => view?.destroy());
  return <div class="editor-host" ref={host} />;
}
