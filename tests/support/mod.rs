//! Helpers shared by the integration tests.
//!
//! Every test binary compiles this whole module, so the helpers a given
//! binary does not use look dead to it.
#![allow(dead_code)]

pub mod e2e_config;
pub mod stub_profile_service;
