//! Static GPUI transcript harness for iterating on cross-message text selection.

#[path = "notepad_selection/transcript.rs"]
mod transcript;

use gpui::{
    div, px, size, App, AppContext, Application, Bounds, ClipboardItem, Context, FocusHandle, Hsla,
    InteractiveElement, IntoElement, ParentElement, Render, ScrollHandle, SharedString,
    StatefulInteractiveElement, Styled, Window, WindowBounds, WindowKind, WindowOptions,
};
use gpui_selection_vendor::{GlobalState, SelectableText, TextSelection, TextSelectionLayer};
use personal_agent::ui_gpui::{
    components::markdown_content::{
        blocks_to_elements_with_leaf_factory, parse_markdown_blocks, MarkdownLeaf,
        MarkdownLeafFactory,
    },
    theme::{active_theme_slug, is_valid_theme_slug, set_active_theme_slug, Theme},
    theme_catalog::ThemeCatalog,
};
use transcript::{transcript, Role};

const GREEN_SCREEN_SLUG: &str = "green-screen";
const WINDOW_WIDTH: f32 = 760.0;
const WINDOW_HEIGHT: f32 = 900.0;
const SELF_CHECK_ONLY_ARG: &str = "--theme-self-check";

struct ResolvedPalette {
    user_bubble_bg: String,
    user_bubble_text: String,
    text_primary: String,
    selection_bg: String,
    selection_fg: String,
    assistant_bubble_bg: String,
}

impl ResolvedPalette {
    fn print(&self) {
        println!(
            "theme-self-check active_theme_slug={} catalog=embedded \
             user_bubble_bg={} user_bubble_text={} text_primary={} \
             selection_bg={} selection_fg={} assistant_bubble_bg={}",
            active_theme_slug(),
            self.user_bubble_bg,
            self.user_bubble_text,
            self.text_primary,
            self.selection_bg,
            self.selection_fg,
            self.assistant_bubble_bg,
        );
    }

    fn verify_green_screen(&self) -> Result<(), String> {
        let expected = [
            ("user_bubble_bg", self.user_bubble_bg.as_str(), "#6a9955"),
            (
                "user_bubble_text",
                self.user_bubble_text.as_str(),
                "#000000",
            ),
            ("text_primary", self.text_primary.as_str(), "#6a9955"),
            ("selection_bg", self.selection_bg.as_str(), "#6a9955"),
            ("selection_fg", self.selection_fg.as_str(), "#000000"),
            (
                "assistant_bubble_bg",
                self.assistant_bubble_bg.as_str(),
                "#000000",
            ),
        ];

        for (name, actual, required) in expected {
            if actual != required {
                return Err(format!(
                    "Green Screen {name} resolved to {actual}, expected {required}"
                ));
            }
        }
        Ok(())
    }
}

struct NotepadSelection {
    chat_scroll_handle: ScrollHandle,
    focus_handle: FocusHandle,
}

impl NotepadSelection {
    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            chat_scroll_handle: ScrollHandle::new(),
            focus_handle: cx.focus_handle(),
        }
    }

    fn handle_key_down(event: &gpui::KeyDownEvent, window: &mut Window, cx: &mut App) {
        let modifiers = &event.keystroke.modifiers;
        let copy_modifier = if cfg!(target_os = "macos") {
            modifiers.platform
        } else {
            modifiers.control
        };
        if copy_modifier && event.keystroke.key.eq_ignore_ascii_case("c") {
            let selected = TextSelection::selected_text(window, cx);
            if !selected.is_empty() {
                cx.write_to_clipboard(ClipboardItem::new_string(selected));
            }
        }
    }

    fn render_title_bar() -> impl IntoElement {
        div()
            .flex_none()
            .w_full()
            .flex()
            .items_center()
            .justify_between()
            .px(px(Theme::SPACING_MD))
            .py(px(Theme::SPACING_SM))
            .bg(Theme::bg_dark())
            .border_b_1()
            .border_color(Theme::border())
            .text_color(Theme::text_primary())
            .text_size(px(Theme::font_size_ui()))
            .child("notepad — selection prototype (static transcript)")
            .child(
                div()
                    .px(px(Theme::SPACING_SM))
                    .py(px(Theme::SPACING_XS))
                    .rounded(px(Theme::RADIUS_SM))
                    .border_1()
                    .border_color(Theme::border())
                    .text_size(px(Theme::font_size_small()))
                    .child(format!("theme: {}", active_theme_slug())),
            )
    }

    fn render_user_message(
        markdown: &str,
        factory: &mut dyn MarkdownLeafFactory,
        document_order: &mut u64,
        first_separator: &str,
    ) -> gpui::AnyElement {
        let blocks = parse_markdown_blocks(markdown);
        let rendered = blocks_to_elements_with_leaf_factory(
            &blocks,
            Theme::user_bubble_text(),
            Theme::user_bubble_bg(),
            factory,
            document_order,
            first_separator,
        );
        let bubble = div()
            .max_w(px(300.0))
            .px(px(10.0))
            .py(px(10.0))
            .rounded(px(12.0))
            .text_size(px(Theme::font_size_mono()))
            .children(rendered);

        div()
            .w_full()
            .flex()
            .justify_end()
            .child(Theme::user_bubble(bubble))
            .into_any_element()
    }

    fn render_assistant_message(
        markdown: &str,
        factory: &mut dyn MarkdownLeafFactory,
        document_order: &mut u64,
        first_separator: &str,
    ) -> gpui::AnyElement {
        let blocks = parse_markdown_blocks(markdown);
        let rendered = blocks_to_elements_with_leaf_factory(
            &blocks,
            Theme::text_primary(),
            Theme::assistant_bubble_bg(),
            factory,
            document_order,
            first_separator,
        );
        div()
            .flex()
            .flex_col()
            .items_start()
            .w_full()
            .gap(px(Theme::SPACING_SM))
            .child(Theme::assistant_bubble(
                div()
                    .w_full()
                    .px(px(Theme::SPACING_MD))
                    .py(px(Theme::SPACING_SM))
                    .rounded(px(Theme::RADIUS_LG))
                    .children(rendered),
            ))
            .child(
                div()
                    .text_sm()
                    .text_color(Theme::text_muted())
                    .child("via gpt-5-codex"),
            )
            .into_any_element()
    }

    fn render_message(
        role: Role,
        markdown: &str,
        factory: &mut dyn MarkdownLeafFactory,
        document_order: &mut u64,
        first_separator: &str,
    ) -> gpui::AnyElement {
        match role {
            Role::User => {
                Self::render_user_message(markdown, factory, document_order, first_separator)
            }
            Role::Assistant => {
                Self::render_assistant_message(markdown, factory, document_order, first_separator)
            }
        }
    }

    fn render_chat_area(&self) -> impl IntoElement {
        let mut factory = SelectionLeafFactory {
            scroll_offset: self.chat_scroll_handle.offset(),
        };
        let mut document_order = 0;
        let mut messages = Vec::new();
        for (index, (role, markdown)) in transcript().into_iter().enumerate() {
            let separator = if index == 0 { "" } else { "\n\n" };
            messages.push(
                div()
                    .id(SharedString::from(format!("msg-{index}")))
                    .w_full()
                    .flex()
                    .justify_start()
                    .child(Self::render_message(
                        role,
                        markdown,
                        &mut factory,
                        &mut document_order,
                        separator,
                    )),
            );
        }

        div()
            .id("chat-area")
            .flex_1()
            .min_h_0()
            .w_full()
            .bg(Theme::bg_base())
            .overflow_x_hidden()
            .overflow_y_scroll()
            .track_scroll(&self.chat_scroll_handle)
            .p(px(Theme::SPACING_MD))
            .flex()
            .flex_col()
            .items_stretch()
            .justify_start()
            .gap(px(Theme::SPACING_SM))
            .children(messages)
    }
}

impl Render for NotepadSelection {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(Theme::bg_base())
            .text_color(Theme::text_primary())
            .font_family(Theme::mono_font_family())
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|_, event, window, cx| {
                Self::handle_key_down(event, window, cx);
            }))
            .child(TextSelectionLayer)
            .child(Self::render_title_bar())
            .child(self.render_chat_area())
    }
}

struct SelectionLeafFactory {
    scroll_offset: gpui::Point<gpui::Pixels>,
}

impl MarkdownLeafFactory for SelectionLeafFactory {
    fn create_leaf(&mut self, leaf: MarkdownLeaf) -> gpui::AnyElement {
        let id = SharedString::from(format!("selection-leaf-{}", leaf.document_order));
        SelectableText::new(
            id,
            leaf.plain_text,
            leaf.text_runs,
            leaf.surface_background,
            leaf.surface_foreground,
        )
        .document_order(leaf.document_order)
        .scroll_offset(self.scroll_offset)
        .copy_separator_before(leaf.copy_separator_before)
        .into_any_element()
    }
}

fn color_hex(color: Hsla) -> String {
    let rgba = color.to_rgb();
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let components = (
        (rgba.r * 255.0).round() as u8,
        (rgba.g * 255.0).round() as u8,
        (rgba.b * 255.0).round() as u8,
    );
    format!(
        "#{:02x}{:02x}{:02x}",
        components.0, components.1, components.2
    )
}

fn activate_and_verify_theme() -> Result<ResolvedPalette, String> {
    let catalog = ThemeCatalog::load_bundled()
        .map_err(|error| format!("failed to load embedded theme catalog: {error}"))?;
    let green_screen = catalog
        .get(GREEN_SCREEN_SLUG)
        .ok_or_else(|| "embedded theme catalog has no green-screen theme".to_string())?;
    if green_screen.colors.message.user_border != "#6a9955" {
        return Err("embedded Green Screen user bubble token is not #6a9955".to_string());
    }
    if !is_valid_theme_slug("default-light") {
        return Err("runtime theme catalog did not load all embedded themes".to_string());
    }

    set_active_theme_slug(GREEN_SCREEN_SLUG);
    let palette = ResolvedPalette {
        user_bubble_bg: color_hex(Theme::user_bubble_bg()),
        user_bubble_text: color_hex(Theme::user_bubble_text()),
        text_primary: color_hex(Theme::text_primary()),
        selection_bg: color_hex(Theme::selection_bg()),
        selection_fg: color_hex(Theme::selection_fg()),
        assistant_bubble_bg: color_hex(Theme::assistant_bubble_bg()),
    };
    palette.verify_green_screen()?;
    Ok(palette)
}

fn main() {
    let palette = activate_and_verify_theme().expect("Green Screen theme self-check failed");
    palette.print();

    if std::env::args().any(|arg| arg == SELF_CHECK_ONLY_ARG) {
        return;
    }

    Application::new().run(|cx: &mut App| {
        GlobalState::init(cx);
        let bounds = Bounds::centered(None, size(px(WINDOW_WIDTH), px(WINDOW_HEIGHT)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                kind: WindowKind::Normal,
                focus: true,
                show: true,
                titlebar: None,
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(NotepadSelection::new);
                let focus_handle = view.read(cx).focus_handle.clone();
                window.focus(&focus_handle, cx);
                view
            },
        )
        .expect("failed to open notepad selection window");
        cx.activate(true);
    });
}
