mod tests {
    use super::super::*;
    use crate::ui_gpui::theme::Theme;

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

    #[test]
    fn selectable_leaves_use_the_surface_their_block_paints() {
        let blocks = parse_markdown_blocks(
            "### Heading\n\n- list item\n\n> quote\n\n```rust\ncode\n```\n\n| head | value |\n|---|---|\n| row one | 1 |\n| row two | 2 |",
        );
        let parent_background = gpui::hsla(0.5, 0.8, 0.5, 1.0);
        let parent_foreground = gpui::hsla(0.9, 0.8, 0.5, 1.0);
        let mut factory = CapturingLeafFactory::default();
        let mut order = 0;
        let _ = blocks_to_elements_with_leaf_factory(
            &blocks,
            parent_foreground,
            parent_background,
            &mut factory,
            &mut order,
            "",
        );

        assert_eq!(markdown_leaf_count(&blocks), factory.leaves.len());

        let leaf = |text: &str| {
            factory
                .leaves
                .iter()
                .find(|leaf| leaf.plain_text.as_ref() == text)
                .unwrap_or_else(|| panic!("missing captured leaf {text:?}"))
        };
        for text in ["Heading", "• ", "list item"] {
            assert_eq!(leaf(text).surface_background, parent_background);
            assert_eq!(leaf(text).surface_foreground, parent_foreground);
        }

        let quote = leaf("quote");
        assert_eq!(quote.surface_background, Theme::bg_base());
        assert_eq!(quote.surface_foreground, Theme::text_primary());

        let code = leaf("code\n");
        assert_eq!(code.surface_background, Theme::bg_dark());
        assert_eq!(code.surface_foreground, Theme::text_primary());

        for text in ["head", "value", "row one", "1", "row two", "2"] {
            assert_eq!(leaf(text).surface_foreground, Theme::text_primary());
        }
        for text in ["head", "value", "row two", "2"] {
            assert_eq!(leaf(text).surface_background, Theme::bg_dark());
        }
        for text in ["row one", "1"] {
            assert_eq!(leaf(text).surface_background, Theme::bg_base());
        }
    }

    #[test]
    fn selectable_link_ranges_are_leaf_local_utf8_byte_ranges() {
        let blocks = parse_markdown_blocks("é [Rust](https://www.rust-lang.org/) docs");
        let mut factory = CapturingLeafFactory::default();
        let mut order = 0;
        let _ = blocks_to_elements_with_leaf_factory(
            &blocks,
            Theme::text_primary(),
            Theme::bg_base(),
            &mut factory,
            &mut order,
            "",
        );

        assert_eq!(factory.leaves.len(), 1);
        let leaf = &factory.leaves[0];
        assert_eq!(leaf.plain_text.as_ref(), "é Rust docs");
        assert_eq!(
            leaf.links,
            [(3..7, "https://www.rust-lang.org/".to_string())]
        );
        let (range, _) = &leaf.links[0];
        assert!(leaf.plain_text.is_char_boundary(range.start));
        assert!(leaf.plain_text.is_char_boundary(range.end));
        assert_eq!(&leaf.plain_text[range.clone()], "Rust");
    }

    #[test]
    fn selectable_leaves_exclude_unsafe_link_targets() {
        let blocks = parse_markdown_blocks(
            "[script](javascript:alert(1)) [file](file:///tmp/private) [web](https://example.com)",
        );
        let mut factory = CapturingLeafFactory::default();
        let mut order = 0;
        let _ = blocks_to_elements_with_leaf_factory(
            &blocks,
            Theme::text_primary(),
            Theme::bg_base(),
            &mut factory,
            &mut order,
            "",
        );

        assert_eq!(factory.leaves.len(), 1);
        assert_eq!(
            factory.leaves[0].links,
            [(12..15, "https://example.com".to_string())]
        );
    }
}
