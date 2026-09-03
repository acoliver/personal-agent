// ============================================================================
// Steering presenter wiring (issue #222).
//
// The presenter sits on both halves of the steering path: the user event that
// offers a steer to the service, and the service events that say what became
// of it. These cover each half against the doubles the cancel tests already
// established, so a steer that is accepted, refused, queued or delivered is
// asserted through the same router the application runs.
//
// @plan PLAN-20260903-ISSUE222.P03
// ============================================================================

use super::cancel_tests::{MockAppSettingsService, RecordingChatService};
use super::*;
use crate::events::types::UserEvent;

/// Every `ViewCommand` already queued for the view, in order.
fn drain_view_commands(view_rx: &mut mpsc::Receiver<ViewCommand>) -> Vec<ViewCommand> {
    let mut commands = Vec::new();
    while let Ok(command) = view_rx.try_recv() {
        commands.push(command);
    }
    commands
}

/// Route `event` through the real user-event dispatcher, so these tests fail
/// if the `SteerStreaming` arm is missing as well as if the handler is wrong.
async fn dispatch_user_event(
    chat_service: &Arc<dyn ChatService>,
    view_tx: &mpsc::Sender<ViewCommand>,
    event: UserEvent,
) {
    let conversation_service = Arc::new(MockConversationService) as Arc<dyn ConversationService>;
    let profile_service = Arc::new(MockProfileService) as Arc<dyn ProfileService>;
    let app_settings_service = Arc::new(MockAppSettingsService) as Arc<dyn AppSettingsService>;
    let current_export_format = Arc::new(std::sync::Mutex::new(
        crate::models::ConversationExportFormat::Md,
    ));
    let pending_draft_conversation_id = Arc::new(std::sync::Mutex::new(None));

    let deps = ChatPresenterDeps {
        conversation_service: &conversation_service,
        chat_service,
        profile_service: &profile_service,
    };
    let state = ChatPresenterState {
        app_settings_service: &app_settings_service,
        current_export_format: &current_export_format,
        pending_draft_conversation_id: &pending_draft_conversation_id,
    };
    ChatPresenter::handle_user_event(&deps, &state, &mut view_tx.clone(), event).await;
}

/// An accepted steer reaches the service carrying the conversation and text it
/// was typed for, and tells the view nothing: the queued entry is announced by
/// `ChatEvent::SteeringQueued`, so reporting it here would show it twice.
///
/// @plan PLAN-20260903-ISSUE222.P03
/// @requirement REQ-222-002
#[tokio::test]
async fn steer_streaming_reaches_the_service_and_reports_no_rejection() {
    let chat_service = Arc::new(RecordingChatService::new());
    let (view_tx, mut view_rx) = mpsc::channel::<ViewCommand>(100);
    let conversation_id = Uuid::new_v4();

    dispatch_user_event(
        &(chat_service.clone() as Arc<dyn ChatService>),
        &view_tx,
        UserEvent::SteerStreaming {
            conversation_id,
            text: "read the failing test first".to_string(),
        },
    )
    .await;

    assert_eq!(
        chat_service.steer_calls(),
        vec![(conversation_id, "read the failing test first".to_string())],
        "SteerStreaming must reach ChatService::steer with its own conversation and text"
    );

    let commands = drain_view_commands(&mut view_rx);
    assert!(
        !commands
            .iter()
            .any(|command| matches!(command, ViewCommand::SteeringRejected { .. })),
        "an accepted steer must not be reported as rejected, got {commands:?}"
    );
    assert!(
        !commands
            .iter()
            .any(|command| matches!(command, ViewCommand::SteeringQueued { .. })),
        "the queued entry is announced by ChatEvent::SteeringQueued, not by the send path, \
         got {commands:?}"
    );
}

/// A refused steer withdraws nothing and shows the service's own words, so a
/// user who steers a finished turn learns why it did not take.
///
/// @plan PLAN-20260903-ISSUE222.P03
/// @requirement REQ-222-004
#[tokio::test]
async fn steer_streaming_rejection_surfaces_the_service_error() {
    let chat_service = Arc::new(RecordingChatService::rejecting("No active turn to steer"));
    let (view_tx, mut view_rx) = mpsc::channel::<ViewCommand>(100);
    let conversation_id = Uuid::new_v4();

    dispatch_user_event(
        &(chat_service.clone() as Arc<dyn ChatService>),
        &view_tx,
        UserEvent::SteerStreaming {
            conversation_id,
            text: "too late".to_string(),
        },
    )
    .await;

    let commands = drain_view_commands(&mut view_rx);

    let Some(rejected_at) = commands
        .iter()
        .position(|command| matches!(command, ViewCommand::SteeringRejected { .. }))
    else {
        panic!("a refused steer must emit SteeringRejected, got {commands:?}");
    };
    let ViewCommand::SteeringRejected {
        conversation_id: rejected_conversation_id,
        error,
    } = &commands[rejected_at]
    else {
        unreachable!("position matched SteeringRejected");
    };
    assert_eq!(
        *rejected_conversation_id, conversation_id,
        "the rejection must name the conversation the steer was typed for"
    );
    assert!(
        error.contains("No active turn to steer"),
        "the rejection must carry the service's reason, got {error:?}"
    );

    let Some(error_at) = commands
        .iter()
        .position(|command| matches!(command, ViewCommand::ShowError { .. }))
    else {
        panic!("a refused steer must also surface a ShowError, got {commands:?}");
    };
    let ViewCommand::ShowError {
        title,
        message,
        severity,
    } = &commands[error_at]
    else {
        unreachable!("position matched ShowError");
    };
    assert_eq!(title, "Steering Error");
    assert_eq!(
        message, error,
        "the shown error must be the same text the rejection carried"
    );
    assert_eq!(
        *severity,
        ErrorSeverity::Warning,
        "a refused steer costs the user nothing already sent, so it is a warning"
    );
    assert!(
        rejected_at < error_at,
        "the queued entry is withdrawn before the error explains why, got {commands:?}"
    );
}

/// Steering is additive in both directions: neither an accepted nor a refused
/// steer may reach `cancel`, which stays reachable only from an explicit stop.
///
/// @plan PLAN-20260903-ISSUE222.P03
/// @requirement REQ-222-006
#[tokio::test]
async fn steer_streaming_never_cancels_the_turn() {
    for chat_service in [
        Arc::new(RecordingChatService::new()),
        Arc::new(RecordingChatService::rejecting("Steering queue is full")),
    ] {
        let (view_tx, _view_rx) = mpsc::channel::<ViewCommand>(100);
        let conversation_id = Uuid::new_v4();

        dispatch_user_event(
            &(chat_service.clone() as Arc<dyn ChatService>),
            &view_tx,
            UserEvent::SteerStreaming {
                conversation_id,
                text: "keep going, but check the config".to_string(),
            },
        )
        .await;

        assert_eq!(
            chat_service.steer_calls().len(),
            1,
            "the steer must have been offered to the service"
        );
        assert!(
            chat_service.cancelled_ids().is_empty(),
            "SteerStreaming must never cancel a turn, cancelled {:?}",
            chat_service.cancelled_ids()
        );
    }
}

/// The service's report that a steer is waiting becomes the view's queued
/// entry, with the id the view needs to withdraw it later.
///
/// @plan PLAN-20260903-ISSUE222.P03
/// @requirement REQ-222-003
#[tokio::test]
async fn steering_queued_event_becomes_a_queued_view_command() {
    let (view_tx, mut view_rx) = mpsc::channel::<ViewCommand>(100);
    let conversation_id = Uuid::new_v4();
    let steer_id = Uuid::new_v4();

    ChatPresenter::handle_chat_event(
        &mut view_tx.clone(),
        ChatEvent::SteeringQueued {
            conversation_id,
            steer_id,
            text: "use the cached index".to_string(),
        },
    )
    .await;

    let commands = drain_view_commands(&mut view_rx);
    assert_eq!(
        commands,
        vec![ViewCommand::SteeringQueued {
            conversation_id,
            steer_id,
            text: "use the cached index".to_string(),
        }],
        "SteeringQueued must forward every field to the view unchanged"
    );
}

/// Delivery is reported with the same id the queued entry carried, which is
/// what lets the view stop rendering that entry as waiting.
///
/// @plan PLAN-20260903-ISSUE222.P03
/// @requirement REQ-222-003
#[tokio::test]
async fn steering_delivered_event_becomes_a_delivered_view_command() {
    let (view_tx, mut view_rx) = mpsc::channel::<ViewCommand>(100);
    let conversation_id = Uuid::new_v4();
    let steer_id = Uuid::new_v4();

    ChatPresenter::handle_chat_event(
        &mut view_tx.clone(),
        ChatEvent::SteeringDelivered {
            conversation_id,
            steer_id,
        },
    )
    .await;

    let commands = drain_view_commands(&mut view_rx);
    assert_eq!(
        commands,
        vec![ViewCommand::SteeringDelivered {
            conversation_id,
            steer_id,
        }],
        "SteeringDelivered must carry the queued entry's own id"
    );
}
