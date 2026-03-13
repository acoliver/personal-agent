use personal_agent::ui_gpui::views::chat_view::ChatView;

#[test]
fn chat_area_scroll_region_has_min_height_reset() {
    let source = include_str!("../src/ui_gpui/views/chat_view.rs");

    let chat_area_pos = source
        .find(".id(\"chat-area\")")
        .expect("chat area should exist");
    let window = &source[chat_area_pos..std::cmp::min(chat_area_pos + 240, source.len())];

    assert!(
        window.contains(".min_h_0()"),
        "Chat scroll region should reset min-height so full transcript remains scrollable inside the flex layout"
    );
}

#[test]
fn main_panel_content_host_has_min_height_reset() {
    let source = include_str!("../src/ui_gpui/views/main_panel.rs");

    let content_host_pos = source
        .find(".flex_1()\n                    .min_h_0()")
        .or_else(|| source.find(".flex_1()\r\n                    .min_h_0()"))
        .expect("main panel content host should have flex_1 + min_h_0");
    let window = &source[content_host_pos..std::cmp::min(content_host_pos + 180, source.len())];

    assert!(
        window.contains(".overflow_hidden()"),
        "Main panel content host should remain overflow-hidden while allowing child scroll regions to shrink"
    );
}

/// @plan PLAN-20260304-GPUIREMEDIATE.P03
/// @requirement REQ-INT-001.2
/// @requirement REQ-ARCH-002.4
/// @pseudocode analysis/pseudocode/03-main-panel-integration.md:009-013
#[test]
fn startup_and_runtime_transcript_delivery_converge_on_store_snapshot_mount_path() {
    let startup_source = include_str!("../src/main_gpui.rs");
    let panel_source = include_str!("../src/ui_gpui/views/main_panel.rs");

    assert!(
        startup_source.contains("build_startup_inputs")
            && panel_source.contains("apply_startup_state")
            && panel_source.contains("apply_store_snapshot"),
        "startup transcript delivery should now converge on startup inputs plus store snapshot application"
    );

    assert!(
        !startup_source.contains("build_startup_view_commands"),
        "auxiliary source/readback guardrail: startup and runtime transcript delivery should already converge on one authoritative state path without legacy startup transcript replay"
    );
}

#[test]
fn chat_view_type_is_still_exported_for_gpui_mounting() {
    let _ = std::any::type_name::<ChatView>();
}
