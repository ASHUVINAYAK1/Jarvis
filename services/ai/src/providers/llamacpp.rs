use std::time::Instant;

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::provider::ModelProvider;
use crate::types::{
    ChatRole, ModelCapabilities, ModelError, ModelInfo, ModelProviderType, ModelRequest,
    ModelResponse, ModelStream, ModelUsage, ProviderHealth,
};

/// llama.cpp provider implementation.
pub struct LlamaCppProvider {
    endpoint: String,
    client: Client,
    model_alias: String,
}

impl LlamaCppProvider {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .unwrap_or_default(),
            model_alias: "llama.cpp-default".to_string(),
        }
    }

    pub fn default_local() -> Self {
        Self::new("http://127.0.0.1:8080")
    }

    pub fn with_model_alias(mut self, alias: impl Into<String>) -> Self {
        self.model_alias = alias.into();
        self
    }
}

impl Default for LlamaCppProvider {
    fn default() -> Self {
        Self::default_local()
    }
}

#[derive(Deserialize)]
struct LlamaCppHealthResponse {
    status: Option<String>,
}

#[derive(Serialize)]
struct OpenAiChatRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
}

#[derive(Serialize)]
struct OpenAiMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct OpenAiChatResponse {
    choices: Vec<OpenAiChoice>,
    usage: Option<OpenAiUsage>,
}

#[derive(Deserialize)]
struct OpenAiChoice {
    message: OpenAiResponseMessage,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct OpenAiResponseMessage {
    content: Option<String>,
}

#[derive(Deserialize)]
struct OpenAiUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

#[async_trait]
impl ModelProvider for LlamaCppProvider {
    fn provider_type(&self) -> ModelProviderType {
        ModelProviderType::LlamaCpp
    }

    fn endpoint(&self) -> &str {
        &self.endpoint
    }

    #[instrument(skip(self))]
    async fn check_health(&self) -> Result<ProviderHealth, ModelError> {
        let url = format!("{}/health", self.endpoint.trim_end_matches('/'));
        let res = self.client.get(&url).send().await;

        match res {
            Ok(resp) if resp.status().is_success() => {
                let body: LlamaCppHealthResponse =
                    resp.json().await.unwrap_or(LlamaCppHealthResponse {
                        status: Some("ok".to_string()),
                    });

                Ok(ProviderHealth {
                    provider_type: ModelProviderType::LlamaCpp,
                    is_online: true,
                    endpoint: self.endpoint.clone(),
                    version: Some("llama.cpp server".to_string()),
                    available_models_count: 1,
                    message: Some(body.status.unwrap_or_else(|| "ready".to_string())),
                })
            }
            Ok(resp) => Ok(ProviderHealth {
                provider_type: ModelProviderType::LlamaCpp,
                is_online: false,
                endpoint: self.endpoint.clone(),
                version: None,
                available_models_count: 0,
                message: Some(format!("llama.cpp returned HTTP status {}", resp.status())),
            }),
            Err(e) => Ok(ProviderHealth {
                provider_type: ModelProviderType::LlamaCpp,
                is_online: false,
                endpoint: self.endpoint.clone(),
                version: None,
                available_models_count: 0,
                message: Some(format!("llama.cpp server not reachable: {}", e)),
            }),
        }
    }

    #[instrument(skip(self))]
    async fn list_models(&self) -> Result<Vec<ModelInfo>, ModelError> {
        // llama-server typically serves one actively loaded GGUF model
        Ok(vec![ModelInfo {
            id: self.model_alias.clone(),
            name: self.model_alias.clone(),
            provider_type: ModelProviderType::LlamaCpp,
            family: "gguf".to_string(),
            parameter_count: None,
            quantization: Some("GGUF".to_string()),
            context_window: 8192,
            capabilities: ModelCapabilities {
                text_generation: true,
                streaming: true,
                tool_calling: false,
                structured_output: true,
                json_mode: true,
                vision: false,
                embeddings: false,
                context_window: 8192,
            },
            is_available: true,
            modified_at: None,
            size_bytes: None,
        }])
    }

    #[instrument(skip(self, request))]
    async fn generate(&self, request: &ModelRequest) -> Result<ModelResponse, ModelError> {
        let start_time = Instant::now();
        let url = format!(
            "{}/v1/chat/completions",
            self.endpoint.trim_end_matches('/')
        );

        let mut messages = Vec::new();
        for msg in &request.messages {
            let role_str = match msg.role {
                ChatRole::System => "system",
                ChatRole::User => "user",
                ChatRole::Assistant => "assistant",
                ChatRole::Tool => "user",
            };
            messages.push(OpenAiMessage {
                role: role_str.to_string(),
                content: msg.content.clone(),
            });
        }

        let req_body = OpenAiChatRequest {
            model: request
                .model_id
                .clone()
                .unwrap_or_else(|| self.model_alias.clone()),
            messages,
            stream: false,
            temperature: request.temperature,
            max_tokens: request.max_tokens,
        };

        let resp = self
            .client
            .post(&url)
            .json(&req_body)
            .send()
            .await
            .map_err(|e| ModelError::ConnectionFailure(self.endpoint.clone(), e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err_text = resp.text().await.unwrap_or_default();
            return Err(ModelError::RuntimeFailure(format!(
                "llama.cpp error {}: {}",
                status, err_text
            )));
        }

        let body: OpenAiChatResponse = resp
            .json()
            .await
            .map_err(|e| ModelError::InvalidResponse(e.to_string()))?;

        let choice = body.choices.into_iter().next().ok_or_else(|| {
            ModelError::InvalidResponse("No choices returned from llama.cpp".to_string())
        })?;

        let text = choice.message.content.unwrap_or_default();
        let usage = body
            .usage
            .map(|u| ModelUsage {
                prompt_tokens: u.prompt_tokens,
                completion_tokens: u.completion_tokens,
                total_tokens: u.total_tokens,
            })
            .unwrap_or_default();

        let latency_ms = start_time.elapsed().as_millis() as u64;

        Ok(ModelResponse {
            text,
            tool_calls: Vec::new(),
            usage,
            model_id: self.model_alias.clone(),
            provider_type: ModelProviderType::LlamaCpp,
            finish_reason: choice.finish_reason,
            latency_ms,
            was_fallback: false,
            fallback_reason: None,
        })
    }

    async fn stream(&self, _request: &ModelRequest) -> Result<ModelStream, ModelError> {
        Err(ModelError::UnsupportedCapability(
            "streaming".to_string(),
            "llama.cpp provider stream wrapper".to_string(),
        ))
    }
}
