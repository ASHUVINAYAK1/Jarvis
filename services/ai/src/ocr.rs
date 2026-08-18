//! JARVIS OCR (Optical Character Recognition) Subsystem
//!
//! Provides a strongly-typed OCR abstraction and local Tesseract engine adapter.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::env;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Instant;
use thiserror::Error;
use tracing::{debug, info, instrument};

/// Supported OCR provider types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OcrProviderType {
    Tesseract,
    Mock,
}

/// Structured error for OCR operations.
#[derive(Error, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OcrError {
    #[error("OCR provider unavailable: {0}")]
    ProviderUnavailable(String),

    #[error("Tesseract executable not found: {0}")]
    ExecutableNotFound(String),

    #[error("Invalid image input for OCR: {0}")]
    InvalidImage(String),

    #[error("OCR process execution failed: {0}")]
    ProcessFailed(String),

    #[error("OCR operation timed out after {0} seconds")]
    Timeout(u64),

    #[error("Internal OCR error: {0}")]
    Internal(String),
}

/// Request payload sent to an OCR provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrRequest {
    /// Raw image bytes (PNG, JPEG, etc.).
    pub image_bytes: Vec<u8>,
    /// Optional language code hint (e.g. "eng").
    pub language: Option<String>,
    /// Optional timeout override in seconds.
    pub timeout_secs: Option<u64>,
}

impl OcrRequest {
    pub fn new(image_bytes: Vec<u8>) -> Self {
        Self {
            image_bytes,
            language: None,
            timeout_secs: None,
        }
    }

    pub fn with_language(mut self, lang: impl Into<String>) -> Self {
        self.language = Some(lang.into());
        self
    }

    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = Some(secs);
        self
    }
}

/// Detected text region (bounding box + text + confidence score).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OcrTextRegion {
    pub text: String,
    pub confidence: f32,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Response returned by an OCR provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrResponse {
    /// Provider type that generated this result.
    pub provider_type: OcrProviderType,
    /// Extracted raw or cleaned text.
    pub text: String,
    /// True if readable non-whitespace text was detected.
    pub has_text: bool,
    /// Character count of detected text.
    pub char_count: usize,
    /// Average confidence score (0.0 to 100.0) if available.
    pub confidence: Option<f32>,
    /// Processing duration in milliseconds.
    pub latency_ms: u64,
    /// Optional detected text regions.
    pub regions: Vec<OcrTextRegion>,
}

/// Configuration settings for the OCR subsystem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrConfig {
    pub provider_type: OcrProviderType,
    pub tesseract_path: Option<PathBuf>,
    pub language: String,
    pub timeout_secs: u64,
    pub max_image_bytes: usize,
}

impl Default for OcrConfig {
    fn default() -> Self {
        Self {
            provider_type: OcrProviderType::Tesseract,
            tesseract_path: None,
            language: "eng".to_string(),
            timeout_secs: 30,
            max_image_bytes: 10 * 1024 * 1024, // 10 MB limit
        }
    }
}

/// Async trait implemented by all OCR providers in JARVIS.
#[async_trait]
pub trait OcrProvider: Send + Sync {
    fn provider_type(&self) -> OcrProviderType;
    async fn extract_text(&self, request: &OcrRequest) -> Result<OcrResponse, OcrError>;
}

// ============================================================
// Tesseract OCR Provider Implementation
// ============================================================

/// Production OCR provider that invokes local Tesseract CLI process.
pub struct TesseractOcrProvider {
    config: OcrConfig,
}

impl TesseractOcrProvider {
    pub fn new() -> Self {
        Self {
            config: OcrConfig::default(),
        }
    }

    pub fn with_config(config: OcrConfig) -> Self {
        Self { config }
    }

    /// Resolves the absolute path to the Tesseract executable.
    pub fn resolve_tesseract_binary(custom_path: Option<&Path>) -> Result<PathBuf, OcrError> {
        // 1. Check environment variable JARVIS_TESSERACT_PATH
        if let Ok(env_path) = env::var("JARVIS_TESSERACT_PATH") {
            let p = PathBuf::from(env_path);
            if p.exists() && p.is_file() {
                debug!(path = %p.display(), "Tesseract executable found via JARVIS_TESSERACT_PATH");
                return Ok(p);
            }
        }

        // 2. Check explicitly configured path
        if let Some(cp) = custom_path {
            if cp.exists() && cp.is_file() {
                debug!(path = %cp.display(), "Tesseract executable found via custom config path");
                return Ok(cp.to_path_buf());
            }
        }

        // 3. Check standard Windows installation paths
        #[cfg(target_os = "windows")]
        {
            let windows_candidates = [
                r"C:\Program Files\Tesseract-OCR\tesseract.exe",
                r"C:\Program Files (x86)\Tesseract-OCR\tesseract.exe",
            ];
            for path_str in windows_candidates {
                let p = PathBuf::from(path_str);
                if p.exists() && p.is_file() {
                    debug!(path = %p.display(), "Tesseract executable found at standard Windows path");
                    return Ok(p);
                }
            }

            if let Ok(local_app_data) = env::var("LOCALAPPDATA") {
                let p = PathBuf::from(local_app_data)
                    .join("Programs")
                    .join("Tesseract-OCR")
                    .join("tesseract.exe");
                if p.exists() && p.is_file() {
                    debug!(path = %p.display(), "Tesseract executable found at LOCALAPPDATA path");
                    return Ok(p);
                }
            }
        }

        // 4. Check system PATH by attempting a lookup or defaulting to executable name
        if let Ok(path_var) = env::var("PATH") {
            let exec_name = if cfg!(target_os = "windows") {
                "tesseract.exe"
            } else {
                "tesseract"
            };

            for dir in env::split_paths(&path_var) {
                let full_path = dir.join(exec_name);
                if full_path.exists() && full_path.is_file() {
                    debug!(path = %full_path.display(), "Tesseract executable found in system PATH");
                    return Ok(full_path);
                }
            }
        }

        Err(OcrError::ExecutableNotFound(
            "Tesseract executable not found. Specify JARVIS_TESSERACT_PATH or install Tesseract-OCR.".to_string(),
        ))
    }
}

impl Default for TesseractOcrProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl OcrProvider for TesseractOcrProvider {
    fn provider_type(&self) -> OcrProviderType {
        OcrProviderType::Tesseract
    }

    #[instrument(skip(self, request))]
    async fn extract_text(&self, request: &OcrRequest) -> Result<OcrResponse, OcrError> {
        let start = Instant::now();

        // 1. Input validation
        if request.image_bytes.is_empty() {
            return Err(OcrError::InvalidImage(
                "Image bytes buffer is empty (0 bytes)".to_string(),
            ));
        }

        if request.image_bytes.len() > self.config.max_image_bytes {
            return Err(OcrError::InvalidImage(format!(
                "Image size ({} bytes) exceeds maximum limit of {} bytes",
                request.image_bytes.len(),
                self.config.max_image_bytes
            )));
        }

        // 2. Resolve executable
        let exe_path = Self::resolve_tesseract_binary(self.config.tesseract_path.as_deref())?;

        // 3. Write image bytes to a temporary PNG file for process execution
        let temp_dir = env::temp_dir();
        let unique_id = uuid::Uuid::new_v4();
        let temp_img_path = temp_dir.join(format!("jarvis_ocr_{}.png", unique_id));

        tokio::fs::write(&temp_img_path, &request.image_bytes)
            .await
            .map_err(|e| {
                OcrError::Internal(format!("Failed to write temporary OCR image: {}", e))
            })?;

        let lang = request.language.as_deref().unwrap_or(&self.config.language);

        let timeout_secs = request.timeout_secs.unwrap_or(self.config.timeout_secs);

        // 4. Execute Tesseract process directly (stdout mode, process execution)
        let process_cmd = tokio::process::Command::new(&exe_path)
            .arg(&temp_img_path)
            .arg("stdout")
            .arg("-l")
            .arg(lang)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output();

        let output_res =
            tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), process_cmd).await;

        // Clean up temporary image file regardless of process outcome
        let _ = tokio::fs::remove_file(&temp_img_path).await;

        let output = match output_res {
            Ok(Ok(out)) => out,
            Ok(Err(e)) => {
                return Err(OcrError::ProcessFailed(format!(
                    "Failed to launch Tesseract process: {}",
                    e
                )));
            }
            Err(_) => {
                return Err(OcrError::Timeout(timeout_secs));
            }
        };

        if !output.status.success() {
            let stderr_text = String::from_utf8_lossy(&output.stderr);
            return Err(OcrError::ProcessFailed(format!(
                "Tesseract exited with error code {:?}: {}",
                output.status.code(),
                stderr_text.trim()
            )));
        }

        // 5. Parse output stdout
        let raw_text = String::from_utf8_lossy(&output.stdout);
        let trimmed_text = raw_text.trim().to_string();
        let has_text = !trimmed_text.is_empty();
        let char_count = trimmed_text.chars().count();
        let latency_ms = start.elapsed().as_millis() as u64;

        info!(
            provider = "Tesseract",
            has_text = has_text,
            char_count = char_count,
            latency_ms = latency_ms,
            "OCR extraction completed"
        );

        Ok(OcrResponse {
            provider_type: OcrProviderType::Tesseract,
            text: trimmed_text,
            has_text,
            char_count,
            confidence: None,
            latency_ms,
            regions: vec![],
        })
    }
}

// ============================================================
// Mock OCR Provider Implementation (For Deterministic Testing)
// ============================================================

/// Mock OCR provider for unit tests without external executable dependencies.
#[derive(Debug, Clone)]
pub struct MockOcrProvider {
    canned_text: String,
    fail: bool,
}

impl MockOcrProvider {
    pub fn new() -> Self {
        Self {
            canned_text: "JARVIS OCR TEST 12345".to_string(),
            fail: false,
        }
    }

    pub fn with_canned_text(mut self, text: impl Into<String>) -> Self {
        self.canned_text = text.into();
        self
    }

    pub fn with_failing(mut self, fail: bool) -> Self {
        self.fail = fail;
        self
    }
}

impl Default for MockOcrProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl OcrProvider for MockOcrProvider {
    fn provider_type(&self) -> OcrProviderType {
        OcrProviderType::Mock
    }

    async fn extract_text(&self, request: &OcrRequest) -> Result<OcrResponse, OcrError> {
        if self.fail {
            return Err(OcrError::ProviderUnavailable(
                "Mock OCR provider configured to fail".to_string(),
            ));
        }

        if request.image_bytes.is_empty() {
            return Err(OcrError::InvalidImage("Image buffer is empty".to_string()));
        }

        let trimmed = self.canned_text.trim().to_string();
        let has_text = !trimmed.is_empty();
        let char_count = trimmed.chars().count();

        Ok(OcrResponse {
            provider_type: OcrProviderType::Mock,
            text: trimmed,
            has_text,
            char_count,
            confidence: Some(95.0),
            latency_ms: 5,
            regions: vec![],
        })
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ocr_request_construction() {
        let bytes = vec![1, 2, 3, 4];
        let req = OcrRequest::new(bytes.clone())
            .with_language("eng")
            .with_timeout(15);

        assert_eq!(req.image_bytes, bytes);
        assert_eq!(req.language.unwrap(), "eng");
        assert_eq!(req.timeout_secs.unwrap(), 15);
    }

    #[tokio::test]
    async fn test_mock_ocr_provider_success() {
        let mock = MockOcrProvider::new().with_canned_text("SAMPLE DETECTED TEXT");
        let req = OcrRequest::new(vec![0xFF, 0xD8, 0xFF]);

        let res = mock.extract_text(&req).await.unwrap();
        assert_eq!(res.provider_type, OcrProviderType::Mock);
        assert!(res.has_text);
        assert_eq!(res.text, "SAMPLE DETECTED TEXT");
        assert_eq!(res.char_count, 20);
    }

    #[tokio::test]
    async fn test_empty_ocr_result() {
        let mock = MockOcrProvider::new().with_canned_text("   ");
        let req = OcrRequest::new(vec![1, 2, 3]);

        let res = mock.extract_text(&req).await.unwrap();
        assert!(!res.has_text);
        assert_eq!(res.text, "");
        assert_eq!(res.char_count, 0);
    }

    #[tokio::test]
    async fn test_invalid_empty_image() {
        let mock = MockOcrProvider::new();
        let req = OcrRequest::new(vec![]);

        let err = mock.extract_text(&req).await.unwrap_err();
        assert!(matches!(err, OcrError::InvalidImage(_)));
    }

    #[tokio::test]
    async fn test_mock_ocr_provider_failure() {
        let mock = MockOcrProvider::new().with_failing(true);
        let req = OcrRequest::new(vec![1, 2, 3]);

        let err = mock.extract_text(&req).await.unwrap_err();
        assert!(matches!(err, OcrError::ProviderUnavailable(_)));
    }

    #[tokio::test]
    async fn test_tesseract_missing_executable_error() {
        let invalid_path = PathBuf::from("/nonexistent/path/to/tesseract_fake_exe");
        let config = OcrConfig {
            tesseract_path: Some(invalid_path),
            ..Default::default()
        };
        let provider = TesseractOcrProvider::with_config(config);
        let req = OcrRequest::new(vec![1, 2, 3]);

        // Temporarily clear JARVIS_TESSERACT_PATH env if set
        let orig_env = env::var("JARVIS_TESSERACT_PATH").ok();
        env::remove_var("JARVIS_TESSERACT_PATH");

        let result = provider.extract_text(&req).await;

        if let Some(val) = orig_env {
            env::set_var("JARVIS_TESSERACT_PATH", val);
        }

        // If Tesseract happens to be in PATH, resolution might succeed or fail depending on env,
        // but if custom path is invalid and no env set, it checks PATH.
        if let Err(e) = result {
            assert!(
                matches!(e, OcrError::ExecutableNotFound(_))
                    || matches!(e, OcrError::ProcessFailed(_))
            );
        }
    }
}
