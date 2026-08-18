//! JARVIS Local AI Subsystem
//!
//! Exposes the provider-independent ModelGateway, ModelRouter, ModelProvider trait,
//! and local inference adapters (Ollama, llama.cpp, Mock).
//!
//! # Architecture
//!
//! ```text
//! JARVIS Core / Orchestrator
//!            ↓
//!       ModelGateway
//!            ↓
//!       ModelRouter (Category routing & automatic fallback)
//!     ┌───────┴────────┐
//!     ▼                ▼
//! OllamaProvider   LlamaCppProvider
//!     ▼                ▼
//! Local Models     GGUF Models
//! ```
//!
//! IMPLEMENTATION STATUS: Phase 4, Milestones M04.01 → M04.13

pub mod gateway;
pub mod ocr;
pub mod provider;
pub mod providers;
pub mod router;
pub mod screen_elements;
pub mod types;
pub mod vision;

pub use gateway::ModelGateway;
pub use ocr::{
    MockOcrProvider, OcrConfig, OcrError, OcrProvider, OcrProviderType, OcrRequest, OcrResponse,
    OcrTextRegion, TesseractOcrProvider,
};
pub use provider::ModelProvider;
pub use providers::{LlamaCppProvider, MockModelProvider, OllamaProvider};
pub use router::{ModelRouter, ModelRoutingConfig};
pub use screen_elements::{
    build_detection_prompt, parse_elements_from_vision_response, DetectionSource,
    ElementDetectionRequest, ElementDetectionResult, ElementType, ScreenElement,
};
pub use types::*;
pub use vision::{
    MockVisionProvider, OllamaVisionProvider, VisionConfig, VisionImage, VisionImageFormat,
    VisionModelProvider, VisionRequest, VisionResponse,
};

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use futures::StreamExt;
    use std::sync::Arc;

    use super::*;

    #[tokio::test]
    async fn test_mock_provider_health_and_generation() {
        let provider = MockModelProvider::new().with_canned_text("Hello from JARVIS AI");

        let health = provider.check_health().await.unwrap();
        assert!(health.is_online);
        assert_eq!(health.provider_type, ModelProviderType::Mock);

        let models = provider.list_models().await.unwrap();
        assert_eq!(models.len(), 1);
        assert!(models[0].capabilities.text_generation);

        let req = ModelRequest::new().with_message(ChatMessage::user("Greetings"));
        let resp = provider.generate(&req).await.unwrap();
        assert_eq!(resp.text, "Hello from JARVIS AI");
        assert_eq!(resp.provider_type, ModelProviderType::Mock);
    }

    #[tokio::test]
    async fn test_mock_provider_streaming() {
        let provider = MockModelProvider::new().with_canned_text("This is a streaming test");

        let req = ModelRequest::new().with_message(ChatMessage::user("Stream please"));
        let mut stream = provider.stream(&req).await.unwrap();

        let mut collected = String::new();
        while let Some(chunk_res) = stream.next().await {
            let chunk = chunk_res.unwrap();
            collected.push_str(&chunk.delta_text);
        }

        assert!(collected.contains("This is a streaming test"));
    }

    #[tokio::test]
    async fn test_router_category_and_fallback() {
        let primary_mock = Arc::new(MockModelProvider::new().with_canned_text("Primary response"));
        let backup_mock = Arc::new(MockModelProvider::new().with_canned_text("Backup response"));

        let config = ModelRoutingConfig::default();
        let router = ModelRouter::new(config)
            .with_provider(primary_mock.clone())
            .with_provider(backup_mock.clone());

        // 1. Normal execution with primary
        let req = ModelRequest::new().with_category(ModelCategory::General);
        let resp = router.generate(&req).await.unwrap();
        assert_eq!(resp.text, "Primary response");
        assert!(!resp.was_fallback);

        // 2. Primary fails, router automatically falls back to secondary
        primary_mock.set_failing(true);

        let resp_fallback = router.generate(&req).await.unwrap();
        assert_eq!(resp_fallback.text, "Backup response");
        assert!(resp_fallback.was_fallback);
        assert!(resp_fallback.fallback_reason.is_some());
    }

    #[tokio::test]
    async fn test_gateway_action_planning() {
        let tool_calls = vec![ModelToolCall {
            id: "call_123".to_string(),
            tool_name: "open_application".to_string(),
            arguments_json: r#"{"application":"chrome"}"#.to_string(),
        }];

        let provider = Arc::new(MockModelProvider::new().with_canned_tool_calls(tool_calls));
        let router =
            Arc::new(ModelRouter::new(ModelRoutingConfig::default()).with_provider(provider));
        let gateway = ModelGateway::new(router);

        let tools = vec![ToolSchema {
            name: "open_application".to_string(),
            description: "Opens an application".to_string(),
            parameters: serde_json::json!({"type": "object"}),
        }];

        let plan = gateway.plan_action("open chrome", tools).await.unwrap();
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].tool_name, "open_application");
        assert_eq!(plan[0].arguments_json, r#"{"application":"chrome"}"#);
    }

    #[tokio::test]
    async fn test_ollama_provider_health_offline_handling() {
        // Points to non-existent port to test graceful offline health reporting
        let provider = OllamaProvider::new("http://127.0.0.1:9999");
        let health = provider.check_health().await.unwrap();
        assert!(!health.is_online);
        assert_eq!(health.provider_type, ModelProviderType::Ollama);
    }

    #[tokio::test]
    async fn test_gateway_vision_integration() {
        let mock_model_provider = Arc::new(MockModelProvider::new());
        let router = Arc::new(ModelRouter::new(ModelRoutingConfig::default()).with_provider(mock_model_provider));
        let gateway = ModelGateway::new(router);

        let vision_provider = MockVisionProvider::new().with_canned_description("Screen contains Chrome browser window");
        let img = VisionImage::from_png_bytes(vec![1, 2, 3, 4]);
        let req = VisionRequest::new(img, "What is visible on the screen?");

        let resp = gateway.analyze_image(&vision_provider, &req).await.unwrap();
        assert_eq!(resp.description, "Screen contains Chrome browser window");
        assert_eq!(resp.provider_type, ModelProviderType::Mock);
    }
}
