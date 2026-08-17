use std::pin::Pin;

use chrono::{DateTime, Utc};
use futures::Stream;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Type of local model provider runtime.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModelProviderType {
    Ollama,
    LlamaCpp,
    Mock,
    Custom(String),
}

impl std::fmt::Display for ModelProviderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ollama => write!(f, "Ollama"),
            Self::LlamaCpp => write!(f, "llama.cpp"),
            Self::Mock => write!(f, "Mock"),
            Self::Custom(name) => write!(f, "Custom({})", name),
        }
    }
}

/// Categorical purpose of a model for intelligent routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModelCategory {
    /// Low-latency, small model for quick queries & simple classification
    Fast,
    /// Balanced general-purpose conversation & instruction following
    General,
    /// Complex multi-step reasoning, coding, and problem solving
    Reasoning,
    /// High-precision JSON schema tool calling and execution planning
    ToolCalling,
    /// Multimodal vision understanding (screenshots, images, UI)
    Vision,
    /// Text embeddings for local vector memory & RAG
    Embedding,
}

impl std::fmt::Display for ModelCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Feature capabilities supported by a model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelCapabilities {
    pub text_generation: bool,
    pub streaming: bool,
    pub tool_calling: bool,
    pub structured_output: bool,
    pub json_mode: bool,
    pub vision: bool,
    pub embeddings: bool,
    pub context_window: u32,
}

impl Default for ModelCapabilities {
    fn default() -> Self {
        Self {
            text_generation: true,
            streaming: true,
            tool_calling: false,
            structured_output: false,
            json_mode: false,
            vision: false,
            embeddings: false,
            context_window: 4096,
        }
    }
}

/// Complete metadata for a locally available or configured model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub provider_type: ModelProviderType,
    pub family: String,
    pub parameter_count: Option<String>,
    pub quantization: Option<String>,
    pub context_window: u32,
    pub capabilities: ModelCapabilities,
    pub is_available: bool,
    pub modified_at: Option<DateTime<Utc>>,
    pub size_bytes: Option<u64>,
}

/// Role of a message in a conversation context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    System,
    User,
    Assistant,
    Tool,
}

/// A structured chat message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ModelToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::System,
            content: content.into(),
            images: None,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::User,
            content: content.into(),
            images: None,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::Assistant,
            content: content.into(),
            images: None,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn tool(content: impl Into<String>, tool_call_id: impl Into<String>) -> Self {
        Self {
            role: ChatRole::Tool,
            content: content.into(),
            images: None,
            tool_calls: None,
            tool_call_id: Some(tool_call_id.into()),
        }
    }
}

/// Tool definition presented to the model for structured tool-calling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSchema {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// Desired response format from the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResponseFormat {
    Text,
    JsonObject,
    JsonSchema(serde_json::Value),
}

/// A structured request sent to a model provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRequest {
    pub model_id: Option<String>,
    pub category: Option<ModelCategory>,
    pub messages: Vec<ChatMessage>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub max_tokens: Option<u32>,
    pub tools: Option<Vec<ToolSchema>>,
    pub response_format: Option<ResponseFormat>,
    pub timeout_secs: Option<u64>,
    pub correlation_id: Option<String>,
}

impl ModelRequest {
    pub fn new() -> Self {
        Self {
            model_id: None,
            category: None,
            messages: Vec::new(),
            temperature: None,
            top_p: None,
            max_tokens: None,
            tools: None,
            response_format: None,
            timeout_secs: Some(30),
            correlation_id: None,
        }
    }

    pub fn with_message(mut self, message: ChatMessage) -> Self {
        self.messages.push(message);
        self
    }

    pub fn with_category(mut self, category: ModelCategory) -> Self {
        self.category = Some(category);
        self
    }

    pub fn with_model(mut self, model_id: impl Into<String>) -> Self {
        self.model_id = Some(model_id.into());
        self
    }

    pub fn with_tools(mut self, tools: Vec<ToolSchema>) -> Self {
        self.tools = Some(tools);
        self
    }

    pub fn with_json_output(mut self) -> Self {
        self.response_format = Some(ResponseFormat::JsonObject);
        self
    }
}

impl Default for ModelRequest {
    fn default() -> Self {
        Self::new()
    }
}

/// A tool call proposed by the model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelToolCall {
    pub id: String,
    pub tool_name: String,
    pub arguments_json: String,
}

/// Token usage statistics for a request.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct ModelUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// The complete response from a model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelResponse {
    pub text: String,
    pub tool_calls: Vec<ModelToolCall>,
    pub usage: ModelUsage,
    pub model_id: String,
    pub provider_type: ModelProviderType,
    pub finish_reason: Option<String>,
    pub latency_ms: u64,
    pub was_fallback: bool,
    pub fallback_reason: Option<String>,
}

/// A streaming token chunk returned during generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelChunk {
    pub delta_text: String,
    pub delta_tool_calls: Vec<ModelToolCall>,
    pub is_done: bool,
    pub usage: Option<ModelUsage>,
}

pub type ModelStream = Pin<Box<dyn Stream<Item = Result<ModelChunk, ModelError>> + Send>>;

/// Health status of a model provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderHealth {
    pub provider_type: ModelProviderType,
    pub is_online: bool,
    pub endpoint: String,
    pub version: Option<String>,
    pub available_models_count: usize,
    pub message: Option<String>,
}

/// Structured error taxonomy for the AI subsystem.
#[derive(Debug, Error)]
pub enum ModelError {
    #[error("Provider '{0}' is unavailable: {1}")]
    ProviderUnavailable(ModelProviderType, String),

    #[error("Model '{0}' was not found or is unavailable")]
    ModelUnavailable(String),

    #[error("Failed to load model '{0}': {1}")]
    ModelLoadFailure(String, String),

    #[error("Connection failure communicating with '{0}': {1}")]
    ConnectionFailure(String, String),

    #[error("Request timed out after {0} seconds")]
    Timeout(u64),

    #[error("Invalid model request: {0}")]
    InvalidRequest(String),

    #[error("Invalid response from provider: {0}")]
    InvalidResponse(String),

    #[error("Capability '{0}' is not supported by model '{1}'")]
    UnsupportedCapability(String, String),

    #[error("Out of memory on local device: {0}")]
    OutOfMemory(String),

    #[error("Operation was cancelled")]
    Cancelled,

    #[error("Runtime failure: {0}")]
    RuntimeFailure(String),
}
