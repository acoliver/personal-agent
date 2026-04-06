//! Configuration module for `PersonalAgent`

mod provider_defaults;
pub mod quirks_manifest;
mod settings;

pub use provider_defaults::{
    default_api_base_url_for_provider, provider_api_url, provider_api_url_map,
};
pub use quirks_manifest::quirks_manifest;
pub use settings::{CompressionConfig, Config, ContextManagement};
