//! Test-only hooks on `ChatServiceImpl` for driving real stream state.
//!
//! Moved here from `chat_impl.rs` so that file stays under the repo's
//! 1000-line ceiling. Pure relocation: the methods are unchanged and remain
//! compiled only under `cfg(test)`.
//!
//! @plan PLAN-20260905-STEERINT.P04

use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex};

use uuid::Uuid;

use super::{
    ActiveStream, ChatServiceImpl, ServiceError, ServiceResult, SteeringQueues, StreamLifecycle,
};

#[cfg(test)]
impl ChatServiceImpl {
    /// Test-only shim to begin a stream for a specific conversation.
    ///
    /// Exercises the real `begin_stream` guard path (same synchronous
    /// reservation used in production `send_message`), then promotes the
    /// reservation from `Starting` to `Running` by attaching a long-sleeping
    /// mock task. This matches what `spawn_stream_task` does for real
    /// streams, so tests see `is_streaming_for` return `true` without
    /// driving an actual LLM stream.
    ///
    /// Returns `Err` with the real error message if `begin_stream`
    /// rejects the reservation (e.g. duplicate conversation).
    ///
    /// @plan PLAN-20260416-ISSUE173.P14-CR5
    /// @requirement REQ-173-001.1
    /// @requirement REQ-173-001.2
    pub(crate) fn begin_stream_for_test(&self, conversation_id: Uuid) -> ServiceResult<()> {
        let (stream_id, cancel) = self.begin_stream(conversation_id)?;

        // Mock long-running task. When the stream is cancelled or cleared,
        // the `CancellationToken` is cancelled (and `task.abort()` is called
        // by `cancel_active_stream` / `clear_all_streams_for_test`), which
        // wakes this sleep and lets the task exit cleanly.
        let mut task = Some(tokio::spawn(async move {
            tokio::select! {
                () = cancel.cancelled() => {}
                () = tokio::time::sleep(tokio::time::Duration::from_hours(1)) => {}
            }
        }));

        let promoted = {
            let mut map = self.active_streams.lock().expect("active_streams poisoned");
            if let Some(entry) = map.get_mut(&conversation_id) {
                if entry.stream_id == stream_id {
                    entry.task = task.take();
                    entry.state = StreamLifecycle::Running;
                    true
                } else {
                    false
                }
            } else {
                false
            }
        };
        if promoted {
            Ok(())
        } else {
            // Entry was replaced or removed between reserve and promotion —
            // drop the spawned task to avoid leaking it.
            if let Some(handle) = task {
                handle.abort();
            }
            Err(ServiceError::Internal(
                "Stream reservation lost before promotion".to_string(),
            ))
        }
    }

    /// Test-only helper to read back the `stream_id` of the active entry for
    /// a conversation, if any. Used by CR #4 regression tests that verify
    /// a stale task cannot evict a newer stream.
    ///
    /// @plan PLAN-20260416-ISSUE173.P14-CR4
    /// @requirement REQ-173-001.3
    pub(crate) fn stream_id_for_test(&self, conversation_id: Uuid) -> Option<Uuid> {
        let map = self.active_streams.lock().expect("active_streams poisoned");
        map.get(&conversation_id).map(|a| a.stream_id)
    }

    /// Test-only handles to the two registries a spawned stream task is
    /// given, so tests can drive the steering delivery loop against the same
    /// state the real service reads through `is_streaming_for` and `steer`.
    ///
    /// @plan PLAN-20260903-ISSUE222.P02
    /// @requirement REQ-222-005
    pub(in crate::services::chat_impl) fn stream_registries_for_test(
        &self,
    ) -> (Arc<StdMutex<HashMap<Uuid, ActiveStream>>>, SteeringQueues) {
        (self.active_streams.clone(), self.steering_queues.clone())
    }

    /// Test-only handle to a conversation's steering transport: the fork
    /// queue the send's slot holds, so acceptance tests can observe the
    /// commit that puts committed text where a run will deliver it.
    ///
    /// @plan PLAN-20260905-STEERINT.P03
    /// @requirement REQ-SI-002
    pub(in crate::services::chat_impl) fn steering_transport_for_test(
        &self,
        conversation_id: Uuid,
    ) -> Option<serdes_ai_agent::SteeringQueue> {
        let map = self.active_streams.lock().expect("active_streams poisoned");
        map.get(&conversation_id).map(|a| a.steering.clone())
    }

    /// Test-only helper to clear all mock streams.
    /// @plan PLAN-20260416-ISSUE173.P03
    /// @requirement REQ-173-001.3
    pub(crate) fn clear_all_streams_for_test(&self) {
        let mut map = self.active_streams.lock().expect("active_streams poisoned");
        for (_, active) in map.drain() {
            active.cancel.cancel();
            if let Some(handle) = active.task {
                handle.abort();
            }
        }
    }
}
