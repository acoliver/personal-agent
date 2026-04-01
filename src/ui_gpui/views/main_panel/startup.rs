//! `MainPanel` startup and lifecycle helpers.
//!
//! Contains `MainPanelAppState` (the GPUI global), and the `impl MainPanel`
//! block covering store subscription, child-view initialisation, bridge
//! polling, runtime start, and the optional test-conversation-switch probe.
//!
//! @plan PLAN-20260325-ISSUE11B.P02
//! @plan PLAN-20260304-GPUIREMEDIATE.P04
//! @requirement REQ-ARCH-001.1
//! @requirement REQ-ARCH-001.3
//! @requirement REQ-ARCH-004.1

use crate::events::types::UserEvent;
use crate::ui_gpui::bridge::GpuiBridge;
use crate::ui_gpui::GpuiAppStore;
use gpui::Global;
use std::sync::Arc;

use super::MainPanel;

/// Global app state containing the bridge.
///
/// Used by `MainPanel` to receive `ViewCommands`.
/// @plan PLAN-20250130-GPUIREDUX.P11
/// @plan PLAN-20260304-GPUIREMEDIATE.P04
/// @requirement REQ-ARCH-001.1
/// @requirement REQ-ARCH-001.3
/// @requirement REQ-ARCH-004.1
/// @pseudocode analysis/pseudocode/03-main-panel-integration.md:001-035
#[derive(Clone)]
pub struct MainPanelAppState {
    pub gpui_bridge: Arc<GpuiBridge>,
    pub popup_window: Option<gpui::WindowHandle<MainPanel>>,
    pub app_store: Arc<GpuiAppStore>,
}

impl Global for MainPanelAppState {}

impl MainPanel {
    /// @plan PLAN-20260304-GPUIREMEDIATE.P04
    /// @requirement REQ-ARCH-001.3
    /// @requirement REQ-ARCH-004.1
    /// @pseudocode analysis/pseudocode/03-main-panel-integration.md:022-035
    pub(super) fn ensure_store_subscription(&mut self, cx: &mut gpui::Context<Self>) {
        if self.store_subscription_task.is_some() {
            return;
        }

        let Some(app_state) = cx.try_global::<MainPanelAppState>() else {
            tracing::warn!("MainPanel: no app state available for store subscription");
            return;
        };

        let store_rx = app_state.app_store.subscribe();
        let entity = cx.entity();
        self.store_subscription_task = Some(cx.spawn(async move |_, cx| {
            while let Ok(snapshot) = store_rx.recv_async().await {
                let () = entity.update(cx, |this, cx| {
                    this.apply_store_snapshot(snapshot, cx);
                });
            }
        }));
    }

    /// Initialize all child views with bridge
    pub(super) fn request_runtime_snapshots(cx: &mut gpui::Context<Self>) {
        if let Some(app_state) = cx.try_global::<MainPanelAppState>() {
            let bridge = app_state.gpui_bridge.clone();
            let _ = bridge.emit(UserEvent::RefreshProfiles);
            let _ = bridge.emit(UserEvent::RefreshHistory);
            let _ = bridge.emit(UserEvent::RefreshApiKeys);
            let _ = bridge.emit(UserEvent::RefreshToolApprovalPolicy);
        }
    }

    /// @plan PLAN-20260304-GPUIREMEDIATE.P08
    /// @requirement REQ-ARCH-005.1
    /// @pseudocode analysis/pseudocode/03-main-panel-integration.md:014-127
    pub(super) fn apply_startup_state(&mut self, cx: &mut gpui::Context<Self>) {
        if let Some(app_state) = cx.try_global::<MainPanelAppState>() {
            self.apply_store_snapshot(app_state.app_store.current_snapshot(), cx);
        }
    }

    /// @plan PLAN-20260304-GPUIREMEDIATE.P05
    /// @requirement REQ-ARCH-003.4
    /// @requirement REQ-ARCH-004.1
    /// @pseudocode analysis/pseudocode/03-main-panel-integration.md:079-088
    pub(super) fn ensure_bridge_polling(&mut self, _cx: &mut gpui::Context<Self>) {
        if self.bridge_poll_task.is_none() {
            tracing::debug!(
                "MainPanel: bridge polling retained as no-op; app-root pump owns bridge draining"
            );
        }
    }

    pub(super) fn maybe_start_test_conversation_switch(&mut self, cx: &mut gpui::Context<Self>) {
        if self.test_conversation_switch_task.is_some() {
            return;
        }

        let enabled = std::env::var("PA_TEST_CONVERSATION_SWITCH").ok().as_deref() == Some("1");
        if !enabled {
            return;
        }

        self.test_conversation_switch_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(1200))
                .await;

            let first_target = this
                .read_with(cx, |this, cx| {
                    let chat_view = this.chat_view.as_ref()?;
                    let history_view = this.history_view.as_ref()?;
                    let current_conversation_id = chat_view.read(cx).state.active_conversation_id;
                    history_view
                        .read(cx)
                        .conversations()
                        .iter()
                        .find(|conversation| {
                            Some(conversation.id) != current_conversation_id
                                && conversation.message_count > 0
                        })
                        .map(|conversation| conversation.id)
                })
                .ok()
                .flatten();

            let Some(first_target) = first_target else {
                tracing::warn!(
                    "MainPanel: test conversation switch mode could not find a switch target"
                );
                return;
            };

            tracing::info!(
                conversation_id = %first_target,
                "MainPanel: test conversation switch selecting alternate conversation"
            );
            let _ = this.update(cx, |this, cx| {
                if let Some(chat_view) = this.chat_view.as_ref() {
                    chat_view.update(cx, |view, cx| {
                        view.select_conversation_by_id(first_target, cx);
                    });
                }
            });

            cx.background_executor()
                .timer(std::time::Duration::from_millis(1200))
                .await;

            let second_target = this
                .read_with(cx, |this, cx| {
                    let chat_view = this.chat_view.as_ref()?;
                    let history_view = this.history_view.as_ref()?;
                    let current_conversation_id = chat_view.read(cx).state.active_conversation_id;
                    history_view
                        .read(cx)
                        .conversations()
                        .iter()
                        .find(|conversation| Some(conversation.id) != current_conversation_id)
                        .map(|conversation| conversation.id)
                })
                .ok()
                .flatten();

            let Some(second_target) = second_target else {
                tracing::warn!(
                    "MainPanel: test conversation switch mode could not find a return target"
                );
                return;
            };

            tracing::info!(
                conversation_id = %second_target,
                "MainPanel: test conversation switch returning to original conversation"
            );
            let _ = this.update(cx, |this, cx| {
                if let Some(chat_view) = this.chat_view.as_ref() {
                    chat_view.update(cx, |view, cx| {
                        view.select_conversation_by_id(second_target, cx);
                    });
                }
            });
        }));
    }
}
