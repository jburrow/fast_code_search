#![cfg(feature = "ml-models")]
//! Model download and caching for semantic search
//!
//! Downloads ONNX models and tokenizers from HuggingFace Hub
//! and caches them locally for reuse.

use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tracing::{debug, info, warn};

const MAX_RETRIES: u32 = 3;
const RETRY_DELAY_SECS: u64 = 2;

/// Model metadata
#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub name: String,
    pub onnx_url: String,
    pub tokenizer_url: String,
    pub config_url: String,
    pub expected_sha256: Option<String>,
}

impl ModelInfo {
    /// Get CodeBERT model info (ONNX-optimized version)
    ///
    /// Uses the ONNX Community's converted model from HuggingFace.
    /// This model is properly optimized for ONNX Runtime inference.
    pub fn codebert() -> Self {
        Self {
            name: "microsoft/codebert-base".to_string(),
            // Using onnx-community's ONNX-converted model which is properly exported
            // The original microsoft/codebert-base doesn't have ONNX models
            // Note: Xenova/codebert-base was deprecated/removed, now using onnx-community
            onnx_url: "https://huggingface.co/onnx-community/codebert-base-ONNX/resolve/main/onnx/model.onnx"
                .to_string(),
            tokenizer_url:
                "https://huggingface.co/onnx-community/codebert-base-ONNX/resolve/main/tokenizer.json"
                    .to_string(),
            config_url: "https://huggingface.co/onnx-community/codebert-base-ONNX/resolve/main/config.json"
                .to_string(),
            expected_sha256: None,
        }
    }

    /// Alternative: UniXcoder model (newer, multilingual)
    #[allow(dead_code)]
    pub fn unixcoder() -> Self {
        Self {
            name: "microsoft/unixcoder-base".to_string(),
            // Using community ONNX-converted model
            // Note: model.onnx is in root, not in onnx/ subfolder for this repo
            onnx_url:
                "https://huggingface.co/sailesh27/unixcoder-base-onnx/resolve/main/model.onnx"
                    .to_string(),
            tokenizer_url:
                "https://huggingface.co/sailesh27/unixcoder-base-onnx/resolve/main/tokenizer.json"
                    .to_string(),
            config_url:
                "https://huggingface.co/sailesh27/unixcoder-base-onnx/resolve/main/config.json"
                    .to_string(),
            expected_sha256: None,
        }
    }
}

/// Model downloader with progress tracking and retry logic
pub struct ModelDownloader {
    cache_dir: PathBuf,
}

impl ModelDownloader {
    /// Create new downloader with cache directory
    pub fn new(cache_dir: PathBuf) -> Self {
        Self { cache_dir }
    }

    /// Get model directory path
    fn model_dir(&self, model_name: &str) -> PathBuf {
        let safe_name = model_name.replace('/', "-");
        self.cache_dir.join("models").join(safe_name)
    }

    /// Check if model is cached
    pub fn is_cached(&self, model_name: &str) -> bool {
        let model_dir = self.model_dir(model_name);
        model_dir.join("model.onnx").exists()
            && model_dir.join("tokenizer.json").exists()
            && model_dir.join("config.json").exists()
    }

    /// Get path to cached model
    pub fn get_model_path(&self, model_name: &str) -> Result<PathBuf> {
        let model_dir = self.model_dir(model_name);
        if !self.is_cached(model_name) {
            anyhow::bail!(
                "Model '{}' not found in cache at {}. Run with ML feature enabled to download.",
                model_name,
                model_dir.display()
            );
        }
        Ok(model_dir)
    }

    /// Download model if not cached
    pub fn ensure_model(&self, model_info: &ModelInfo) -> Result<PathBuf> {
        if self.is_cached(&model_info.name) {
            info!(model = %model_info.name, "Model found in cache");
            return self.get_model_path(&model_info.name);
        }

        info!(model = %model_info.name, "Downloading model files");
        self.download_model(model_info)?;
        self.get_model_path(&model_info.name)
    }

    /// Download model from HuggingFace with retry logic
    fn download_model(&self, model_info: &ModelInfo) -> Result<()> {
        let model_dir = self.model_dir(&model_info.name);
        fs::create_dir_all(&model_dir).with_context(|| {
            format!(
                "Failed to create model cache directory: {}. Check permissions.",
                model_dir.display()
            )
        })?;

        info!("Downloading CodeBERT ONNX model (~500MB). This is a one-time operation.");
        info!("Files will be cached at: {}", model_dir.display());

        // Download ONNX model (largest file)
        self.download_file_with_retry(
            &model_info.onnx_url,
            &model_dir.join("model.onnx"),
            "ONNX model",
        )?;

        // Download tokenizer
        self.download_file_with_retry(
            &model_info.tokenizer_url,
            &model_dir.join("tokenizer.json"),
            "Tokenizer",
        )?;

        // Download config
        self.download_file_with_retry(
            &model_info.config_url,
            &model_dir.join("config.json"),
            "Config",
        )?;

        // Verify checksum if provided (or via env override)
        let expected_hash = self.resolve_expected_sha256(model_info)?;
        if let Some(expected_hash) = expected_hash {
            info!("Verifying model checksum...");
            let model_path = model_dir.join("model.onnx");
            if !self.verify_checksum(&model_path, &expected_hash)? {
                anyhow::bail!(
                    "Model checksum verification failed. Downloaded file may be corrupted. \
                     Please delete {} and try again.",
                    model_path.display()
                );
            }
            info!("Checksum verification passed");
        } else {
            warn!(
                "No SHA256 checksum configured for model '{}'. \
                 Set FCS_MODEL_SHA256 or FCS_MODEL_SHA256_{} to enable verification.",
                model_info.name,
                Self::env_model_suffix(&model_info.name)
            );
        }

        info!(
            model = %model_info.name,
            path = %model_dir.display(),
            "Model downloaded and cached successfully"
        );
        Ok(())
    }

    /// Download a file from URL to path with retry logic and progress bar
    fn download_file_with_retry(&self, url: &str, path: &Path, description: &str) -> Result<()> {
        for attempt in 1..=MAX_RETRIES {
            match self.download_file(url, path, description) {
                Ok(()) => return Ok(()),
                Err(e) if attempt < MAX_RETRIES => {
                    warn!(
                        "Download attempt {}/{} failed for {}: {}. Retrying in {}s...",
                        attempt, MAX_RETRIES, description, e, RETRY_DELAY_SECS
                    );
                    std::thread::sleep(Duration::from_secs(RETRY_DELAY_SECS));
                }
                Err(e) => {
                    return Err(e).with_context(|| {
                        format!(
                            "Failed to download {} after {} attempts. \
                             Check your internet connection and firewall settings.",
                            description, MAX_RETRIES
                        )
                    });
                }
            }
        }
        unreachable!()
    }

    /// Download a file from URL to path with progress indicator
    fn download_file(&self, url: &str, path: &Path, description: &str) -> Result<()> {
        debug!(url = %url, path = %path.display(), "Downloading {}", description);

        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(300)) // 5 minute timeout
            .build()
            .context("Failed to create HTTP client")?;

        let response = client
            .get(url)
            .send()
            .with_context(|| format!("Failed to connect to {}", url))?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Failed to download {}: HTTP {} from {}. \
                 The file may not exist or you may not have access.",
                description,
                response.status(),
                url
            );
        }

        // Get content length for progress bar
        let total_size = response.content_length().unwrap_or(0);

        // Create progress bar
        let pb = if total_size > 0 {
            let pb = ProgressBar::new(total_size);
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                    .unwrap()
                    .progress_chars("#>-"),
            );
            pb.set_message(format!("Downloading {}", description));
            Some(pb)
        } else {
            info!("Downloading {} (size unknown)...", description);
            None
        };

        // Download with progress
        let bytes = response
            .bytes()
            .with_context(|| format!("Failed to read response from {}", url))?;

        if let Some(pb) = &pb {
            pb.set_position(bytes.len() as u64);
            pb.finish_with_message(format!("Downloaded {}", description));
        }

        // Write to file
        let mut file = fs::File::create(path)
            .with_context(|| format!("Failed to create file: {}", path.display()))?;

        file.write_all(&bytes)
            .with_context(|| format!("Failed to write file: {}", path.display()))?;

        debug!(
            path = %path.display(),
            size = bytes.len(),
            "{} downloaded successfully",
            description
        );
        Ok(())
    }

    /// Verify file checksum using SHA256
    fn verify_checksum(&self, path: &Path, expected_hash: &str) -> Result<bool> {
        let bytes =
            fs::read(path).with_context(|| format!("Failed to read file: {}", path.display()))?;

        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let hash = format!("{:x}", hasher.finalize());

        Ok(hash.eq_ignore_ascii_case(expected_hash))
    }

    fn resolve_expected_sha256(&self, model_info: &ModelInfo) -> Result<Option<String>> {
        if let Some(expected) = model_info.expected_sha256.as_ref() {
            return Self::normalize_sha256(expected).map(Some);
        }

        let env_hash = std::env::var("FCS_MODEL_SHA256")
            .ok()
            .filter(|value| !value.trim().is_empty());
        if let Some(expected) = env_hash {
            return Self::normalize_sha256(&expected).map(Some);
        }

        let scoped_env = format!("FCS_MODEL_SHA256_{}", Self::env_model_suffix(&model_info.name));
        let scoped_hash = std::env::var(scoped_env)
            .ok()
            .filter(|value| !value.trim().is_empty());

        if let Some(expected) = scoped_hash {
            return Self::normalize_sha256(&expected).map(Some);
        }

        Ok(None)
    }

    fn normalize_sha256(value: &str) -> Result<String> {
        let trimmed = value.trim();
        if trimmed.len() != 64 || !trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
            anyhow::bail!(
                "Invalid SHA256 value '{}'. Expected 64 hex characters.",
                trimmed
            );
        }
        Ok(trimmed.to_lowercase())
    }

    fn env_model_suffix(model_name: &str) -> String {
        model_name
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect::<String>()
            .to_uppercase()
    }
}

/// Get default cache directory
pub fn default_cache_dir() -> Result<PathBuf> {
    let cache_dir = dirs::cache_dir()
        .context(
            "Failed to determine cache directory. \
             On Linux, ensure $HOME is set. On Windows, ensure %LOCALAPPDATA% is set.",
        )?
        .join("fast_code_search_semantic");

    Ok(cache_dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_model_info_codebert() {
        let info = ModelInfo::codebert();
        assert_eq!(info.name, "microsoft/codebert-base");
        assert!(info.onnx_url.contains("huggingface.co"));
        assert!(info.onnx_url.contains("Xenova")); // Should use ONNX-converted version
    }

    #[test]
    fn test_model_info_unixcoder() {
        let info = ModelInfo::unixcoder();
        assert_eq!(info.name, "microsoft/unixcoder-base");
        assert!(info.onnx_url.contains("Xenova"));
    }

    #[test]
    fn test_model_dir_path() {
        let temp = tempdir().unwrap();
        let downloader = ModelDownloader::new(temp.path().to_path_buf());

        let model_dir = downloader.model_dir("microsoft/codebert-base");
        assert!(model_dir
            .to_string_lossy()
            .contains("microsoft-codebert-base"));
    }

    #[test]
    fn test_is_cached_not_found() {
        let temp = tempdir().unwrap();
        let downloader = ModelDownloader::new(temp.path().to_path_buf());

        assert!(!downloader.is_cached("microsoft/codebert-base"));
    }

    #[test]
    fn test_default_cache_dir() {
        let cache_dir = default_cache_dir().unwrap();
        assert!(cache_dir
            .to_string_lossy()
            .contains("fast_code_search_semantic"));
    }

    #[test]
    fn test_get_model_path_not_cached() {
        let temp = tempdir().unwrap();
        let downloader = ModelDownloader::new(temp.path().to_path_buf());

        let result = downloader.get_model_path("microsoft/codebert-base");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("not found in cache"));
    }

    // Note: Actual download tests are skipped in CI as they require network access
    // and download large files. Run manually for testing with:
    // cargo test --features ml-models test_download_model -- --ignored
    #[test]
    #[ignore]
    fn test_download_model() {
        let temp = tempdir().unwrap();
        let downloader = ModelDownloader::new(temp.path().to_path_buf());
        let model_info = ModelInfo::codebert();

        let result = downloader.ensure_model(&model_info);
        assert!(result.is_ok());
        assert!(downloader.is_cached(&model_info.name));
    }
}
