//! Vision Model Provider Abstraction & Local Multimodal Inference (Moondream / LLaVA)
//!
//! Provides the core abstraction and local implementations for processing images/screenshots
//! and returning structured natural language visual understanding.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Instant;

use crate::types::{ModelError, ModelProviderType, ModelUsage};

/// Supported image formats for vision analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VisionImageFormat {
    Png,
    Jpeg,
    Webp,
}

impl VisionImageFormat {
    pub fn mime_type(&self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Webp => "image/webp",
        }
    }
}

/// Image input structure for vision model analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionImage {
    /// Raw image bytes
    pub data: Vec<u8>,
    /// Image format (PNG, JPEG, WebP)
    pub format: VisionImageFormat,
    /// Image width in pixels if known
    pub width: Option<u32>,
    /// Image height in pixels if known
    pub height: Option<u32>,
}

impl VisionImage {
    pub fn from_png_bytes(bytes: Vec<u8>) -> Self {
        Self {
            data: bytes,
            format: VisionImageFormat::Png,
            width: None,
            height: None,
        }
    }

    pub fn from_jpeg_bytes(bytes: Vec<u8>) -> Self {
        Self {
            data: bytes,
            format: VisionImageFormat::Jpeg,
            width: None,
            height: None,
        }
    }

    pub fn with_dimensions(mut self, width: u32, height: u32) -> Self {
        self.width = Some(width);
        self.height = Some(height);
        self
    }

    /// Convert raw bytes into a Base64-encoded string for local vision provider APIs.
    pub fn to_base64(&self) -> String {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(&self.data)
    }

    /// Validate image byte size and dimensions against resource limits.
    pub fn validate(
        &self,
        max_bytes: usize,
        max_dims: Option<(u32, u32)>,
    ) -> Result<(), ModelError> {
        if self.data.is_empty() {
            return Err(ModelError::InvalidRequest(
                "Image byte buffer is empty".to_string(),
            ));
        }

        if self.data.len() > max_bytes {
            return Err(ModelError::InvalidRequest(format!(
                "Image byte size ({} bytes) exceeds maximum allowed limit of {} bytes",
                self.data.len(),
                max_bytes
            )));
        }

        if let (Some(w), Some(h)) = (self.width, self.height) {
            if let Some((max_w, max_h)) = max_dims {
                if w > max_w || h > max_h {
                    return Err(ModelError::InvalidRequest(format!(
                        "Image dimensions ({}x{}) exceed maximum allowed limit of {}x{}",
                        w, h, max_w, max_h
                    )));
                }
            }
        }

        Ok(())
    }
}

/// A structured request sent to a vision model provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionRequest {
    /// Input image / screenshot data
    pub image: VisionImage,
    /// Prompt / instruction describing what to analyze
    pub prompt: String,
    /// Optional model identifier override (e.g. "moondream", "llava")
    pub model_id: Option<String>,
    /// Sampling temperature
    pub temperature: Option<f32>,
    /// Maximum completion tokens
    pub max_tokens: Option<u32>,
    /// Request timeout in seconds
    pub timeout_secs: Option<u64>,
}

impl VisionRequest {
    pub fn new(image: VisionImage, prompt: impl Into<String>) -> Self {
        Self {
            image,
            prompt: prompt.into(),
            model_id: None,
            temperature: None,
            max_tokens: None,
            timeout_secs: Some(60),
        }
    }

    pub fn with_model(mut self, model_id: impl Into<String>) -> Self {
        self.model_id = Some(model_id.into());
        self
    }

    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = Some(secs);
        self
    }
}

/// Structured response returned by a vision model provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionResponse {
    /// Natural-language description or analysis of the image
    pub description: String,
    /// Model identifier that produced the response
    pub model_id: String,
    /// Provider runtime type
    pub provider_type: ModelProviderType,
    /// Inference latency in milliseconds
    pub latency_ms: u64,
    /// Token usage statistics if available
    pub usage: Option<ModelUsage>,
}

/// Configuration parameters for vision model inference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionConfig {
    pub provider_type: ModelProviderType,
    pub model_name: String,
    pub endpoint: String,
    pub timeout_secs: u64,
    pub max_image_bytes: usize,
    pub max_dimensions: Option<(u32, u32)>,
}

impl Default for VisionConfig {
    fn default() -> Self {
        Self {
            provider_type: ModelProviderType::Ollama,
            model_name: "moondream:latest".to_string(),
            endpoint: "http://127.0.0.1:11434".to_string(),
            timeout_secs: 60,
            max_image_bytes: 10 * 1024 * 1024, // 10 MB default limit
            max_dimensions: Some((4096, 4096)),
        }
    }
}

/// Common abstract interface for local Vision Model Providers.
#[async_trait]
pub trait VisionModelProvider: Send + Sync {
    /// Return the runtime provider type.
    fn provider_type(&self) -> ModelProviderType;

    /// Return the active vision model identifier.
    fn model_name(&self) -> &str;

    /// Perform multimodal visual image analysis.
    async fn analyze_image(&self, request: &VisionRequest) -> Result<VisionResponse, ModelError>;
}

/// Local Ollama Vision Provider (supports Moondream & LLaVA).
pub struct OllamaVisionProvider {
    config: VisionConfig,
    client: reqwest::Client,
}

impl OllamaVisionProvider {
    pub fn new(config: VisionConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .unwrap_or_default();

        Self { config, client }
    }

    pub fn with_model(mut self, model_name: impl Into<String>) -> Self {
        self.config.model_name = model_name.into();
        self
    }
}

impl Default for OllamaVisionProvider {
    fn default() -> Self {
        Self::new(VisionConfig::default())
    }
}

#[async_trait]
impl VisionModelProvider for OllamaVisionProvider {
    fn provider_type(&self) -> ModelProviderType {
        self.config.provider_type.clone()
    }

    fn model_name(&self) -> &str {
        &self.config.model_name
    }

    async fn analyze_image(&self, request: &VisionRequest) -> Result<VisionResponse, ModelError> {
        let start = Instant::now();

        // 1. Validate request image limits
        request
            .image
            .validate(self.config.max_image_bytes, self.config.max_dimensions)?;

        let model_id = request
            .model_id
            .as_deref()
            .unwrap_or(&self.config.model_name);

        let b64_img = request.image.to_base64();

        // 2. Build Ollama API chat request with base64 image
        let payload = serde_json::json!({
            "model": model_id,
            "stream": false,
            "messages": [
                {
                    "role": "user",
                    "content": request.prompt,
                    "images": [b64_img]
                }
            ],
            "options": {
                "temperature": request.temperature.unwrap_or(0.2)
            }
        });

        let url = format!("{}/api/chat", self.config.endpoint);

        let response = self
            .client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| {
                ModelError::ConnectionFailure(
                    url.clone(),
                    format!("Failed to connect to local vision provider: {}", e),
                )
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let err_text = response.text().await.unwrap_or_default();
            return Err(ModelError::ProviderUnavailable(
                ModelProviderType::Ollama,
                format!(
                    "Ollama vision inference request failed ({}) for model '{}': {}",
                    status, model_id, err_text
                ),
            ));
        }

        let resp_json: serde_json::Value = response.json().await.map_err(|e| {
            ModelError::InvalidResponse(format!(
                "Failed to parse JSON response from vision provider: {}",
                e
            ))
        })?;

        let description = resp_json
            .get("message")
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .trim()
            .to_string();

        if description.is_empty() {
            return Err(ModelError::InvalidResponse(
                "Vision model returned an empty description".to_string(),
            ));
        }

        Ok(VisionResponse {
            description,
            model_id: model_id.to_string(),
            provider_type: ModelProviderType::Ollama,
            latency_ms: start.elapsed().as_millis() as u64,
            usage: None,
        })
    }
}

/// Deterministic Mock Vision Model Provider for unit testing.
pub struct MockVisionProvider {
    canned_description: String,
    failing: bool,
    config: VisionConfig,
}

impl MockVisionProvider {
    pub fn new() -> Self {
        Self {
            canned_description: "The screen shows a Google Chrome browser window open to GitHub."
                .to_string(),
            failing: false,
            config: VisionConfig::default(),
        }
    }

    pub fn with_canned_description(mut self, desc: impl Into<String>) -> Self {
        self.canned_description = desc.into();
        self
    }

    pub fn with_failing(mut self, failing: bool) -> Self {
        self.failing = failing;
        self
    }

    pub fn with_config(mut self, config: VisionConfig) -> Self {
        self.config = config;
        self
    }
}

impl Default for MockVisionProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl VisionModelProvider for MockVisionProvider {
    fn provider_type(&self) -> ModelProviderType {
        ModelProviderType::Mock
    }

    fn model_name(&self) -> &str {
        "mock-vision-model"
    }

    async fn analyze_image(&self, request: &VisionRequest) -> Result<VisionResponse, ModelError> {
        if self.failing {
            return Err(ModelError::ProviderUnavailable(
                ModelProviderType::Mock,
                "Mock vision provider is configured to fail".to_string(),
            ));
        }

        // Validate request image limits
        request
            .image
            .validate(self.config.max_image_bytes, self.config.max_dimensions)?;

        Ok(VisionResponse {
            description: self.canned_description.clone(),
            model_id: "mock-vision-model".to_string(),
            provider_type: ModelProviderType::Mock,
            latency_ms: 5,
            usage: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vision_request_construction() {
        let img = VisionImage::from_png_bytes(vec![1, 2, 3, 4]).with_dimensions(1920, 1080);
        let req = VisionRequest::new(img.clone(), "What is on the screen?")
            .with_model("moondream")
            .with_timeout(30);

        assert_eq!(req.prompt, "What is on the screen?");
        assert_eq!(req.model_id, Some("moondream".to_string()));
        assert_eq!(req.timeout_secs, Some(30));
        assert_eq!(req.image.width, Some(1920));
        assert_eq!(req.image.height, Some(1080));
        assert!(req.image.validate(1000, Some((2000, 2000))).is_ok());
    }

    #[test]
    fn test_invalid_empty_image_rejected() {
        let img = VisionImage::from_png_bytes(vec![]);
        let req = VisionRequest::new(img, "Describe");
        let res = req.image.validate(1000, None);

        assert!(res.is_err());
        match res.unwrap_err() {
            ModelError::InvalidRequest(msg) => assert!(msg.contains("empty")),
            e => panic!("Unexpected error type: {:?}", e),
        }
    }

    #[test]
    fn test_image_size_limit_exceeded() {
        let img = VisionImage::from_png_bytes(vec![0u8; 5000]);
        let req = VisionRequest::new(img, "Describe");
        let res = req.image.validate(1000, None);

        assert!(res.is_err());
        match res.unwrap_err() {
            ModelError::InvalidRequest(msg) => assert!(msg.contains("exceeds maximum")),
            e => panic!("Unexpected error type: {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_provider_unavailable_returns_structured_error() {
        let config = VisionConfig {
            endpoint: "http://127.0.0.1:59999".to_string(), // Dead port
            timeout_secs: 1,
            ..Default::default()
        };
        let provider = OllamaVisionProvider::new(config);
        let img = VisionImage::from_png_bytes(vec![1, 2, 3, 4]);
        let req = VisionRequest::new(img, "Test");

        let res = provider.analyze_image(&req).await;
        assert!(res.is_err());
        match res.unwrap_err() {
            ModelError::ConnectionFailure(endpoint, _) => assert!(endpoint.contains("59999")),
            e => panic!("Unexpected error type: {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_mock_vision_provider() {
        let provider =
            MockVisionProvider::new().with_canned_description("Verified mock screen description");
        let img = VisionImage::from_png_bytes(vec![10, 20, 30]);
        let req = VisionRequest::new(img, "Analyze UI");

        let resp = provider.analyze_image(&req).await.unwrap();
        assert_eq!(resp.description, "Verified mock screen description");
        assert_eq!(resp.provider_type, ModelProviderType::Mock);
        assert_eq!(resp.model_id, "mock-vision-model");
    }
}
