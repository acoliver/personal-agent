#[cfg(test)]
mod tests {
    use super::super::ChatView;
    use gpui::px;

    #[test]
    fn profile_dropdown_left_aligns_under_trigger_in_popup() {
        let left = ChatView::compute_profile_dropdown_left(px(760.0), 0.0);
        assert_eq!(left, px(276.0));
    }

    #[test]
    fn profile_dropdown_left_shifts_for_sidebar_toggle_in_popout() {
        let left = ChatView::compute_profile_dropdown_left(px(760.0), 36.0);
        assert_eq!(left, px(312.0));
    }

    #[test]
    fn profile_dropdown_left_clamps_to_right_bound_on_narrow_windows() {
        let clamped_right = ChatView::compute_profile_dropdown_left(px(520.0), 0.0);
        assert_eq!(clamped_right, px(248.0));
    }

    #[test]
    fn profile_dropdown_left_uses_minimum_margin_for_narrow_windows() {
        let left = ChatView::compute_profile_dropdown_left(px(200.0), 0.0);
        assert_eq!(left, px(12.0));
    }
}
