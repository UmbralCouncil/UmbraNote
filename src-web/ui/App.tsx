import { For, Show, createMemo, createSignal, onMount } from "solid-js";
import type { EditorController } from "../editor/MarkdownEditor";
import { MarkdownEditor } from "../editor/MarkdownEditor";
import type { FormatAction } from "../editor/format";
import type { NoteFile, NoteSummary } from "../lib/types";
import noteIcon from "../../assets/note.png";

const tools: { label: string; title: string; action: FormatAction }[] = [
  { label: "B", title: "Bold", action: "bold" },
  { label: "I", title: "Italic", action: "italic" },
  { label: "H", title: "Heading", action: "heading" },
  { label: "•", title: "List", action: "list" },
  { label: "❯", title: "Quote", action: "quote" },
  { label: "↗", title: "Link", action: "link" },
  { label: "`", title: "Inline code", action: "code" },
  { label: "</>", title: "Code block", action: "codeblock" },
];

export function App() {
  const [notes, setNotes] = createSignal<NoteSummary[]>([]);
  const [active, setActive] = createSignal<NoteFile>();
  const [draft, setDraft] = createSignal("");
  const [title, setTitle] = createSignal("");
  const [query, setQuery] = createSignal("");
  const [dirty, setDirty] = createSignal(false);
  const [collapsed, setCollapsed] = createSignal(true);
  const [status, setStatus] = createSignal("Ready");
  const [error, setError] = createSignal("");
  let editor: EditorController | undefined;
  let autosave: ReturnType<typeof setTimeout> | undefined;

  const filtered = createMemo(() => {
    const needle = query().trim().toLocaleLowerCase();
    return needle ? notes().filter((note) => note.title.toLocaleLowerCase().includes(needle)) : notes();
  });

  async function refresh() {
    setNotes(await window.umbra.listNotes());
  }

  async function save(showStatus = true): Promise<boolean> {
    const note = active();
    if (!note || !dirty()) return true;
    clearTimeout(autosave);
    const content = draft();
    try {
      const saved = await window.umbra.saveNote(note.path, content);
      setActive({ ...saved, content });
      const unchangedSinceSaveStarted = draft() === content;
      setDirty(!unchangedSinceSaveStarted);
      if (showStatus && unchangedSinceSaveStarted) setStatus("Saved to disk");
      setError("");
      await refresh();
      return true;
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
      setStatus("Save failed");
      return false;
    }
  }

  function changed(content: string) {
    setDraft(content);
    setDirty(true);
    setStatus("Unsaved changes");
    clearTimeout(autosave);
    autosave = setTimeout(() => void save(false), 900);
  }

  async function open(note: NoteSummary) {
    if (active()?.path === note.path) return;
    if (!(await save(false))) return;
    try {
      const loaded = await window.umbra.readNote(note.path);
      setActive(loaded);
      setDraft(loaded.content);
      setTitle(loaded.title);
      setDirty(false);
      setStatus("Opened");
      setError("");
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  async function createNote() {
    if (!(await save(false))) return;
    try {
      const note = await window.umbra.createNote();
      setActive(note);
      setDraft(note.content);
      setTitle(note.title);
      setDirty(false);
      setStatus("New note created");
      setError("");
      await refresh();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  async function importNote() {
    if (!(await save(false))) return;
    try {
      const note = await window.umbra.importNote();
      if (!note) return;
      setActive(note);
      setDraft(note.content);
      setTitle(note.title);
      setDirty(false);
      setStatus("Markdown note imported");
      setError("");
      await refresh();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  async function rename() {
    const note = active();
    if (!note) return;
    if (!(await save(false))) return;
    try {
      const renamed = await window.umbra.renameNote(note.path, title());
      setActive(renamed);
      setDraft(renamed.content);
      setTitle(renamed.title);
      setDirty(false);
      setStatus("Renamed");
      await refresh();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  onMount(async () => {
    try {
      await refresh();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  });

  return (
    <main class="app-shell" classList={{ "sidebar-collapsed": collapsed() }}>
      <aside class="sidebar">
        <header class="brand-row">
          <div class="brand-mark"><img src={noteIcon} alt="" /></div>
          <Show when={!collapsed()}>
            <div class="brand-name"><strong>UMBRA</strong><span>NOTE</span></div>
          </Show>
          <button class="icon-button collapse" onClick={() => setCollapsed((value) => !value)}>
            {collapsed() ? "›" : "‹"}
          </button>
        </header>

        <div class="sidebar-controls">
          <Show when={!collapsed()}>
            <input
              class="search"
              value={query()}
              onInput={(event) => setQuery(event.currentTarget.value)}
              placeholder="Search notes…"
              aria-label="Search notes"
            />
          </Show>
          <button class="primary new-note" onClick={createNote} title="New note">
            <span>＋</span><Show when={!collapsed()}>New note</Show>
          </button>
        </div>

        <nav class="note-list" aria-label="Notes">
          <For each={filtered()}>
            {(note) => (
              <button
                class="note-row"
                classList={{ active: active()?.path === note.path }}
                onClick={() => void open(note)}
                title={note.title}
              >
                <Show when={collapsed()} fallback={<><strong>{note.title}</strong><span>MARKDOWN</span></>}>
                  <strong>{note.title.at(0)?.toLocaleUpperCase() ?? "N"}</strong>
                </Show>
              </button>
            )}
          </For>
          <Show when={!collapsed() && notes().length === 0}>
            <div class="sidebar-empty">No notes yet.</div>
          </Show>
        </nav>

        <Show when={!collapsed()}>
          <footer class="sidebar-footer"><span>LOCAL MARKDOWN</span>Portable files, always yours.</footer>
        </Show>
      </aside>

      <section class="workspace">
        <Show
          when={active()}
          fallback={
            <div class="welcome-page">
              <div class="welcome-heading">
                <div class="empty-mark"><img src={noteIcon} alt="Umbra Note" /></div>
                <div>
                  <h1>Welcome to Umbra Note</h1>
                  <p>A quiet place for portable Markdown.</p>
                </div>
              </div>
              <div class="welcome-actions">
                <button class="primary" onClick={createNote}>＋ New note</button>
                <button class="quiet" onClick={importNote}>⇧ Import Markdown</button>
              </div>
              <section class="recent-panel" aria-labelledby="recent-heading">
                <header>
                  <h2 id="recent-heading">Recent notes</h2>
                  <span>{notes().length}</span>
                </header>
                <div class="recent-list">
                  <For each={notes().slice(0, 12)}>
                    {(recent) => (
                      <button onClick={() => void open(recent)}>
                        <span class="recent-icon">MD</span>
                        <span class="recent-copy">
                          <strong>{recent.title}</strong>
                          <small>{recent.path}</small>
                        </span>
                        <time>{new Date(recent.modifiedMs).toLocaleDateString()}</time>
                      </button>
                    )}
                  </For>
                  <Show when={notes().length === 0}>
                    <div class="recent-empty">Your recently opened notes will appear here.</div>
                  </Show>
                </div>
              </section>
              <Show when={error()}><div class="error-message">{error()}</div></Show>
            </div>
          }
        >
          {(note) => (
            <>
              <header class="document-header">
                <div class="title-stack">
                  <input
                    class="title-input"
                    value={title()}
                    onInput={(event) => setTitle(event.currentTarget.value)}
                    onKeyDown={(event) => event.key === "Enter" && void rename()}
                    aria-label="Note title"
                  />
                  <span class="save-state" classList={{ dirty: dirty() }}>
                    {dirty() ? "●  UNSAVED" : "✓  SAVED"}
                  </span>
                </div>
                <button class="quiet" onClick={rename}>Rename</button>
                <button class="primary" onClick={() => void save()}>Save <kbd>Ctrl S</kbd></button>
              </header>

              <div class="toolbar" role="toolbar" aria-label="Markdown formatting">
                <For each={tools}>
                  {(tool) => (
                    <button title={tool.title} onClick={() => editor?.format(tool.action)}>
                      {tool.label}
                    </button>
                  )}
                </For>
                <span class="live-badge"><i /> LIVE PREVIEW</span>
              </div>

              <MarkdownEditor
                path={note().path}
                content={note().content}
                onChange={changed}
                onSave={() => void save()}
                onReady={(controller) => (editor = controller)}
              />

              <footer class="status-bar">
                <span>{draft().trim() ? `${draft().trim().split(/\s+/).length} words · ${draft().split("\n").length} lines` : "Empty note"}</span>
                <span classList={{ error: Boolean(error()) }}>{error() || status()}</span>
              </footer>
            </>
          )}
        </Show>
      </section>
    </main>
  );
}
