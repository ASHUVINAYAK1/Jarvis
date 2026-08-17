//! ModelProvider trait defining the common interface for local model engines.

use async_trait::async_trait;

use crate::types::{
    ModelError, ModelInfo, ModelProviderType, ModelRequest, ModelResponse, ModelStream,
    ProviderHealth,
};

/// Common abstract interface implemented by all AI model providers (Ollama, llama.cpp, Mock).
#[async_trait]
pub trait ModelProvider: Send + Sync {
    /// Return the runtime provider type.
    fn provider_type(&self) -> ModelProviderType;

    /// Return the base endpoint or identifier for this provider.
    fn endpoint(&self) -> &str;

    /// Check if the provider daemon/engine is running and responsive.
    async fn check_health(&self) -> Result<ProviderHealth, ModelError>;

    /// Discover and list all models currently installed or available in this provider.
    async fn list_models(&self) -> Result<Vec<ModelInfo>, ModelError>;

    /// Perform synchronous non-streaming chat generation.
    async fn generate(&self, request: &ModelRequest) -> Result<ModelResponse, ModelError>;

    /// Perform token-by-token streaming chat generation.
    async fn stream(&self, request: &ModelRequest) -> Result<ModelStream, ModelError>;
}
