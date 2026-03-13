//! Configuration module for `PersonalAgent`

mod provider_defaults;
mod settings;

pub use provider_defaults::{
    default_api_base_url_for_provider, provider_api_url, provider_api_url_map,
};
pub use settings::{Config, ContextManagement};
