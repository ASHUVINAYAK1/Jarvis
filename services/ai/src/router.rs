//! Intelligent Model Router & Fallback Policy Engine
//!
//! Directs AI requests to the optimal local model provider based on requested
//! capabilities, category (`Fast`, `General`, `Reasoning`, `ToolCalling`, `Vision`),
//! and handles deterministic fallback when a provider is unavailable.

use std::collections::HashMap;
use std::sync::Arc;

use tracing::{info, warn};

use crate::provider::ModelProvider;
use crate::types::{
    ModelCategory, ModelError, ModelInfo, ModelProviderType, ModelRequest, ModelResponse,
    ModelStream,
};

/// Configuration routing map binding categories to preferred models.
#[derive(Debug, Clone)]
pub struct ModelRoutingConfig {
    pub default_category: ModelCategory,
    pub category_models: HashMap<ModelCategory, String>,
    pub fallback_category: Option<ModelCategory>,
}

impl Default for ModelRoutingConfig {
    fn default() -> Self {
        let mut category_models = HashMap::new();
        category_models.insert(ModelCategory::Fast, "qwen2.5:3b".to_string());
        category_models.insert(ModelCategory::General, "qwen2.5:7b".to_string());
        category_models.insert(ModelCategory::Reasoning, "qwen2.5:14b".to_string());
        category_models.insert(ModelCategory::ToolCalling, "qwen2.5:7b".to_string());
        category_models.insert(ModelCategory::Vision, "llava:latest".to_string());
        category_models.insert(ModelCategory::Embedding, "nomic-embed-text:latest".to_string());

        Self {
            default_category: ModelCategory::General,
            category_models,
            fallback_category: Some(ModelCategory::Fast),
        }
    }
}

/// Routes model requests across registered providers.
pub struct ModelRouter {
    providers: Vec<Arc<dyn ModelProvider>>,
    config: ModelRoutingConfig,
}

impl ModelRouter {
    pub fn new(config: ModelRoutingConfig) -> Self {
        Self {
            providers: Vec::new(),
            config,
        }
    }

    pub fn with_provider(mut self, provider: Arc<dyn ModelProvider>) -> Self {
        self.providers.push(provider);
        self
    }

    pub fn add_provider(&mut self, provider: Arc<dyn ModelProvider>) {
        self.providers.push(provider);
    }

    /// List all discovered models across all registered providers.
    pub async fn list_all_models(&self) -> Vec<ModelInfo> {
        let mut all = Vec::new();
        for provider in &self.providers {
            if let Ok(models) = provider.list_models().await {
                all.extend(models);
            }
        }
        all
    }

    /// Find an active provider capable of fulfilling the request.
    pub fn resolve_provider(&self, provider_type: Option<ModelProviderType>) -> Option<Arc<dyn ModelProvider>> {
        if let Some(pt) = provider_type {
            self.providers.iter().find(|p| p.provider_type() == pt).cloned()
        } else {
            self.providers.first().cloned()
        }
    }

    /// Route and execute chat generation with automatic fallback.
    pub async fn generate(&self, request: &ModelRequest) -> Result<ModelResponse, ModelError> {
        if self.providers.is_empty() {
            return Err(ModelError::ProviderUnavailable(
                ModelProviderType::Custom("None".to_string()),
                "No AI model providers registered in ModelRouter".to_string(),
            ));
        }

        let target_category = request.category.unwrap_or(self.config.default_category);
        let preferred_model = request.model_id.clone().or_else(|| {
            self.config.category_models.get(&target_category).cloned()
        });

        let mut req_with_model = request.clone();
        req_with_model.model_id = preferred_model.clone();

        // Try primary provider
        let mut last_error = None;
        for (idx, provider) in self.providers.iter().enumerate() {
            match provider.generate(&req_with_model).await {
                Ok(mut resp) => {
                    if idx > 0 {
                        resp.was_fallback = true;
                        resp.fallback_reason = Some(format!(
                            "Primary provider failed, fell back to {:?}",
                            provider.provider_type()
                        ));
                    }
                    return Ok(resp);
                }
                Err(e) => {
                    warn!(
                        provider = ?provider.provider_type(),
                        error = %e,
                        "Provider failed, attempting fallback"
                    );
                    last_error = Some(e);
                }
            }
        }

        // Try secondary category fallback if configured
        if let Some(fb_cat) = self.config.fallback_category {
            if fb_cat != target_category {
                if let Some(fb_model) = self.config.category_models.get(&fb_cat) {
                    info!(
                        from_category = ?target_category,
                        to_category = ?fb_cat,
                        fallback_model = %fb_model,
                        "Attempting category-level fallback"
                    );

                    let mut fb_req = request.clone();
                    fb_req.model_id = Some(fb_model.clone());

                    for provider in &self.providers {
                        if let Ok(mut resp) = provider.generate(&fb_req).await {
                            resp.was_fallback = true;
                            resp.fallback_reason = Some(format!(
                                "Fell back to {:?} category ({}) due to primary failure",
                                fb_cat, fb_model
                            ));
                            return Ok(resp);
                        }
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            ModelError::RuntimeFailure("All configured AI providers and fallbacks failed".to_string())
        }))
    }

    /// Route and execute token streaming.
    pub async fn stream(&self, request: &ModelRequest) -> Result<ModelStream, ModelError> {
        if self.providers.is_empty() {
            return Err(ModelError::ProviderUnavailable(
                ModelProviderType::Custom("None".to_string()),
                "No AI model providers registered in ModelRouter".to_string(),
            ));
        }

        let mut last_error = None;
        for provider in &self.providers {
            match provider.stream(request).await {
                Ok(stream) => return Ok(stream),
                Err(e) => {
                    warn!(
                        provider = ?provider.provider_type(),
                        error = %e,
                        "Stream provider failed, attempting next"
                    );
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            ModelError::RuntimeFailure("All providers failed to initiate stream".to_string())
        }))
    }
}
