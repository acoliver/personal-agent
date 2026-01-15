//! LLM integration using SerdesAI
//!
//! This module provides the bridge between PersonalAgent's config/profile system
//! and the SerdesAI library for making LLM requests.

mod client;

pub use client::{LlmClient, LlmError, Message, Role, StreamEvent};
