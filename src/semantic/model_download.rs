//! Model download and caching for semantic search
//!
//! Downloads ONNX models and tokenizers from HuggingFace Hub
//! and caches them locally for reuse.

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Model metadata
#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub name: String,
    pub onnx_url: String,
    pub tokenizer_url: String,
    pub config_url: String,
}

impl ModelInfo {
    /// Get CodeBERT model info
    pub fn codebert() -> Self {
        Self {
            name: "microsoft/codebert-base".to_string(),
            onnx_url: "https://huggingface.co/microsoft/codebert-base/resolve/main/onnx/model.onnx".to_string(),
            tokenizer_url: "https://huggingface.co/microsoft/codebert-base/resolve/main/tokenizer.json".to_string(),
            config_url: "https://huggingface.co/microsoft/codebert-base/resolve/main/config.json".to_string(),
        }
    }
}

/// Model downloader
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
            anyhow::bail!("Model {} not found in cache", model_name);
        }
        Ok(model_dir)
    }

    /// Download model if not cached
    pub fn ensure_model(&self, model_info: &ModelInfo) -> Result<PathBuf> {
        if self.is_cached(&model_info.name) {
            info!(model = %model_info.name, "Model found in cache");
            return self.get_model_path(&model_info.name);
        }

        info!(model = %model_info.name, "Downloading model");
        self.download_model(model_info)?;
        self.get_model_path(&model_info.name)
    }

    /// Download model from HuggingFace
    fn download_model(&self, model_info: &ModelInfo) -> Result<()> {
        let model_dir = self.model_dir(&model_info.name);
        fs::create_dir_all(&model_dir)
            .with_context(|| format!("Failed to create model directory: {}", model_dir.display()))?;

        // Download ONNX model
        info!("Downloading ONNX model (~500MB, this may take a while)...");
        self.download_file(
            &model_info.onnx_url,
            &model_dir.join("model.onnx"),
        )?;

        // Download tokenizer
        info!("Downloading tokenizer...");
        self.download_file(
            &model_info.tokenizer_url,
            &model_dir.join("tokenizer.json"),
        )?;

        // Download config
        info!("Downloading config...");
        self.download_file(
            &model_info.config_url,
            &model_dir.join("config.json"),
        )?;

        info!(model = %model_info.name, path = %model_dir.display(), "Model downloaded successfully");
        Ok(())
    }

    /// Download a file from URL to path
    fn download_file(&self, url: &str, path: &Path) -> Result<()> {
        debug!(url = %url, path = %path.display(), "Downloading file");

        let response = reqwest::blocking::get(url)
            .with_context(|| format!("Failed to download from {}", url))?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to download {}: HTTP {}", url, response.status());
        }

        let bytes = response.bytes()
            .with_context(|| format!("Failed to read response from {}", url))?;

        let mut file = fs::File::create(path)
            .with_context(|| format!("Failed to create file: {}", path.display()))?;

        file.write_all(&bytes)
            .with_context(|| format!("Failed to write file: {}", path.display()))?;

        debug!(path = %path.display(), size = bytes.len(), "File downloaded");
        Ok(())
    }

    /// Verify file checksum (optional, for future use)
    #[allow(dead_code)]
    fn verify_checksum(&self, path: &Path, expected_hash: &str) -> Result<bool> {
        let bytes = fs::read(path)
            .with_context(|| format!("Failed to read file: {}", path.display()))?;

        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let hash = format!("{:x}", hasher.finalize());

        Ok(hash == expected_hash)
    }
}

/// Get default cache directory
pub fn default_cache_dir() -> Result<PathBuf> {
    let cache_dir = dirs::cache_dir()
        .context("Failed to get cache directory")?
        .join("fast_code_search_semantic");

    Ok(cache_dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_model_info() {
        let info = ModelInfo::codebert();
        assert_eq!(info.name, "microsoft/codebert-base");
        assert!(info.onnx_url.contains("huggingface.co"));
    }

    #[test]
    fn test_model_dir_path() {
        let temp = tempdir().unwrap();
        let downloader = ModelDownloader::new(temp.path().to_path_buf());
        
        let model_dir = downloader.model_dir("microsoft/codebert-base");
        assert!(model_dir.to_string_lossy().contains("microsoft-codebert-base"));
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
        assert!(cache_dir.to_string_lossy().contains("fast_code_search_semantic"));
    }

    // Note: Actual download tests are skipped in CI as they require network access
    // and download large files. Run manually for testing.
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
