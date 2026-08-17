//! Deterministic Mock Model Provider for unit tests and offline CI.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::provider::ModelProvider;
use crate::types::{
    ModelCapabilities, ModelChunk, ModelError, ModelInfo, ModelProviderType, ModelRequest,
    ModelResponse, ModelStream, ModelToolCall, ModelUsage, ProviderHealth,
};

/// Mock model provider with configurable responses and failure triggers.
#[derive(Clone)]
pub struct MockModelProvider {
    canned_text: String,
    canned_tool_calls: Vec<ModelToolCall>,
    should_fail: Arc<AtomicBool>,
    fail_message: String,
    call_count: Arc<AtomicUsize>,
    latency: Duration,
    model_id: String,
}

impl MockModelProvider {
    pub fn new() -> Self {
        Self {
            canned_text: "I am JARVIS, sir. How may I assist you today?".to_string(),
            canned_tool_calls: Vec::new(),
            should_fail: Arc::new(AtomicBool::new(false)),
            fail_message: "Simulated mock provider failure".to_string(),
            call_count: Arc::new(AtomicUsize::new(0)),
            latency: Duration::from_millis(1),
            model_id: "mock-model-v1".to_string(),
        }
    }

    pub fn with_canned_text(mut self, text: impl Into<String>) -> Self {
        self.canned_text = text.into();
        self
    }

    pub fn with_canned_tool_calls(mut self, tool_calls: Vec<ModelToolCall>) -> Self {
        self.canned_tool_calls = tool_calls;
        self
    }

    pub fn set_failing(&self, fail: bool) {
        self.should_fail.store(fail, Ordering::SeqCst);
    }

    pub fn call_count(&self) -> usize {
        self.call_count.load(Ordering::SeqCst)
    }
}

impl Default for MockModelProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ModelProvider for MockModelProvider {
    fn provider_type(&self) -> ModelProviderType {
        ModelProviderType::Mock
    }

    fn endpoint(&self) -> &str {
        "mock://local"
    }

    async fn check_health(&self) -> Result<ProviderHealth, ModelError> {
        let is_failing = self.should_fail.load(Ordering::SeqCst);
        Ok(ProviderHealth {
            provider_type: ModelProviderType::Mock,
            is_online: !is_failing,
            endpoint: "mock://local".to_string(),
            version: Some("mock-1.0.0".to_string()),
            available_models_count: 1,
            message: if is_failing {
                Some("Mock provider forced offline".to_string())
            } else {
                Some("Mock provider online".to_string())
            },
        })
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, ModelError> {
        if self.should_fail.load(Ordering::SeqCst) {
            return Err(ModelError::ProviderUnavailable(
                ModelProviderType::Mock,
                self.fail_message.clone(),
            ));
        }

        Ok(vec![ModelInfo {
            id: self.model_id.clone(),
            name: self.model_id.clone(),
            provider_type: ModelProviderType::Mock,
            family: "mock".to_string(),
            parameter_count: Some("7B".to_string()),
            quantization: Some("Q4_K_M".to_string()),
            context_window: 8192,
            capabilities: ModelCapabilities {
                text_generation: true,
                streaming: true,
                tool_calling: true,
                structured_output: true,
                json_mode: true,
                vision: true,
                embeddings: true,
                context_window: 8192,
            },
            is_available: true,
            modified_at: None,
            size_bytes: Some(4_000_000_000),
        }])
    }

    async fn generate(&self, _request: &ModelRequest) -> Result<ModelResponse, ModelError> {
        self.call_count.fetch_add(1, Ordering::SeqCst);

        if self.should_fail.load(Ordering::SeqCst) {
            return Err(ModelError::ProviderUnavailable(
                ModelProviderType::Mock,
                self.fail_message.clone(),
            ));
        }

        if self.latency > Duration::ZERO {
            tokio::time::sleep(self.latency).await;
        }

        Ok(ModelResponse {
            text: self.canned_text.clone(),
            tool_calls: self.canned_tool_calls.clone(),
            usage: ModelUsage {
                prompt_tokens: 15,
                completion_tokens: 25,
                total_tokens: 40,
            },
            model_id: self.model_id.clone(),
            provider_type: ModelProviderType::Mock,
            finish_reason: Some("stop".to_string()),
            latency_ms: self.latency.as_millis() as u64,
            was_fallback: false,
            fallback_reason: None,
        })
    }

    async fn stream(&self, _request: &ModelRequest) -> Result<ModelStream, ModelError> {
        self.call_count.fetch_add(1, Ordering::SeqCst);

        if self.should_fail.load(Ordering::SeqCst) {
            return Err(ModelError::ProviderUnavailable(
                ModelProviderType::Mock,
                self.fail_message.clone(),
            ));
        }

        let (tx, rx) = mpsc::channel(16);
        let tokens: Vec<String> = self
            .canned_text
            .split_whitespace()
            .map(|s| format!("{} ", s))
            .collect();

        tokio::spawn(async move {
            for token in tokens {
                let chunk = ModelChunk {
                    delta_text: token,
                    delta_tool_calls: Vec::new(),
                    is_done: false,
                    usage: None,
                };
                if tx.send(Ok(chunk)).await.is_err() {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }

            let final_chunk = ModelChunk {
                delta_text: String::new(),
                delta_tool_calls: Vec::new(),
                is_done: true,
                usage: Some(ModelUsage {
                    prompt_tokens: 10,
                    completion_tokens: 20,
                    total_tokens: 30,
                }),
            };
            let _ = tx.send(Ok(final_chunk)).await;
        });

        Ok(Box::pin(ReceiverStream::new(rx)))
    }
}
