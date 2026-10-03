mod theme;

use floem::{
    Application,
    event::{Event, EventListener},
    keyboard::{Key, Modifiers},
    kurbo::Size,
    peniko::Color,
    prelude::*,
    reactive::{RwSignal, SignalGet, SignalUpdate, SignalWith, create_memo},
    style::Style,
    text::{
        Attrs, AttrsList, FamilyOwned, LineHeightValue, Style as TextStyle, TextLayout, Weight,
    },
    views::{
        PlaceholderTextClass,
        editor::{
            core::{cursor::CursorMode, editor::EditType, selection::Selection},
            text::{SimpleStyling, WrapMethod},
        },
        rich_text, text_editor,
    },
    window::WindowConfig,
};
use std::{cell::RefCell, rc::Rc};
use theme::*;
use umbra_note::editor::{MarkdownAction, markdown_edit};
use umbra_note::{
    app::AppState,
    markdown::{RenderStyle, render},
    search,
};

const OS_MOD: Modifiers = Modifiers::CONTROL;

#[derive(Clone)]
struct EditorHandle {
    editor: floem::views::editor::Editor,
    doc: Rc<dyn floem::views::editor::text::Document>,
}

pub fn run(initial: AppState) {
    let app = Application::new();
    let config = WindowConfig::default()
        .title("Umbra Note")
        .size(Size::new(1220.0, 780.0));
    app.window(move |_| app_view(initial), Some(config)).run();
}

fn app_view(initial: AppState) -> impl IntoView {
    let state = RwSignal::new(initial);
    let filter = RwSignal::new(String::new());
    let title = RwSignal::new(state.with(|s| {
        s.active
            .as_ref()
            .map(|d| d.title.clone())
            .unwrap_or_default()
    }));
    let collapsed = RwSignal::new(false);
    let status = RwSignal::new(("Ready".to_string(), false));
    let editor_dirty = RwSignal::new(false);
    let handle: Rc<RefCell<Option<EditorHandle>>> = Rc::new(RefCell::new(None));
    // The editor must only be rebuilt when the active document changes. A
    // Memo filters ordinary body/dirty-state updates that happen on every key.
    let active_generation = create_memo(move |_| state.with(|s| s.generation));

    let sidebar = sidebar(
        state,
        filter,
        title,
        collapsed,
        status,
        handle.clone(),
        editor_dirty,
    )
    .style(move |s| {
        let width = if collapsed.get() { 64.0 } else { 240.0 };
        s.width(width)
            .min_width(width)
            .max_width(width)
            .height_full()
    });

    let content_handle = handle.clone();
    let content = dyn_container(
        move || active_generation.get(),
        move |_| match state.with_untracked(|s| s.active.clone()) {
            Some(doc) => editor_view(
                doc,
                state,
                title,
                status,
                content_handle.clone(),
                editor_dirty,
            )
            .into_any(),
            None => {
                empty_state(state, title, status, content_handle.clone(), editor_dirty).into_any()
            }
        },
    )
    .style(|s| s.flex_grow(1.0_f32).height_full().min_width(0.0));

    (sidebar, content)
        .h_stack()
        .style(|s| {
            s.size_full()
                .background(CANVAS)
                .color(TEXT)
                .font_family("Inter".to_string())
        })
        .on_key_down(Key::Character("s".into()), |m| m == OS_MOD, {
            let handle = handle.clone();
            move |_| save(state, title, status, &handle, editor_dirty)
        })
        .on_key_down(Key::Character("n".into()), |m| m == OS_MOD, {
            let handle = handle.clone();
            move |_| create_note(state, title, status, &handle, editor_dirty)
        })
        .on_key_down(Key::Character("b".into()), |m| m == OS_MOD, {
            let handle = handle.clone();
            move |_| format_selection(&handle, MarkdownAction::Bold)
        })
        .on_key_down(Key::Character("i".into()), |m| m == OS_MOD, {
            let handle = handle.clone();
            move |_| format_selection(&handle, MarkdownAction::Italic)
        })
        .on_event_stop(EventListener::WindowClosed, {
            let handle = handle.clone();
            move |_| {
                let _ = persist_current(state, title, &handle);
            }
        })
        .window_title(move || {
            state.with(|s| match &s.active {
                Some(doc) => format!(
                    "{}{} — Umbra Note",
                    if editor_dirty.get() || doc.is_dirty() {
                        "• "
                    } else {
                        ""
                    },
                    doc.title
                ),
                None => "Umbra Note".into(),
            })
        })
}

fn sidebar(
    state: RwSignal<AppState>,
    filter: RwSignal<String>,
    title: RwSignal<String>,
    collapsed: RwSignal<bool>,
    status: RwSignal<(String, bool)>,
    handle: Rc<RefCell<Option<EditorHandle>>>,
    editor_dirty: RwSignal<bool>,
) -> impl IntoView {
    let brand = (
        container("U".style(|s| {
            s.font_size(18.0)
                .font_weight(Weight::BOLD)
                .color(Color::WHITE)
        }))
        .style(|s| {
            s.size(38.0, 38.0)
                .items_center()
                .justify_center()
                .background(ACCENT)
                .border_radius(12.0)
        }),
        dyn_container(
            move || collapsed.get(),
            |small| {
                if small {
                    empty().into_any()
                } else {
                    (
                        "UMBRA".style(|s| s.font_size(11.0).font_weight(Weight::BOLD)),
                        "NOTE".style(|s| s.font_size(11.0).color(ACCENT_SOFT)),
                    )
                        .v_stack()
                        .style(|s| s.gap(0.0))
                        .into_any()
                }
            },
        )
        .style(|s| s.flex_grow(1.0_f32).min_width(0.0)),
        button(label(move || if collapsed.get() { "›" } else { "‹" }))
            .action(move || collapsed.update(|value| *value = !*value))
            .style(icon_button),
    )
        .h_stack()
        .style(|s| {
            s.width_full()
                .items_center()
                .gap(10.0)
                .padding_horiz(14.0)
                .height(76.0)
        });

    let controls_handle = handle.clone();
    let controls = dyn_container(
        move || collapsed.get(),
        move |small| {
            if small {
                let handle = controls_handle.clone();
                button("+")
                    .on_event_stop(EventListener::PointerDown, move |event| {
                        if matches!(event, Event::PointerDown(pointer) if pointer.button.is_primary()) {
                            create_note(state, title, status, &handle, editor_dirty);
                        }
                    })
                    .style(primary_icon)
                    .into_any()
            } else {
                let handle = controls_handle.clone();
                let search_box = text_input(filter)
                    .placeholder("Search notes…")
                    .keyboard_navigable()
                    .style(|s| {
                        s.width_full()
                            .height(40.0)
                            .padding_horiz(12.0)
                            .background(RAISED)
                            .border(1.0)
                            .border_color(BORDER)
                            .border_radius(11.0)
                            .color(TEXT)
                            .focus(|s| s.border_color(ACCENT))
                            .class(PlaceholderTextClass, |s| s.color(DIM))
                    });
                (
                    search_box,
                    button("＋  New note")
                        .on_event_stop(EventListener::PointerDown, move |event| {
                            if matches!(event, Event::PointerDown(pointer) if pointer.button.is_primary()) {
                                create_note(state, title, status, &handle, editor_dirty);
                            }
                        })
                        .style(|s| primary_button(s).width_full()),
                )
                    .v_stack()
                    .style(|s| {
                        s.width_full()
                            .gap(10.0)
                            .padding_horiz(12.0)
                            .padding_bottom(18.0)
                    })
                    .into_any()
            }
        },
    )
    .style(|s| s.width_full());

    let list = dyn_stack(
        move || {
            state.with(|s| {
                search::titles(&s.notes, &filter.get())
                    .into_iter()
                    .cloned()
                    .collect::<Vec<_>>()
            })
        },
        |note| note.path.clone(),
        move |note| {
            note_row(
                note,
                state,
                title,
                collapsed,
                status,
                handle.clone(),
                editor_dirty,
            )
        },
    )
    .style(|s| s.width_full().flex_col().gap(4.0).padding_horiz(8.0));

    (
        brand,
        controls,
        scroll(list).style(|s| s.width_full().flex_grow(1.0_f32).min_height(0.0)),
        dyn_container(
            move || collapsed.get(),
            |small| {
                if small {
                    empty().into_any()
                } else {
                    (
                        "LOCAL MARKDOWN".style(|s| s.font_size(10.0).color(DIM)),
                        "Portable files, always yours.".style(|s| s.font_size(11.0).color(MUTED)),
                    )
                        .v_stack()
                        .style(|s| s.width_full().gap(3.0).padding(16.0))
                        .into_any()
                }
            },
        )
        .style(|s| s.width_full()),
    )
        .v_stack()
        .style(|s| {
            s.size_full()
                .background(SIDEBAR)
                .border_right(1.0)
                .border_color(BORDER)
        })
}

fn note_row(
    note: umbra_note::storage::NoteSummary,
    state: RwSignal<AppState>,
    title: RwSignal<String>,
    collapsed: RwSignal<bool>,
    status: RwSignal<(String, bool)>,
    handle: Rc<RefCell<Option<EditorHandle>>>,
    editor_dirty: RwSignal<bool>,
) -> impl IntoView {
    let path = note.path.clone();
    let path_for_active = path.clone();
    let initial = note.title.chars().next().unwrap_or('N').to_string();
    let label = dyn_container(
        move || collapsed.get(),
        move |small| {
            if small {
                initial
                    .clone()
                    .style(|s| s.font_weight(Weight::BOLD).color(ACCENT_SOFT))
                    .into_any()
            } else {
                (
                    note.title
                        .clone()
                        .style(|s| s.font_size(13.0).font_weight(Weight::SEMIBOLD)),
                    "MARKDOWN".style(|s| s.font_size(9.0).color(DIM)),
                )
                    .v_stack()
                    .style(|s| s.gap(3.0).items_start())
                    .into_any()
            }
        },
    );
    container(label)
        .style(move |s| {
            let active = state.with(|state| {
                state
                    .active
                    .as_ref()
                    .is_some_and(|doc| doc.path == path_for_active)
            });
            s.width_full()
                .min_height(54.0)
                .padding_horiz(12.0)
                .items_center()
                .border_radius(10.0)
                .apply_if(active, |s| {
                    s.background(RAISED).border_left(2.0).border_color(ACCENT)
                })
                .hover(|s| s.background(RAISED))
        })
        .on_click_stop(move |_| {
            let body = current_editor_body(&handle);
            let requested_title = title.get_untracked();
            let result = state.try_update(|app| {
                apply_current_edits(app, body, &requested_title)?;
                app.open(path.clone())
            });
            match result {
                Some(Ok(())) => {
                    title.set(state.with(|s| {
                        s.active
                            .as_ref()
                            .map(|d| d.title.clone())
                            .unwrap_or_default()
                    }));
                    editor_dirty.set(false);
                    status.set(("Opened".into(), false));
                }
                Some(Err(error)) => status.set((error.to_string(), true)),
                None => {}
            }
        })
}

fn editor_view(
    doc: umbra_note::editor::Document,
    state: RwSignal<AppState>,
    title: RwSignal<String>,
    status: RwSignal<(String, bool)>,
    slot: Rc<RefCell<Option<EditorHandle>>>,
    editor_dirty: RwSignal<bool>,
) -> impl IntoView {
    title.set(doc.title.clone());
    let preview = RwSignal::new(false);
    let preview_source = RwSignal::new(doc.body());
    let editor = text_editor(doc.body())
        .styling(
            SimpleStyling::builder()
                .font_size(16)
                .line_height(1.65)
                .font_family(vec![
                    FamilyOwned::Name("Inter".into()),
                    FamilyOwned::SansSerif,
                ])
                .build(),
        )
        .editor_style(|s| {
            s.hide_gutter(true)
                .wrap_method(WrapMethod::WrapWidth { width: 820.0 })
                .selection_color(Color::rgba8(139, 92, 246, 80))
                .cursor_color(ACCENT_SOFT)
                .current_line_color(Color::rgba8(139, 92, 246, 10))
        })
        .placeholder("Begin writing in Markdown…")
        .style(|s| {
            s.size_full()
                .background(CANVAS)
                .color(TEXT)
                .padding_horiz(28.0)
                .padding_vert(18.0)
        });
    let floem_doc = editor.doc();
    let floem_editor = editor.editor().clone();
    *slot.borrow_mut() = Some(EditorHandle {
        editor: floem_editor,
        doc: floem_doc.clone(),
    });
    let editor = editor
        .update(move |_| {
            if !editor_dirty.get_untracked() {
                editor_dirty.set(true);
                status.set(("Unsaved changes".into(), false));
            }
        })
        .style(move |s| {
            s.flex_grow(1.0_f32)
                .min_height(0.0)
                .apply_if(preview.get(), |s| s.hide())
        });

    let rendered_preview = markdown_preview(preview_source).style(move |s| {
        s.flex_grow(1.0_f32)
            .min_height(0.0)
            .apply_if(!preview.get(), |s| s.hide())
    });

    let toolbar_slot = slot.clone();
    let toolbar = (
        tool("B", "Bold", {
            let h = toolbar_slot.clone();
            move || format_selection(&h, MarkdownAction::Bold)
        }),
        tool("I", "Italic", {
            let h = toolbar_slot.clone();
            move || format_selection(&h, MarkdownAction::Italic)
        }),
        divider(),
        tool("H", "Heading", {
            let h = toolbar_slot.clone();
            move || format_selection(&h, MarkdownAction::Heading(2))
        }),
        tool("•", "List", {
            let h = toolbar_slot.clone();
            move || format_selection(&h, MarkdownAction::UnorderedList)
        }),
        tool("❯", "Quote", {
            let h = toolbar_slot.clone();
            move || format_selection(&h, MarkdownAction::Blockquote)
        }),
        tool("↗", "Link", {
            let h = toolbar_slot.clone();
            move || format_selection(&h, MarkdownAction::Link)
        }),
        tool("`", "Inline code", {
            let h = toolbar_slot.clone();
            move || format_selection(&h, MarkdownAction::InlineCode)
        }),
        tool("</>", "Code block", move || {
            format_selection(&toolbar_slot, MarkdownAction::FencedCode)
        }),
    )
        .h_stack()
        .style(|s| s.gap(5.0).items_center());

    let title_for_rename = title;
    let preview_handle = slot.clone();
    let header = (
        (
            text_input(title).keyboard_navigable().style(|s| {
                s.width_full()
                    .font_size(25.0)
                    .font_weight(Weight::BOLD)
                    .background(Color::TRANSPARENT)
                    .border(0.0)
                    .color(TEXT)
                    .padding(0.0)
            }),
            label(move || {
                state
                    .with(|s| {
                        if editor_dirty.get() || s.active.as_ref().is_some_and(|d| d.is_dirty()) {
                            "●  UNSAVED"
                        } else {
                            "✓  SAVED"
                        }
                    })
                    .to_string()
            })
            .style(move |s| {
                s.font_size(10.0).color(
                    if editor_dirty.get()
                        || state.with(|x| x.active.as_ref().is_some_and(|d| d.is_dirty()))
                    {
                        ACCENT_SOFT
                    } else {
                        SUCCESS
                    },
                )
            }),
        )
            .v_stack()
            .style(|s| s.gap(7.0).flex_grow(1.0_f32)),
        button(label(
            move || if preview.get() { "Edit" } else { "Preview" },
        ))
        .action(move || {
            if !preview.get_untracked()
                && let Some(body) = current_editor_body(&preview_handle)
            {
                preview_source.set(body);
            }
            preview.update(|value| *value = !*value);
        })
        .style(quiet_button),
        button("Rename")
            .action({
                let handle = slot.clone();
                move || rename(state, title_for_rename, status, &handle, editor_dirty)
            })
            .style(quiet_button),
        button("Save  Ctrl S")
            .action({
                let handle = slot.clone();
                move || save(state, title, status, &handle, editor_dirty)
            })
            .style(primary_button),
    )
        .h_stack()
        .style(|s| {
            s.items_center()
                .gap(12.0)
                .padding_horiz(34.0)
                .padding_top(26.0)
                .padding_bottom(18.0)
        });

    let status_line = label(move || {
        let (message, _) = status.get();
        let stats = if editor_dirty.get() {
            "Editing".to_string()
        } else {
            state.with(|s| {
                s.active
                    .as_ref()
                    .map(|d| {
                        let body = d.body();
                        format!(
                            "{} words  ·  {} lines",
                            body.split_whitespace().count(),
                            body.lines().count()
                        )
                    })
                    .unwrap_or_default()
            })
        };
        format!("{stats}                                      {message}")
    })
    .style(move |s| {
        s.font_size(11.0)
            .color(if status.get().1 { ERROR } else { MUTED })
            .padding_horiz(32.0)
            .height(34.0)
    });

    (
        header,
        container(toolbar).style(move |s| {
            s.height(51.0)
                .padding_horiz(30.0)
                .items_center()
                .background(SURFACE)
                .border_top(1.0)
                .border_bottom(1.0)
                .border_color(BORDER)
                .apply_if(preview.get(), |s| s.hide())
        }),
        (editor, rendered_preview)
            .v_stack()
            .style(|s| s.flex_grow(1.0_f32).min_height(0.0)),
        status_line,
    )
        .v_stack()
        .style(|s| s.size_full().background(CANVAS))
}

fn markdown_preview(source: RwSignal<String>) -> impl IntoView {
    let preview = rich_text(move || markdown_layout(&source.get()));
    scroll(container(preview).style(|s| {
        s.width_full()
            .max_width(920.0)
            .padding_horiz(48.0)
            .padding_vert(34.0)
    }))
    .style(|s| {
        s.size_full()
            .background(CANVAS)
            .justify_center()
            .min_height(0.0)
    })
}

fn markdown_layout(source: &str) -> TextLayout {
    let rendered = render(source);
    let sans = [FamilyOwned::Name("Inter".into()), FamilyOwned::SansSerif];
    let mono = [
        FamilyOwned::Name("monospace".into()),
        FamilyOwned::Monospace,
    ];
    let base = Attrs::new()
        .color(TEXT)
        .family(&sans)
        .font_size(16.0)
        .line_height(LineHeightValue::Normal(1.65));
    let mut attrs = AttrsList::new(base);

    // Inline styling first and block styling second lets heading and block
    // typography establish the final size while preserving portable source.
    for span in rendered.spans.iter().filter(|span| {
        !matches!(
            span.style,
            RenderStyle::Heading(_)
                | RenderStyle::CodeBlock
                | RenderStyle::Quote
                | RenderStyle::Table
        )
    }) {
        let style = match span.style {
            RenderStyle::Strong => base.weight(Weight::BOLD),
            RenderStyle::Emphasis => base.style(TextStyle::Italic),
            RenderStyle::Code => base.family(&mono).color(ACCENT_SOFT),
            RenderStyle::Link | RenderStyle::Image => base.color(ACCENT_SOFT),
            _ => continue,
        };
        attrs.add_span(span.range.clone(), style);
    }
    for span in &rendered.spans {
        let style = match span.style {
            RenderStyle::Heading(level) => base
                .font_size(match level {
                    1 => 30.0,
                    2 => 25.0,
                    3 => 21.0,
                    _ => 18.0,
                })
                .weight(Weight::BOLD)
                .color(Color::WHITE),
            RenderStyle::CodeBlock => base.family(&mono).color(Color::rgb8(205, 190, 235)),
            RenderStyle::Quote => base.style(TextStyle::Italic).color(ACCENT_SOFT),
            RenderStyle::Table => base.family(&mono).color(Color::rgb8(205, 198, 220)),
            _ => continue,
        };
        attrs.add_span(span.range.clone(), style);
    }

    let mut layout = TextLayout::new();
    layout.set_text(&rendered.text, attrs);
    layout
}

fn empty_state(
    state: RwSignal<AppState>,
    title: RwSignal<String>,
    status: RwSignal<(String, bool)>,
    handle: Rc<RefCell<Option<EditorHandle>>>,
    editor_dirty: RwSignal<bool>,
) -> impl IntoView {
    container(
        (
            container("U".style(|s| s.font_size(30.0).font_weight(Weight::BOLD))).style(|s| {
                s.size(70.0, 70.0)
                    .items_center()
                    .justify_center()
                    .background(RAISED)
                    .border(1.0)
                    .border_color(ACCENT)
                    .border_radius(22.0)
            }),
            "A quiet place for clear thought."
                .style(|s| s.font_size(28.0).font_weight(Weight::BOLD)),
            "Markdown notes, stored locally and ready whenever inspiration arrives."
                .style(|s| s.font_size(14.0).color(MUTED)),
            button("＋  Create your first note")
                .on_event_stop(EventListener::PointerDown, move |event| {
                    if matches!(event, Event::PointerDown(pointer) if pointer.button.is_primary()) {
                        create_note(state, title, status, &handle, editor_dirty);
                    }
                })
                .style(primary_button),
            label(move || {
                let (message, is_error) = status.get();
                if is_error { message } else { String::new() }
            })
            .style(|s| s.font_size(12.0).color(ERROR)),
        )
            .v_stack()
            .style(|s| s.items_center().gap(18.0)),
    )
    .style(|s| {
        s.size_full()
            .items_center()
            .justify_center()
            .background(CANVAS)
    })
}

fn tool(label: &'static str, _tip: &'static str, action: impl Fn() + 'static) -> impl IntoView {
    button(label).action(action).style(|s| {
        s.height(32.0)
            .min_width(34.0)
            .padding_horiz(9.0)
            .background(Color::TRANSPARENT)
            .border_radius(8.0)
            .color(MUTED)
            .hover(|s| s.background(RAISED).color(TEXT))
    })
}

fn divider() -> impl IntoView {
    empty().style(|s| {
        s.width(1.0)
            .height(20.0)
            .background(BORDER)
            .margin_horiz(4.0)
    })
}

fn icon_button(s: Style) -> Style {
    s.size(32.0, 32.0)
        .border_radius(8.0)
        .background(Color::TRANSPARENT)
        .color(MUTED)
        .hover(|s| s.background(RAISED).color(TEXT))
}
fn primary_icon(s: Style) -> Style {
    primary_button(s).size(42.0, 42.0).margin_horiz(14.0)
}
fn primary_button(s: Style) -> Style {
    s.height(38.0)
        .padding_horiz(15.0)
        .border_radius(10.0)
        .background(ACCENT)
        .color(Color::WHITE)
        .font_weight(Weight::SEMIBOLD)
        .hover(|s| s.background(Color::rgb8(155, 112, 250)))
}
fn quiet_button(s: Style) -> Style {
    s.height(38.0)
        .padding_horiz(14.0)
        .border_radius(10.0)
        .background(RAISED)
        .border(1.0)
        .border_color(BORDER)
        .color(TEXT)
        .hover(|s| s.border_color(ACCENT))
}

fn create_note(
    state: RwSignal<AppState>,
    title: RwSignal<String>,
    status: RwSignal<(String, bool)>,
    handle: &Rc<RefCell<Option<EditorHandle>>>,
    editor_dirty: RwSignal<bool>,
) {
    let body = current_editor_body(handle);
    let requested_title = title.get_untracked();
    match state.try_update(|app| {
        apply_current_edits(app, body, &requested_title)?;
        app.create()
    }) {
        Some(Ok(())) => {
            title.set("Untitled note".into());
            editor_dirty.set(false);
            status.set(("New note created".into(), false));
        }
        Some(Err(error)) => status.set((error.to_string(), true)),
        None => status.set(("Application state is unavailable".into(), true)),
    }
}

fn save(
    state: RwSignal<AppState>,
    title: RwSignal<String>,
    status: RwSignal<(String, bool)>,
    handle: &Rc<RefCell<Option<EditorHandle>>>,
    editor_dirty: RwSignal<bool>,
) {
    let result = persist_current(state, title, handle);
    match result {
        Some(Ok(())) => {
            editor_dirty.set(false);
            status.set(("Saved to disk".into(), false));
        }
        Some(Err(error)) => status.set((error.to_string(), true)),
        None => status.set(("Application state is unavailable".into(), true)),
    }
}

fn rename(
    state: RwSignal<AppState>,
    title: RwSignal<String>,
    status: RwSignal<(String, bool)>,
    handle: &Rc<RefCell<Option<EditorHandle>>>,
    editor_dirty: RwSignal<bool>,
) {
    match persist_current(state, title, handle) {
        Some(Ok(())) => {
            editor_dirty.set(false);
            status.set(("Renamed and saved".into(), false));
        }
        Some(Err(error)) => status.set((error.to_string(), true)),
        None => status.set(("Application state is unavailable".into(), true)),
    }
}

fn current_editor_body(handle: &Rc<RefCell<Option<EditorHandle>>>) -> Option<String> {
    handle
        .borrow()
        .as_ref()
        .map(|handle| handle.doc.text().to_string())
}

fn apply_current_edits(
    app: &mut AppState,
    body: Option<String>,
    requested_title: &str,
) -> anyhow::Result<()> {
    if let (Some(active), Some(body)) = (&mut app.active, body) {
        active.set_body(body);
    }
    let should_rename = app
        .active
        .as_ref()
        .is_some_and(|active| active.title != requested_title.trim());
    if should_rename {
        app.rename(requested_title.to_string())?;
    }
    Ok(())
}

fn persist_current(
    state: RwSignal<AppState>,
    title: RwSignal<String>,
    handle: &Rc<RefCell<Option<EditorHandle>>>,
) -> Option<anyhow::Result<()>> {
    let body = current_editor_body(handle);
    let requested_title = title.get_untracked();
    state.try_update(|app| {
        apply_current_edits(app, body, &requested_title)?;
        app.save()
    })
}

fn format_selection(slot: &Rc<RefCell<Option<EditorHandle>>>, action: MarkdownAction) {
    let Some(handle) = slot.borrow().clone() else {
        return;
    };
    let cursor = handle.editor.cursor.get_untracked();
    let (start, end) = match cursor.mode {
        CursorMode::Insert(selection) => selection
            .regions()
            .last()
            .map(|r| (r.min(), r.max()))
            .unwrap_or((0, 0)),
        CursorMode::Normal(offset) => (offset, offset),
        CursorMode::Visual { start, end, .. } => (start.min(end), start.max(end)),
    };
    let source = handle.doc.text().to_string();
    let Ok(formatted) = markdown_edit(&source, start..end, action) else {
        return;
    };
    let selection = formatted.selection.clone();
    handle.doc.edit_single(
        Selection::region(formatted.start, formatted.end),
        &formatted.replacement,
        EditType::InsertChars,
    );
    handle.editor.cursor.update(|cursor| {
        cursor.mode = CursorMode::Insert(Selection::region(selection.start, selection.end))
    });
}
