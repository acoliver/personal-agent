//! Behavioral tests for link styling on the surface a leaf renders onto.
//!
//! Issue #223: links styled with the global accent disappeared inside the
//! Green Screen user bubble because accent equals the bubble background.
//! These tests pin the contract that link glyph and underline colors derive
//! from the surface text color the caller passes in.

mod tests {
    use super::super::*;
    use crate::ui_gpui::theme::{active_theme_slug, set_active_theme_slug, Theme};

    use std::sync::{Mutex, MutexGuard};

    // Safety: tests that mutate global theme state are serialized with a mutex
    // so they don't interfere with each other. The guard restores the previous
    // slug on drop, even during unwinding.
    static THEME_SWITCH_LOCK: Mutex<()> = Mutex::new(());

    struct ThemeSwitchGuard {
        _lock: MutexGuard<'static, ()>,
        prev_slug: String,
    }

    impl ThemeSwitchGuard {
        fn acquire() -> Self {
            let lock = THEME_SWITCH_LOCK
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let prev_slug = active_theme_slug();
            Self {
                _lock: lock,
                prev_slug,
            }
        }
    }

    impl Drop for ThemeSwitchGuard {
        fn drop(&mut self) {
            set_active_theme_slug(&self.prev_slug);
        }
    }

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    fn colors_equal(a: gpui::Hsla, b: gpui::Hsla) -> bool {
        approx_eq(a.h, b.h) && approx_eq(a.s, b.s) && approx_eq(a.l, b.l) && approx_eq(a.a, b.a)
    }

    fn colors_differ(a: gpui::Hsla, b: gpui::Hsla) -> bool {
        !colors_equal(a, b)
    }

    #[derive(Default)]
    struct CapturingLeafFactory {
        leaves: Vec<MarkdownLeaf>,
    }

    impl MarkdownLeafFactory for CapturingLeafFactory {
        fn create_leaf(&mut self, leaf: MarkdownLeaf) -> gpui::AnyElement {
            self.leaves.push(leaf);
            gpui::div().into_any_element()
        }
    }

    /// Renders `markdown` onto a bubble surface the way the chat view does and
    /// returns every captured leaf.
    fn render_onto_surface(
        markdown: &str,
        text_color: gpui::Hsla,
        surface_bg: gpui::Hsla,
    ) -> Vec<MarkdownLeaf> {
        let blocks = parse_markdown_blocks(markdown);
        let mut factory = CapturingLeafFactory::default();
        let mut order = 0;
        let _ = blocks_to_elements_with_leaf_factory(
            &blocks,
            text_color,
            surface_bg,
            &mut factory,
            &mut order,
            "",
        );
        factory.leaves
    }

    /// Returns the single text run whose byte range covers `needle`.
    fn run_covering<'a>(leaves: &'a [MarkdownLeaf], needle: &str) -> &'a gpui::TextRun {
        let leaf = leaves
            .iter()
            .find(|leaf| leaf.plain_text.as_ref().contains(needle))
            .unwrap_or_else(|| panic!("no captured leaf contains {needle:?}"));
        let text = leaf.plain_text.as_ref();
        let start = text
            .find(needle)
            .unwrap_or_else(|| panic!("needle {needle:?} vanished from leaf text"));
        let end = start + needle.len();
        let mut cursor = 0;
        for run in &leaf.text_runs {
            let run_end = cursor + run.len;
            if start >= cursor && end <= run_end {
                return run;
            }
            cursor = run_end;
        }
        panic!("no single run covers {needle:?}");
    }

    /// @plan:PLAN-20260902-ISSUE223.P02
    /// @requirement:REQ-MSG-LINK-001
    /// Green Screen accent equals the user bubble background, so a link
    /// painted with the accent is invisible there (issue #223).
    #[test]
    fn green_screen_user_bubble_link_uses_bubble_text_color_not_background() {
        let _guard = ThemeSwitchGuard::acquire();
        set_active_theme_slug("green-screen");

        assert!(
            colors_equal(Theme::accent(), Theme::user_bubble_bg()),
            "green-screen must keep accent == user bubble bg for this regression to bite"
        );
        assert!(
            colors_differ(Theme::user_bubble_text(), Theme::user_bubble_bg()),
            "user bubble text must contrast with the bubble background"
        );

        let leaves = render_onto_surface(
            "See https://example.com now",
            Theme::user_bubble_text(),
            Theme::user_bubble_bg(),
        );
        let run = run_covering(&leaves, "https://example.com");
        assert!(
            colors_equal(run.color, Theme::user_bubble_text()),
            "link glyph must use the surface text color, got {:?}",
            run.color
        );
        assert!(
            colors_differ(run.color, Theme::user_bubble_bg()),
            "link glyph equals the bubble background: invisible link (issue #223)"
        );
        let underline = run.underline.as_ref().expect("link run keeps an underline");
        assert!(
            underline
                .color
                .is_some_and(|c| colors_equal(c, Theme::user_bubble_text())),
            "underline color must follow the surface text color"
        );
    }

    /// @plan:PLAN-20260902-ISSUE223.P02
    /// The underline is the link affordance once the glyph color follows the
    /// surface text color; it must survive the fix.
    #[test]
    fn link_runs_keep_an_underline_on_the_user_bubble() {
        let _guard = ThemeSwitchGuard::acquire();
        set_active_theme_slug("green-screen");

        let leaves = render_onto_surface(
            "See [docs](https://example.com) now",
            Theme::user_bubble_text(),
            Theme::user_bubble_bg(),
        );
        let run = run_covering(&leaves, "docs");
        assert!(
            run.underline.is_some(),
            "link affordance must stay an underline"
        );
    }

    /// @plan:PLAN-20260902-ISSUE223.P02
    /// Plain spans on the same surface must keep the passed text color.
    #[test]
    fn plain_spans_on_the_user_bubble_keep_the_passed_text_color() {
        let _guard = ThemeSwitchGuard::acquire();
        set_active_theme_slug("green-screen");

        let leaves = render_onto_surface(
            "See https://example.com now",
            Theme::user_bubble_text(),
            Theme::user_bubble_bg(),
        );
        for plain in ["See", "now"] {
            let run = run_covering(&leaves, plain);
            assert!(
                colors_equal(run.color, Theme::user_bubble_text()),
                "plain span {plain:?} must keep the passed text color"
            );
            assert!(run.underline.is_none(), "plain span must not be underlined");
        }
    }

    /// @plan:PLAN-20260902-ISSUE223.P02
    /// default-light separates accent (#3B82F6) from text.primary (#1f2937),
    /// so this assistant-surface variant catches a global accent reach-in;
    /// in green-screen the two collapse to the same green and could not.
    #[test]
    fn assistant_surface_link_uses_assistant_text_color_not_global_accent() {
        let _guard = ThemeSwitchGuard::acquire();
        set_active_theme_slug("default-light");

        assert!(
            colors_differ(Theme::accent(), Theme::text_primary()),
            "default-light must separate accent from text primary for this test to bite"
        );

        let leaves = render_onto_surface(
            "See https://example.com now",
            Theme::text_primary(),
            Theme::assistant_bubble_bg(),
        );
        let run = run_covering(&leaves, "https://example.com");
        assert!(
            colors_equal(run.color, Theme::text_primary()),
            "link glyph must use the assistant surface text color"
        );
        let underline = run.underline.as_ref().expect("link run keeps an underline");
        assert!(
            underline
                .color
                .is_some_and(|c| colors_equal(c, Theme::text_primary())),
            "underline color must follow the assistant surface text color"
        );
    }
}
