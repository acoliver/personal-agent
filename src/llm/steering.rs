//! Mid-turn steering plumbing between the chat service layer and the agent
//! stream.
//!
//! The transport half lives in the fork (`serdes_ai_agent::SteeringQueue`):
//! it decides when queued text reaches the model, at the tool-call boundary
//! of a running turn. This module carries the two pieces the LLM layer needs
//! to participate without knowing about persistence or the UI: the input
//! bundle attached to a run, and the sink that observes what the model
//! actually received.
//!
//! @plan PLAN-20260905-STEERINT.P02
//! @requirement REQ-SI-001
//! @requirement REQ-SI-003

use async_trait::async_trait;

use crate::llm::LlmError;

// Use std Result to avoid conflict with serdes_ai::prelude::Result
type StdResult<T, E> = std::result::Result<T, E>;

/// Observes steering texts the model actually received mid-turn.
///
/// The LLM layer calls [`SteeringDeliverySink::delivered`] once per
/// [`AgentStreamEvent::SteeringDelivered`], in delivery order. Implementors
/// live in the service layer so storage stays out of the LLM crate; the trait
/// object keeps the dependency pointed this direction only.
///
/// @plan PLAN-20260905-STEERINT.P02
/// @requirement REQ-SI-003
#[async_trait]
pub trait SteeringDeliverySink: Send {
    /// Record a steering text that reached the model.
    ///
    /// # Errors
    ///
    /// Returns `Err` when the delivered text could not be handled (today:
    /// not persisted). The stream fails on the error; the model has already
    /// seen the text, so the divergence is reported, never retried.
    async fn delivered(&mut self, text: &str) -> StdResult<(), LlmError>;
}

/// Everything a run needs to accept mid-turn steering.
///
/// The queue is cloned onto the run's `RunOptions`; the sink observes the
/// resulting deliveries. Owned by the caller of
/// [`AgentClientExt::run_agent_stream`] for the duration of the send.
///
/// @plan PLAN-20260905-STEERINT.P02
/// @requirement REQ-SI-002
pub struct SteeringInput<'a> {
    /// Transport queue the run drains at tool-call boundaries.
    pub queue: serdes_ai_agent::SteeringQueue,
    /// Observer invoked for every text the run delivers to the model.
    pub sink: &'a mut dyn SteeringDeliverySink,
}
