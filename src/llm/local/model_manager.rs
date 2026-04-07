//! Model download and cache management.
//!
//! Handles downloading models from `HuggingFace`, caching them locally,
//! verifying integrity, and managing the cache directory.

use std::path::PathBuf;

use crate::llm::local::error::{LocalModelError, LocalModelResult};

/// Default model configuration.
pub const DEFAULT_MODEL_REPO: &str = "lmstudio-community/Qwen3.5-4B-GGUF";
pub const DEFAULT_MODEL_FILE: &str = "Qwen3.5-4B-Q4_K_M.gguf";
pub const DEFAULT_MODEL_DISPLAY_NAME: &str = "Qwen3.5-4B (Q4_K_M)";
pub const DEFAULT_MODEL_SIZE_BYTES: u64 = 2_710_000_000; // ~2.71 GB

/// Local model manager for download and cache operations.
pub struct LocalModelManager {
    cache_dir: PathBuf,
}

impl LocalModelManager {
    /// Create a new model manager.
    ///
    /// Initializes the cache directory if it doesn't exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the cache directory cannot be created.
    pub fn new() -> LocalModelResult<Self> {
        let cache_dir = dirs::cache_dir()
            .ok_or_else(|| {
                LocalModelError::CacheDirError("Could not determine cache directory".to_string())
            })?
            .join("PersonalAgent")
            .join("models");

        std::fs::create_dir_all(&cache_dir)?;

        Ok(Self { cache_dir })
    }

    /// Get the path to the cache directory.
    #[must_use]
    pub fn cache_dir(&self) -> &std::path::Path {
        &self.cache_dir
    }

    /// Get the path to the default model file.
    #[must_use]
    pub fn model_path(&self) -> PathBuf {
        self.cache_dir.join(DEFAULT_MODEL_FILE)
    }

    /// Check if the default model is downloaded.
    #[must_use]
    pub fn is_model_downloaded(&self) -> bool {
        self.model_path().exists()
    }

    /// Get the model file size if it exists.
    #[must_use]
    pub fn model_size(&self) -> Option<u64> {
        self.model_path().metadata().ok().map(|m| m.len())
    }

    /// Get the model size as a human-readable string.
    #[must_use]
    pub fn model_size_human(&self) -> Option<String> {
        self.model_size().map(format_bytes)
    }

    /// Get the expected download size in bytes.
    #[must_use]
    pub const fn expected_download_size(&self) -> u64 {
        DEFAULT_MODEL_SIZE_BYTES
    }

    /// Get the expected download size as a human-readable string.
    #[must_use]
    pub fn expected_download_size_human(&self) -> String {
        format_bytes(DEFAULT_MODEL_SIZE_BYTES)
    }

    /// Download the default model from `HuggingFace`.
    ///
    /// This uses HTTP range requests for resume support and reports
    /// progress via the provided callback.
    ///
    /// # Arguments
    ///
    /// * `on_progress` - Callback receiving (`bytes_downloaded`, `total_bytes`)
    ///
    /// # Errors
    ///
    /// Returns an error if the download fails or is interrupted.
    pub async fn download_default_model<F>(&self, on_progress: F) -> LocalModelResult<PathBuf>
    where
        F: Fn(u64, u64) + Send + 'static,
    {
        let url = format!(
            "https://huggingface.co/{DEFAULT_MODEL_REPO}/resolve/main/{DEFAULT_MODEL_FILE}"
        );

        let target_path = self.model_path();
        let temp_path = target_path.with_extension("tmp");

        // Check for partial download (resume support)
        let start_byte = if temp_path.exists() {
            temp_path.metadata()?.len()
        } else {
            0
        };

        tracing::info!(
            "Downloading model from {} (starting at {} bytes)",
            url,
            start_byte
        );

        download_file_async(&url, &temp_path, start_byte, on_progress).await?;

        // Move temp file to final location
        std::fs::rename(&temp_path, &target_path)?;

        tracing::info!("Model downloaded to {}", target_path.display());
        Ok(target_path)
    }

    /// Delete the downloaded model.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be deleted.
    pub fn delete_model(&self) -> LocalModelResult<()> {
        let path = self.model_path();
        if path.exists() {
            std::fs::remove_file(path)?;
            tracing::info!("Deleted local model");
        }
        Ok(())
    }

    /// Verify the model file integrity by checking its size.
    ///
    /// A full SHA256 verification would be ideal but is slow for large files.
    /// We check the file size matches expected and verify the GGUF header.
    ///
    /// # Errors
    ///
    /// Returns an error if the file is corrupted or missing.
    pub fn verify_model(&self) -> LocalModelResult<()> {
        let path = self.model_path();

        if !path.exists() {
            return Err(LocalModelError::ModelNotFound(path));
        }

        let metadata = std::fs::metadata(&path)?;
        let size = metadata.len();

        // Check size is approximately correct (allow 5% tolerance for different quantizations)
        let expected = DEFAULT_MODEL_SIZE_BYTES;
        let min_size = expected * 95 / 100;
        let max_size = expected * 105 / 100;

        if size < min_size || size > max_size {
            return Err(LocalModelError::ModelCorrupted(format!(
                "File size {size} bytes is outside expected range {min_size}-{max_size}"
            )));
        }

        // Verify GGUF magic header
        let mut file = std::fs::File::open(&path)?;
        let mut header = [0u8; 4];
        std::io::Read::read_exact(&mut file, &mut header)?;

        // GGUF magic: "GGUF"
        if &header != b"GGUF" {
            return Err(LocalModelError::ModelCorrupted(
                "Invalid GGUF header".to_string(),
            ));
        }

        tracing::info!("Model verified successfully");
        Ok(())
    }
}

impl Default for LocalModelManager {
    fn default() -> Self {
        Self::new().expect("Failed to create LocalModelManager")
    }
}

/// Format bytes as human-readable string.
#[allow(clippy::cast_precision_loss)]
#[must_use]
pub fn format_bytes(bytes: u64) -> String {
    const GB: u64 = 1_073_741_824;
    const MB: u64 = 1_048_576;
    const KB: u64 = 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} bytes")
    }
}

/// Async file download with resume support.
async fn download_file_async<F>(
    url: &str,
    temp_path: &std::path::Path,
    start_byte: u64,
    on_progress: F,
) -> LocalModelResult<()>
where
    F: Fn(u64, u64) + Send + 'static,
{
    use futures::StreamExt;
    use std::io::Write;

    // Build client with timeout
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3600)) // 1 hour timeout
        .build()
        .map_err(|e| LocalModelError::DownloadFailed(e.to_string()))?;

    // Build request with range header for resume
    let mut request = client.get(url);
    if start_byte > 0 {
        request = request.header("Range", format!("bytes={start_byte}-"));
    }

    let response = request
        .send()
        .await
        .map_err(|e| LocalModelError::DownloadFailed(e.to_string()))?;

    if !response.status().is_success() && response.status() != reqwest::StatusCode::PARTIAL_CONTENT
    {
        return Err(LocalModelError::DownloadFailed(format!(
            "HTTP {}: {}",
            response.status(),
            response
                .status()
                .canonical_reason()
                .unwrap_or("Unknown error")
        )));
    }

    // Get total size
    let total_size = response
        .content_length()
        .unwrap_or(DEFAULT_MODEL_SIZE_BYTES);
    let total_with_resume = start_byte + total_size;

    // Open file for append if resuming
    let mut file = if start_byte > 0 {
        std::fs::OpenOptions::new().append(true).open(temp_path)?
    } else {
        std::fs::File::create(temp_path)?
    };

    // Download with progress using chunks
    let mut stream = response.bytes_stream();
    let mut downloaded = start_byte;
    let mut last_progress = 0u64;

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.map_err(|e| LocalModelError::DownloadFailed(e.to_string()))?;

        file.write_all(&chunk)
            .map_err(|e| LocalModelError::DownloadFailed(e.to_string()))?;

        downloaded += chunk.len() as u64;

        // Report progress every 1MB or at completion
        if downloaded - last_progress >= 1_000_000 || downloaded == total_with_resume {
            on_progress(downloaded, total_with_resume);
            last_progress = downloaded;
        }
    }

    file.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(500), "500 bytes");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1_048_576), "1.0 MB");
        assert_eq!(format_bytes(2_710_000_000), "2.52 GB");
    }

    #[test]
    fn test_model_manager_creation() {
        let manager = LocalModelManager::new();
        assert!(manager.is_ok());
    }

    #[test]
    fn test_model_path() {
        let manager = LocalModelManager::new().unwrap();
        let path = manager.model_path();
        assert!(path.ends_with(DEFAULT_MODEL_FILE));
    }
}
