use serde::{Deserialize, Serialize};

/// Export format for saving a conversation transcript to disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ConversationExportFormat {
    Json,
    Txt,
    #[default]
    Md,
}

impl ConversationExportFormat {
    #[must_use]
    pub const fn as_setting_value(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Txt => "txt",
            Self::Md => "md",
        }
    }

    #[must_use]
    pub const fn extension(self) -> &'static str {
        self.as_setting_value()
    }

    #[must_use]
    pub const fn display_label(self) -> &'static str {
        match self {
            Self::Json => "JSON",
            Self::Txt => "TXT",
            Self::Md => "MD",
        }
    }

    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Md => Self::Txt,
            Self::Txt => Self::Json,
            Self::Json => Self::Md,
        }
    }

    #[must_use]
    pub const fn from_setting_value(value: &str) -> Option<Self> {
        if value.eq_ignore_ascii_case("json") {
            Some(Self::Json)
        } else if value.eq_ignore_ascii_case("txt") {
            Some(Self::Txt)
        } else if value.eq_ignore_ascii_case("md") || value.eq_ignore_ascii_case("markdown") {
            Some(Self::Md)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ConversationExportFormat;

    #[test]
    fn default_is_markdown() {
        assert_eq!(
            ConversationExportFormat::default(),
            ConversationExportFormat::Md
        );
    }

    #[test]
    fn cycle_order_is_md_txt_json() {
        assert_eq!(
            ConversationExportFormat::Md.next(),
            ConversationExportFormat::Txt
        );
        assert_eq!(
            ConversationExportFormat::Txt.next(),
            ConversationExportFormat::Json
        );
        assert_eq!(
            ConversationExportFormat::Json.next(),
            ConversationExportFormat::Md
        );
    }

    #[test]
    fn parser_accepts_supported_values_case_insensitively() {
        assert_eq!(
            ConversationExportFormat::from_setting_value("md"),
            Some(ConversationExportFormat::Md)
        );
        assert_eq!(
            ConversationExportFormat::from_setting_value("MARKDOWN"),
            Some(ConversationExportFormat::Md)
        );
        assert_eq!(
            ConversationExportFormat::from_setting_value("txt"),
            Some(ConversationExportFormat::Txt)
        );
        assert_eq!(
            ConversationExportFormat::from_setting_value("JSON"),
            Some(ConversationExportFormat::Json)
        );
        assert_eq!(ConversationExportFormat::from_setting_value("pdf"), None);
    }
}
