//! Theme catalog loading for GPUI theme files.
//!
//! @plan ISSUE12

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeKind {
    Dark,
    Light,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ThemeDefinition {
    pub name: String,
    pub slug: String,
    pub kind: ThemeKind,
    pub colors: ThemeColors,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ThemeColors {
    pub background: String,
    pub panel: PanelColors,
    pub text: TextColors,
    pub input: InputColors,
    pub status: StatusColors,
    pub accent: AccentColors,
    pub selection: SelectionColors,
    pub diff: DiffColors,
    pub scrollbar: ScrollbarColors,
    pub message: MessageColors,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PanelColors {
    pub bg: String,
    pub border: String,
    #[serde(rename = "headerBg")]
    pub header_bg: String,
    #[serde(rename = "headerFg")]
    pub header_fg: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TextColors {
    pub primary: String,
    pub muted: String,
    pub user: String,
    pub responder: String,
    pub thinking: String,
    pub tool: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct InputColors {
    pub bg: String,
    pub fg: String,
    #[serde(rename = "inputHint")]
    pub hint: String,
    pub border: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct StatusColors {
    pub fg: String,
    pub muted: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AccentColors {
    pub primary: String,
    pub secondary: String,
    pub warning: String,
    pub error: String,
    pub success: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SelectionColors {
    pub fg: String,
    pub bg: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DiffColors {
    #[serde(rename = "addedBg")]
    pub added_bg: String,
    #[serde(rename = "addedFg")]
    pub added_fg: String,
    #[serde(rename = "removedBg")]
    pub removed_bg: String,
    #[serde(rename = "removedFg")]
    pub removed_fg: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ScrollbarColors {
    pub thumb: String,
    pub track: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct MessageColors {
    #[serde(rename = "userBorder")]
    pub user_border: String,
    #[serde(rename = "systemBorder")]
    pub system_border: String,
    #[serde(rename = "systemText")]
    pub system_text: String,
    #[serde(rename = "systemBg")]
    pub system_bg: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ThemeCatalogError {
    #[error("theme directory not found: {0}")]
    ThemeDirectoryMissing(PathBuf),

    #[error("failed to read theme directory {path}: {source}")]
    ReadThemeDirectory {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to read theme file {path}: {source}")]
    ReadThemeFile {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to parse theme file {path}: {source}")]
    ParseThemeFile {
        path: PathBuf,
        source: serde_json::Error,
    },

    #[error("duplicate theme slug '{slug}' in files {first_path:?} and {second_path:?}")]
    DuplicateSlug {
        slug: String,
        first_path: PathBuf,
        second_path: PathBuf,
    },
}

#[derive(Debug, Clone, Default)]
pub struct ThemeCatalog {
    themes_by_slug: HashMap<String, ThemeDefinition>,
    source_paths_by_slug: HashMap<String, PathBuf>,
    ordered_slugs: Vec<String>,
}

impl ThemeCatalog {
    /// Load a theme catalog from a directory of JSON theme files.
    ///
    /// # Errors
    ///
    /// Returns a [`ThemeCatalogError`] if the directory is missing, unreadable,
    /// or any JSON file fails to parse or contains a duplicate slug.
    pub fn load_from_dir(dir: &Path) -> Result<Self, ThemeCatalogError> {
        if !dir.exists() {
            return Err(ThemeCatalogError::ThemeDirectoryMissing(dir.to_path_buf()));
        }

        let mut entries = fs::read_dir(dir)
            .map_err(|source| ThemeCatalogError::ReadThemeDirectory {
                path: dir.to_path_buf(),
                source,
            })?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
            .collect::<Vec<_>>();

        entries.sort();

        let mut catalog = Self::default();
        for path in entries {
            let contents =
                fs::read_to_string(&path).map_err(|source| ThemeCatalogError::ReadThemeFile {
                    path: path.clone(),
                    source,
                })?;

            let parsed: ThemeDefinition = serde_json::from_str(&contents).map_err(|source| {
                ThemeCatalogError::ParseThemeFile {
                    path: path.clone(),
                    source,
                }
            })?;

            if let Some(first_path) = catalog.source_paths_by_slug.get(&parsed.slug) {
                return Err(ThemeCatalogError::DuplicateSlug {
                    slug: parsed.slug.clone(),
                    first_path: first_path.clone(),
                    second_path: path,
                });
            }

            catalog.ordered_slugs.push(parsed.slug.clone());
            catalog
                .source_paths_by_slug
                .insert(parsed.slug.clone(), path.clone());
            catalog.themes_by_slug.insert(parsed.slug.clone(), parsed);
        }

        Ok(catalog)
    }

    #[must_use]
    pub fn bundled_theme_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets")
            .join("themes")
    }

    /// Load the catalog from the bundled `assets/themes` directory.
    ///
    /// # Errors
    ///
    /// Returns a [`ThemeCatalogError`] if the assets directory is missing or
    /// any bundled theme file is malformed.
    pub fn load_bundled() -> Result<Self, ThemeCatalogError> {
        Self::load_from_dir(&Self::bundled_theme_dir())
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.ordered_slugs.len()
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.ordered_slugs.is_empty()
    }

    #[must_use]
    pub fn get(&self, slug: &str) -> Option<&ThemeDefinition> {
        self.themes_by_slug.get(slug)
    }

    #[must_use]
    pub fn slugs(&self) -> Vec<&str> {
        self.ordered_slugs.iter().map(String::as_str).collect()
    }
}
