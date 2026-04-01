//! Data-driven provider quirks manifest.
//!
//! Loads a bundled TOML manifest (compiled into the binary) that defines
//! per-provider overrides such as custom headers, transport selection, and
//! base URL. A user-overridable layer at the standard app config path is
//! merged on top so power users can add entries without waiting for a release.

use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

/// Bundled manifest compiled into the binary.
const BUNDLED_MANIFEST: &str = include_str!("provider_quirks.toml");

/// User-override filename placed alongside the main config.
const USER_MANIFEST_FILENAME: &str = "provider_quirks.toml";

/// A single provider entry in the quirks manifest.
#[derive(Debug, Clone, Deserialize, Default, PartialEq, Eq)]
pub struct QuirksEntry {
    /// `SerdesAI` transport provider (e.g. "openai", "anthropic").
    pub transport: Option<String>,
    /// API base URL override.
    pub base_url: Option<String>,
    /// Custom HTTP headers (e.g. User-Agent spoofing).
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

/// Merged registry of provider quirks (bundled + user overlay).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuirksManifest {
    entries: HashMap<String, QuirksEntry>,
}

impl QuirksManifest {
    /// Look up a provider entry by ID.
    #[must_use]
    pub fn get(&self, provider_id: &str) -> Option<&QuirksEntry> {
        self.entries.get(provider_id)
    }

    /// Iterate over all entries.
    #[must_use]
    pub const fn entries(&self) -> &HashMap<String, QuirksEntry> {
        &self.entries
    }

    /// Build the manifest by parsing the bundled TOML and overlaying the
    /// optional user config file.
    fn load() -> Self {
        let mut entries: HashMap<String, QuirksEntry> =
            toml::from_str(BUNDLED_MANIFEST).unwrap_or_default();

        if let Some(user_entries) = Self::load_user_manifest() {
            for (id, user_entry) in user_entries {
                entries.insert(id, user_entry);
            }
        }

        Self { entries }
    }

    /// Try to load the user-overridable manifest from the app config directory.
    fn load_user_manifest() -> Option<HashMap<String, QuirksEntry>> {
        let app_support = dirs::data_local_dir()?;
        let path = app_support
            .join("PersonalAgent")
            .join(USER_MANIFEST_FILENAME);
        let content = std::fs::read_to_string(path).ok()?;
        toml::from_str(&content).ok()
    }
}

/// Return the global quirks manifest (loaded once per process).
pub fn quirks_manifest() -> &'static QuirksManifest {
    static MANIFEST: OnceLock<QuirksManifest> = OnceLock::new();
    MANIFEST.get_or_init(QuirksManifest::load)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_manifest_parses_successfully() {
        let entries: HashMap<String, QuirksEntry> =
            toml::from_str(BUNDLED_MANIFEST).expect("bundled manifest should parse");
        assert!(
            !entries.is_empty(),
            "bundled manifest should have at least one entry"
        );
    }

    #[test]
    fn kimi_entry_has_transport_base_url_and_headers() {
        let entries: HashMap<String, QuirksEntry> =
            toml::from_str(BUNDLED_MANIFEST).expect("parse");
        let kimi = entries
            .get("kimi-for-coding")
            .expect("kimi entry should exist");

        assert_eq!(kimi.transport.as_deref(), Some("openai"));
        assert_eq!(
            kimi.base_url.as_deref(),
            Some("https://api.kimi.com/coding/v1")
        );
        assert_eq!(
            kimi.headers.get("User-Agent").map(String::as_str),
            Some("RooCode/1.0")
        );
    }

    #[test]
    fn anthropic_entry_has_base_url_only() {
        let entries: HashMap<String, QuirksEntry> =
            toml::from_str(BUNDLED_MANIFEST).expect("parse");
        let anthropic = entries
            .get("anthropic")
            .expect("anthropic entry should exist");

        assert!(anthropic.transport.is_none());
        assert_eq!(
            anthropic.base_url.as_deref(),
            Some("https://api.anthropic.com/v1")
        );
        assert!(anthropic.headers.is_empty());
    }

    #[test]
    fn openrouter_entry_has_openai_transport() {
        let entries: HashMap<String, QuirksEntry> =
            toml::from_str(BUNDLED_MANIFEST).expect("parse");
        let entry = entries
            .get("openrouter")
            .expect("openrouter entry should exist");

        assert_eq!(entry.transport.as_deref(), Some("openai"));
        assert_eq!(
            entry.base_url.as_deref(),
            Some("https://openrouter.ai/api/v1")
        );
    }

    #[test]
    fn user_overlay_overrides_bundled_entries() {
        let bundled: HashMap<String, QuirksEntry> =
            toml::from_str(BUNDLED_MANIFEST).expect("parse bundled");

        let user_toml = r#"
[kimi-for-coding]
transport = "openai"
base_url = "https://custom.kimi.example.com/v1"

[kimi-for-coding.headers]
User-Agent = "CustomAgent/2.0"

[my-custom-provider]
transport = "openai"
base_url = "https://my-custom.example.com/v1"
"#;
        let user_entries: HashMap<String, QuirksEntry> =
            toml::from_str(user_toml).expect("parse user");

        let mut merged = bundled;
        for (id, entry) in user_entries {
            merged.insert(id, entry);
        }

        let kimi = merged.get("kimi-for-coding").expect("kimi");
        assert_eq!(
            kimi.base_url.as_deref(),
            Some("https://custom.kimi.example.com/v1"),
            "user entry should override bundled"
        );
        assert_eq!(
            kimi.headers.get("User-Agent").map(String::as_str),
            Some("CustomAgent/2.0")
        );

        assert!(
            merged.contains_key("my-custom-provider"),
            "user-only entries should be added"
        );

        assert!(
            merged.contains_key("anthropic"),
            "bundled entries not in user file should be preserved"
        );
    }

    #[test]
    fn global_manifest_returns_consistent_reference() {
        let m1 = quirks_manifest();
        let m2 = quirks_manifest();
        assert!(std::ptr::eq(m1, m2), "should return same static reference");
    }

    #[test]
    fn global_manifest_contains_bundled_entries() {
        let manifest = quirks_manifest();
        assert!(
            manifest.get("kimi-for-coding").is_some(),
            "global manifest should contain kimi"
        );
        assert!(
            manifest.get("anthropic").is_some(),
            "global manifest should contain anthropic"
        );
    }

    #[test]
    fn all_bundled_providers_present() {
        let entries: HashMap<String, QuirksEntry> =
            toml::from_str(BUNDLED_MANIFEST).expect("parse");

        let expected = [
            "anthropic",
            "openai",
            "kimi-for-coding",
            "openrouter",
            "synthetic",
            "moonshotai",
            "moonshotai-cn",
        ];

        for id in expected {
            assert!(entries.contains_key(id), "missing bundled entry: {id}");
        }
    }

    #[test]
    fn entry_without_headers_has_empty_map() {
        let entries: HashMap<String, QuirksEntry> =
            toml::from_str(BUNDLED_MANIFEST).expect("parse");
        let openai = entries.get("openai").expect("openai entry");
        assert!(openai.headers.is_empty());
    }
}
