use std::time::Instant;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio_stream::wrappers::ReceiverStream;
use tracing::instrument;

use crate::provider::ModelProvider;
use crate::types::{
    ChatRole, ModelCapabilities, ModelChunk, ModelError, ModelInfo,
    ModelProviderType, ModelRequest, ModelResponse, ModelStream, ModelToolCall, ModelUsage,
    ProviderHealth, ResponseFormat,
};

/// Ollama provider implementation.
pub struct OllamaProvider {
    endpoint: String,
    client: Client,
}

impl OllamaProvider {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .unwrap_or_default(),
        }
    }

    pub fn default_local() -> Self {
        Self::new("http://127.0.0.1:11434")
    }
}

impl Default for OllamaProvider {
    fn default() -> Self {
        Self::default_local()
    }
}

// Ollama API DTOs
#[derive(Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Value>,
}

#[derive(Serialize, Deserialize)]
struct OllamaMessage {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    images: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OllamaToolCall>>,
}

#[derive(Serialize, Deserialize)]
struct OllamaToolCall {
    function: OllamaFunctionCall,
}

#[derive(Serialize, Deserialize)]
struct OllamaFunctionCall {
    name: String,
    arguments: Value,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct OllamaChatResponse {
    message: OllamaResponseMessage,
    done: bool,
    #[serde(default)]
    total_duration: Option<u64>,
    #[serde(default)]
    prompt_eval_count: Option<u32>,
    #[serde(default)]
    eval_count: Option<u32>,
}

#[derive(Deserialize)]
struct OllamaResponseMessage {
    content: String,
    #[serde(default)]
    tool_calls: Option<Vec<OllamaToolCall>>,
}

#[derive(Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModelItem>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct OllamaModelItem {
    name: String,
    model: Option<String>,
    size: Option<u64>,
    modified_at: Option<String>,
    details: Option<OllamaModelDetails>,
}

#[derive(Deserialize)]
struct OllamaModelDetails {
    family: Option<String>,
    parameter_size: Option<String>,
    quantization_level: Option<String>,
}

#[derive(Deserialize)]
struct OllamaVersionResponse {
    version: String,
}

#[async_trait]
impl ModelProvider for OllamaProvider {
    fn provider_type(&self) -> ModelProviderType {
        ModelProviderType::Ollama
    }

    fn endpoint(&self) -> &str {
        &self.endpoint
    }

    #[instrument(skip(self))]
    async fn check_health(&self) -> Result<ProviderHealth, ModelError> {
        let url = format!("{}/api/version", self.endpoint.trim_end_matches('/'));
        let res = self.client.get(&url).send().await;

        match res {
            Ok(resp) if resp.status().is_success() => {
                let ver: OllamaVersionResponse = resp.json().await.unwrap_or(OllamaVersionResponse {
                    version: "unknown".to_string(),
                });

                let models = self.list_models().await.unwrap_or_default();

                Ok(ProviderHealth {
                    provider_type: ModelProviderType::Ollama,
                    is_online: true,
                    endpoint: self.endpoint.clone(),
                    version: Some(ver.version),
                    available_models_count: models.len(),
                    message: Some("Ollama local service is running".to_string()),
                })
            }
            Ok(resp) => Ok(ProviderHealth {
                provider_type: ModelProviderType::Ollama,
                is_online: false,
                endpoint: self.endpoint.clone(),
                version: None,
                available_models_count: 0,
                message: Some(format!("Ollama returned HTTP status {}", resp.status())),
            }),
            Err(e) => Ok(ProviderHealth {
                provider_type: ModelProviderType::Ollama,
                is_online: false,
                endpoint: self.endpoint.clone(),
                version: None,
                available_models_count: 0,
                message: Some(format!("Ollama daemon not reachable: {}", e)),
            }),
        }
    }

    #[instrument(skip(self))]
    async fn list_models(&self) -> Result<Vec<ModelInfo>, ModelError> {
        let url = format!("{}/api/tags", self.endpoint.trim_end_matches('/'));
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| ModelError::ConnectionFailure(self.endpoint.clone(), e.to_string()))?;

        if !resp.status().is_success() {
            return Err(ModelError::ProviderUnavailable(
                ModelProviderType::Ollama,
                format!("Failed to query tags: HTTP {}", resp.status()),
            ));
        }

        let tags: OllamaTagsResponse = resp
            .json()
            .await
            .map_err(|e| ModelError::InvalidResponse(e.to_string()))?;

        let mut models = Vec::new();
        for item in tags.models {
            let id = item.name.clone();
            let family = item
                .details
                .as_ref()
                .and_then(|d| d.family.clone())
                .unwrap_or_else(|| "unknown".to_string());
            let param_count = item.details.as_ref().and_then(|d| d.parameter_size.clone());
            let quant = item.details.as_ref().and_then(|d| d.quantization_level.clone());

            let modified = item
                .modified_at
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|d| d.with_timezone(&Utc));

            let capabilities = ModelCapabilities {
                text_generation: true,
                streaming: true,
                tool_calling: id.contains("qwen") || id.contains("llama3") || id.contains("mistral"),
                structured_output: true,
                json_mode: true,
                vision: id.contains("vision") || id.contains("llava") || id.contains("minicpm"),
                embeddings: id.contains("embed") || id.contains("bge") || id.contains("nomic"),
                context_window: 8192,
            };

            models.push(ModelInfo {
                id: id.clone(),
                name: id,
                provider_type: ModelProviderType::Ollama,
                family,
                parameter_count: param_count,
                quantization: quant,
                context_window: 8192,
                capabilities,
                is_available: true,
                modified_at: modified,
                size_bytes: item.size,
            });
        }

        Ok(models)
    }

    #[instrument(skip(self, request), fields(model = ?request.model_id))]
    async fn generate(&self, request: &ModelRequest) -> Result<ModelResponse, ModelError> {
        let start_time = Instant::now();
        let model_name = request
            .model_id
            .clone()
            .unwrap_or_else(|| "qwen2.5:latest".to_string());

        let url = format!("{}/api/chat", self.endpoint.trim_end_matches('/'));

        let mut messages = Vec::new();
        for msg in &request.messages {
            let role_str = match msg.role {
                ChatRole::System => "system",
                ChatRole::User => "user",
                ChatRole::Assistant => "assistant",
                ChatRole::Tool => "tool",
            };
            messages.push(OllamaMessage {
                role: role_str.to_string(),
                content: msg.content.clone(),
                images: msg.images.clone(),
                tool_calls: None,
            });
        }

        let format_val = match &request.response_format {
            Some(ResponseFormat::JsonObject) => Some(json!("json")),
            Some(ResponseFormat::JsonSchema(schema)) => Some(schema.clone()),
            _ => None,
        };

        let tools_val = request.tools.as_ref().map(|tools| {
            let list: Vec<Value> = tools
                .iter()
                .map(|t| {
                    json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.parameters
                        }
                    })
                })
                .collect();
            json!(list)
        });

        let mut options = serde_json::Map::new();
        if let Some(temp) = request.temperature {
            options.insert("temperature".to_string(), json!(temp));
        }
        if let Some(top_p) = request.top_p {
            options.insert("top_p".to_string(), json!(top_p));
        }
        if let Some(max_t) = request.max_tokens {
            options.insert("num_predict".to_string(), json!(max_t));
        }

        let req_body = OllamaChatRequest {
            model: model_name.clone(),
            messages,
            stream: false,
            format: format_val,
            options: if options.is_empty() {
                None
            } else {
                Some(Value::Object(options))
            },
            tools: tools_val,
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
            if err_text.contains("not found") {
                return Err(ModelError::ModelUnavailable(model_name));
            }
            return Err(ModelError::RuntimeFailure(format!(
                "Ollama chat error {}: {}",
                status, err_text
            )));
        }

        let body: OllamaChatResponse = resp
            .json()
            .await
            .map_err(|e| ModelError::InvalidResponse(e.to_string()))?;

        let mut tool_calls = Vec::new();
        if let Some(tcs) = body.message.tool_calls {
            for (idx, tc) in tcs.into_iter().enumerate() {
                tool_calls.push(ModelToolCall {
                    id: format!("call_{}_{}", idx, Utc::now().timestamp_millis()),
                    tool_name: tc.function.name,
                    arguments_json: tc.function.arguments.to_string(),
                });
            }
        }

        let usage = ModelUsage {
            prompt_tokens: body.prompt_eval_count.unwrap_or(0),
            completion_tokens: body.eval_count.unwrap_or(0),
            total_tokens: body.prompt_eval_count.unwrap_or(0) + body.eval_count.unwrap_or(0),
        };

        let latency_ms = start_time.elapsed().as_millis() as u64;

        Ok(ModelResponse {
            text: body.message.content,
            tool_calls,
            usage,
            model_id: model_name,
            provider_type: ModelProviderType::Ollama,
            finish_reason: if body.done {
                Some("stop".to_string())
            } else {
                None
            },
            latency_ms,
            was_fallback: false,
            fallback_reason: None,
        })
    }

    #[instrument(skip(self, request), fields(model = ?request.model_id))]
    async fn stream(&self, request: &ModelRequest) -> Result<ModelStream, ModelError> {
        let model_name = request
            .model_id
            .clone()
            .unwrap_or_else(|| "qwen2.5:latest".to_string());

        let url = format!("{}/api/chat", self.endpoint.trim_end_matches('/'));

        let mut messages = Vec::new();
        for msg in &request.messages {
            let role_str = match msg.role {
                ChatRole::System => "system",
                ChatRole::User => "user",
                ChatRole::Assistant => "assistant",
                ChatRole::Tool => "tool",
            };
            messages.push(OllamaMessage {
                role: role_str.to_string(),
                content: msg.content.clone(),
                images: msg.images.clone(),
                tool_calls: None,
            });
        }

        let req_body = OllamaChatRequest {
            model: model_name.clone(),
            messages,
            stream: true,
            format: None,
            options: None,
            tools: None,
        };

        let resp = self
            .client
            .post(&url)
            .json(&req_body)
            .send()
            .await
            .map_err(|e| ModelError::ConnectionFailure(self.endpoint.clone(), e.to_string()))?;

        if !resp.status().is_success() {
            return Err(ModelError::RuntimeFailure(format!(
                "Ollama stream error: HTTP {}",
                resp.status()
            )));
        }

        let (tx, rx) = tokio::sync::mpsc::channel(64);
        let mut byte_stream = resp.bytes_stream();

        tokio::spawn(async move {
            let mut buffer = String::new();

            while let Some(item) = byte_stream.next().await {
                match item {
                    Ok(bytes) => {
                        let text = String::from_utf8_lossy(&bytes);
                        buffer.push_str(&text);

                        while let Some(pos) = buffer.find('\n') {
                            let line = buffer[..pos].trim().to_string();
                            buffer = buffer[pos + 1..].to_string();

                            if line.is_empty() {
                                continue;
                            }

                            if let Ok(chunk_resp) = serde_json::from_str::<OllamaChatResponse>(&line) {
                                let chunk = ModelChunk {
                                    delta_text: chunk_resp.message.content,
                                    delta_tool_calls: Vec::new(),
                                    is_done: chunk_resp.done,
                                    usage: if chunk_resp.done {
                                        Some(ModelUsage {
                                            prompt_tokens: chunk_resp.prompt_eval_count.unwrap_or(0),
                                            completion_tokens: chunk_resp.eval_count.unwrap_or(0),
                                            total_tokens: chunk_resp.prompt_eval_count.unwrap_or(0)
                                                + chunk_resp.eval_count.unwrap_or(0),
                                        })
                                    } else {
                                        None
                                    },
                                };

                                if tx.send(Ok(chunk)).await.is_err() {
                                    return;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx
                            .send(Err(ModelError::ConnectionFailure(
                                "stream".to_string(),
                                e.to_string(),
                            )))
                            .await;
                        return;
                    }
                }
            }
        });

        Ok(Box::pin(ReceiverStream::new(rx)))
    }
}
