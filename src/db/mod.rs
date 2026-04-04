//! Database layer for SQLite-backed conversation storage.
//!
//! Provides a single-threaded DB worker with a closure-based dispatch handle
//! (`DbHandle`) and schema initialization logic.

pub mod schema;
pub mod worker;

pub use worker::{spawn_db_thread, DbHandle};
