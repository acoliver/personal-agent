//! What a model accepts, and the reasoning effort levels it offers.
//!
//! Two separate axes live here, and keeping them separate is the point.
//! A token budget is a count: how many tokens a model may spend thinking.
//! An effort level is a named value the backend interprets, with no token
//! equivalence at all. Deriving one from the other invents a correspondence
//! that does not exist, which is what this module replaces.
//!
//! Which controls the editor offers is answered by [`ModelCapabilities`]
//! rather than by the API type, so a provider that accepts an unusual
//! combination is described rather than special-cased.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// How hard a reasoning model should think.
///
/// These are opaque named levels, not amounts. The ladder is the one the
/// codex backend accepts; [`Self::Other`] carries a level this build does
/// not know so a value added upstream reaches the wire without a release.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(from = "String", into = "String")]
pub enum ReasoningEffort {
    None,
    Minimal,
    Low,
    /// The backend's own default, and so this client's.
    #[default]
    Medium,
    High,
    XHigh,
    Max,
    Ultra,
    Persistent,
    /// A level this client does not recognise, preserved verbatim.
    Other(String),
}

impl ReasoningEffort {
    /// The value that goes on the wire, which is not always the name.
    ///
    /// Two levels are local names rather than wire values, and the codex
    /// client translates both before sending:
    ///
    /// - `Ultra` is sent as `max`. It is the same request; upstream keeps the
    ///   separate name for presentation.
    /// - `Persistent` is sent as `disabled`, which is what the Responses API
    ///   calls that mode.
    ///
    /// Sending either name verbatim would be a value the endpoint does not
    /// know.
    #[must_use]
    pub fn wire_value(&self) -> &str {
        match self {
            Self::Ultra => "max",
            Self::Persistent => "disabled",
            other => other.as_str(),
        }
    }

    /// The name this level is stored and displayed under.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::None => "none",
            Self::Minimal => "minimal",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::XHigh => "xhigh",
            Self::Max => "max",
            Self::Ultra => "ultra",
            Self::Persistent => "persistent",
            Self::Other(value) => value,
        }
    }

    /// The levels this build knows, in ascending order of effort.
    ///
    /// `Other` is deliberately absent: it exists to carry a value from
    /// elsewhere, not to be offered as a choice.
    #[must_use]
    pub fn known() -> Vec<Self> {
        vec![
            Self::None,
            Self::Minimal,
            Self::Low,
            Self::Medium,
            Self::High,
            Self::XHigh,
            Self::Max,
            Self::Ultra,
            Self::Persistent,
        ]
    }

    /// A label for the picker.
    #[must_use]
    pub fn display_name(&self) -> String {
        match self {
            Self::XHigh => "Extra High".to_string(),
            Self::Other(value) => value.clone(),
            _ => {
                let raw = self.as_str();
                let mut chars = raw.chars();
                chars.next().map_or_else(String::new, |first| {
                    first.to_uppercase().collect::<String>() + chars.as_str()
                })
            }
        }
    }
}

impl fmt::Display for ReasoningEffort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ReasoningEffort {
    type Err = std::convert::Infallible;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        // Known levels are matched case-insensitively, but an unknown one is
        // kept exactly as written. Lowercasing it here would change the value
        // sent to a backend that may well be case-sensitive about a level
        // this build has never heard of.
        Ok(match value.trim().to_ascii_lowercase().as_str() {
            "none" => Self::None,
            "minimal" => Self::Minimal,
            "low" => Self::Low,
            "medium" => Self::Medium,
            "high" => Self::High,
            "xhigh" => Self::XHigh,
            "max" => Self::Max,
            "ultra" => Self::Ultra,
            "persistent" => Self::Persistent,
            _ => Self::Other(value.trim().to_string()),
        })
    }
}

impl From<String> for ReasoningEffort {
    fn from(value: String) -> Self {
        value.parse().unwrap_or(Self::Medium)
    }
}

impl From<ReasoningEffort> for String {
    fn from(value: ReasoningEffort) -> Self {
        value.as_str().to_string()
    }
}

/// What a model accepts.
///
/// The editor renders from this rather than from the API type, and the
/// client checks it before putting a parameter on the wire. A field being
/// false means the endpoint rejects or ignores that parameter, not that the
/// user merely has no opinion about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelCapabilities {
    /// Temperature and top-p.
    pub sampling: bool,
    /// An output token ceiling.
    pub max_tokens: bool,
    /// A thinking budget expressed in tokens.
    ///
    /// Independent of [`Self::reasoning`]. A model may take a budget, a set
    /// of levels, both, or neither, so neither field implies the other.
    pub thinking_budget: bool,
    /// Named reasoning levels, if the model takes any.
    pub reasoning: ReasoningSupport,
}

/// The reasoning levels a model offers, and what comes back.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReasoningSupport {
    /// Levels the model accepts. Empty means it takes no effort at all.
    pub levels: Vec<ReasoningEffort>,
    /// Whether the model returns reasoning summaries when asked for one.
    pub summary: bool,
}

impl ModelCapabilities {
    /// Whether this model takes a reasoning effort at all.
    #[must_use]
    pub const fn takes_reasoning_effort(&self) -> bool {
        !self.reasoning.levels.is_empty()
    }

    /// The levels this model offers.
    #[must_use]
    pub fn reasoning_efforts(&self) -> &[ReasoningEffort] {
        &self.reasoning.levels
    }

    /// Whether asking for a reasoning summary is worthwhile.
    #[must_use]
    pub const fn reasoning_summary(&self) -> bool {
        self.reasoning.summary
    }

    /// Whether `effort` is one this model accepts.
    #[must_use]
    pub fn accepts(&self, effort: &ReasoningEffort) -> bool {
        self.reasoning.levels.contains(effort)
    }

    /// A conventional endpoint: sampling and a token ceiling, no reasoning
    /// controls.
    #[must_use]
    pub fn sampling_only() -> Self {
        Self {
            sampling: true,
            max_tokens: true,
            thinking_budget: false,
            reasoning: ReasoningSupport::default(),
        }
    }

    /// Anthropic-style: sampling, a ceiling, and a thinking budget in
    /// tokens. These models take a count, not a level.
    #[must_use]
    pub fn token_budget_thinking() -> Self {
        Self {
            sampling: true,
            max_tokens: true,
            thinking_budget: true,
            reasoning: ReasoningSupport::default(),
        }
    }

    /// The codex Responses endpoint: no sampling, no ceiling, an effort
    /// ladder instead.
    ///
    /// The endpoint answers `Unsupported parameter: temperature`, and the
    /// same for `max_output_tokens`, so both are absent here rather than
    /// offered and discarded.
    #[must_use]
    pub fn codex_reasoning() -> Self {
        Self {
            sampling: false,
            max_tokens: false,
            thinking_budget: false,
            reasoning: ReasoningSupport {
                // Provisional. Medium, high and max have been seen accepted
                // by the live endpoint; the rest are offered because a
                // refused level fails loudly, whereas withholding one leaves
                // the user unable to ask for the mode they pay for. Every
                // name here appears in the protocol the codex client speaks,
                // so none is invented. Reading the backend's per-model list
                // replaces this guess.
                levels: vec![
                    // `none` is how reasoning is turned off on these models.
                    // There is no separate switch, so leaving it out would
                    // make "off" unreachable.
                    ReasoningEffort::None,
                    ReasoningEffort::Low,
                    ReasoningEffort::Medium,
                    ReasoningEffort::High,
                    ReasoningEffort::XHigh,
                    ReasoningEffort::Max,
                    // Ultra and Persistent are deliberately absent. Ultra is
                    // sent as `max`, so offering both would be two names for
                    // one request. Persistent is sent as `disabled`, which is
                    // not a rung on an effort ladder and would read as the
                    // opposite of what it does. Both remain in the type so a
                    // profile carrying one still works.
                ],
                summary: true,
            },
        }
    }
}

/// Read a level out of stored text.
///
/// Blank means the profile carries no choice, which stays absent rather than
/// becoming a level nobody picked. Sending an explicit default would record a
/// decision the user never made and pin the turn if the backend's own default
/// ever moves.
#[must_use]
pub fn effort_from_stored(stored: &str) -> Option<ReasoningEffort> {
    if stored.trim().is_empty() {
        None
    } else {
        stored.parse().ok()
    }
}

/// What a provider's models accept.
///
/// Keyed on the provider id rather than the editor's API type so the domain
/// layer does not depend on the UI, and so a capability answer is available
/// wherever a profile is.
///
/// This is the local answer. The codex backend publishes a per-model list
/// carrying `supported_reasoning_efforts` and a summary flag, versioned by
/// the `x-models-etag` header we already receive; reading it would replace
/// the `openai-codex` arm with the model's own declaration rather than this
/// provider-wide assumption.
#[must_use]
pub fn capabilities_for(provider_id: &str) -> ModelCapabilities {
    match provider_id {
        "anthropic" => ModelCapabilities::token_budget_thinking(),
        "openai-codex" | "open-responses" => ModelCapabilities::codex_reasoning(),
        _ => ModelCapabilities::sampling_only(),
    }
}

#[cfg(test)]
mod tests {
    use super::{ModelCapabilities, ReasoningEffort};

    #[test]
    fn every_known_level_survives_a_string_round_trip() {
        for effort in ReasoningEffort::known() {
            let wire = effort.as_str().to_string();
            assert_eq!(
                wire.parse::<ReasoningEffort>().expect("infallible"),
                effort,
                "{wire} did not round trip"
            );
        }
    }

    #[test]
    fn the_ladder_reaches_past_high() {
        // The whole point of the change: high was the ceiling when effort
        // was derived from a token budget.
        let known = ReasoningEffort::known();
        let wire: Vec<&str> = known.iter().map(ReasoningEffort::as_str).collect();
        for level in ["xhigh", "max", "ultra"] {
            assert!(wire.contains(&level), "{level} is not reachable: {wire:?}");
        }
    }

    #[test]
    fn an_unknown_level_is_preserved_rather_than_dropped() {
        // A level added upstream has to reach the wire without a release,
        // and must not silently become something else.
        let parsed: ReasoningEffort = "brand_new_level".parse().expect("infallible");
        assert_eq!(
            parsed,
            ReasoningEffort::Other("brand_new_level".to_string())
        );
        assert_eq!(parsed.as_str(), "brand_new_level");
    }

    #[test]
    fn parsing_is_case_and_space_insensitive() {
        assert_eq!(
            "  XHigh ".parse::<ReasoningEffort>().expect("infallible"),
            ReasoningEffort::XHigh
        );
    }

    #[test]
    fn serde_uses_the_wire_string_not_the_variant_name() {
        let json = serde_json::to_string(&ReasoningEffort::XHigh).expect("serialize");
        assert_eq!(json, "\"xhigh\"");
        let back: ReasoningEffort = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, ReasoningEffort::XHigh);
    }

    #[test]
    fn an_unknown_level_deserializes_without_error() {
        let back: ReasoningEffort = serde_json::from_str("\"future\"").expect("deserialize");
        assert_eq!(back, ReasoningEffort::Other("future".to_string()));
    }

    #[test]
    fn known_levels_are_not_offered_as_other() {
        assert!(
            !ReasoningEffort::known()
                .iter()
                .any(|effort| matches!(effort, ReasoningEffort::Other(_))),
            "Other carries a foreign value and is not a choice"
        );
    }

    #[test]
    fn a_budget_model_and_an_effort_model_are_described_differently() {
        // These are separate axes. A model taking a token budget does not
        // thereby take an effort level, and the reverse.
        let budget = ModelCapabilities::token_budget_thinking();
        assert!(budget.thinking_budget);
        assert!(!budget.takes_reasoning_effort());

        let effort = ModelCapabilities::codex_reasoning();
        assert!(!effort.thinking_budget);
        assert!(effort.takes_reasoning_effort());
    }

    #[test]
    fn the_codex_endpoint_refuses_sampling_and_a_token_ceiling() {
        let caps = ModelCapabilities::codex_reasoning();
        assert!(!caps.sampling, "the endpoint rejects temperature");
        assert!(!caps.max_tokens, "the endpoint rejects max_output_tokens");
    }

    #[test]
    fn a_model_only_accepts_the_levels_it_declares() {
        let caps = ModelCapabilities::codex_reasoning();
        assert!(caps.accepts(&ReasoningEffort::XHigh));
        assert!(caps.accepts(&ReasoningEffort::Max));
        assert!(!caps.accepts(&ReasoningEffort::Ultra), "ultra is max");
        assert!(
            !caps.accepts(&ReasoningEffort::Persistent),
            "persistent is not an effort tier"
        );
        assert!(
            !caps.accepts(&ReasoningEffort::Other("invented".to_string())),
            "only declared levels are offered"
        );
    }
}

#[cfg(test)]
mod lookup_tests {
    use super::{capabilities_for, ReasoningEffort};

    #[test]
    fn codex_offers_an_effort_ladder_and_no_sampling() {
        let caps = capabilities_for("openai-codex");
        assert!(caps.takes_reasoning_effort());
        assert!(caps.accepts(&ReasoningEffort::XHigh));
        assert!(!caps.sampling);
        assert!(!caps.thinking_budget, "effort is not a token budget");
    }

    #[test]
    fn anthropic_keeps_its_token_budget_and_gains_no_effort_control() {
        let caps = capabilities_for("anthropic");
        assert!(caps.thinking_budget);
        assert!(caps.sampling);
        assert!(
            !caps.takes_reasoning_effort(),
            "a budget model is not an effort model"
        );
    }

    #[test]
    fn stored_text_that_is_absent_stays_absent() {
        // No choice recorded is not the same as choosing the default. Turning
        // one into the other would pin a level nobody picked.
        for blank in ["", "   "] {
            assert_eq!(
                super::effort_from_stored(blank),
                None,
                "blank {blank:?} should not become a level"
            );
        }
    }

    #[test]
    fn stored_text_that_names_a_level_is_honoured() {
        assert_eq!(
            super::effort_from_stored("xhigh"),
            Some(ReasoningEffort::XHigh)
        );
    }

    #[test]
    fn the_two_local_names_are_translated_before_sending() {
        // Upstream keeps these as settings names and translates them on the
        // request. Sending either verbatim would be a value the endpoint does
        // not know.
        assert_eq!(ReasoningEffort::Ultra.wire_value(), "max");
        assert_eq!(ReasoningEffort::Persistent.wire_value(), "disabled");
    }

    #[test]
    fn every_other_level_is_sent_under_its_own_name() {
        for effort in ReasoningEffort::known() {
            if matches!(effort, ReasoningEffort::Ultra | ReasoningEffort::Persistent) {
                continue;
            }
            assert_eq!(
                effort.wire_value(),
                effort.as_str(),
                "{effort:?} should not be translated"
            );
        }
    }

    #[test]
    fn an_unknown_level_keeps_its_original_spelling() {
        // Lowercasing an unrecognised level would change the value sent to a
        // backend that may be case-sensitive about it.
        let parsed: ReasoningEffort = "Future_Level".parse().expect("infallible");
        assert_eq!(parsed.as_str(), "Future_Level");
    }

    #[test]
    fn an_unknown_provider_gets_the_conventional_shape() {
        let caps = capabilities_for("something-nobody-has-heard-of");
        assert!(caps.sampling);
        assert!(caps.max_tokens);
        assert!(!caps.takes_reasoning_effort());
    }
}
