use std::sync::Arc;

use serde_json::Value;
use tracing::instrument;

use crate::router::ModelRouter;
use crate::types::{
    ChatMessage, ModelCategory, ModelError, ModelRequest, ModelResponse, ModelStream,
    ModelToolCall, ResponseFormat, ToolSchema,
};

/// High-level AI Gateway.
pub struct ModelGateway {
    router: Arc<ModelRouter>,
}

impl ModelGateway {
    pub fn new(router: Arc<ModelRouter>) -> Self {
        Self { router }
    }

    /// Access the underlying model router.
    pub fn router(&self) -> &Arc<ModelRouter> {
        &self.router
    }

    /// Single-turn question or command query.
    #[instrument(skip(self), fields(prompt = %prompt))]
    pub async fn ask(&self, prompt: &str) -> Result<String, ModelError> {
        let request = ModelRequest::new()
            .with_category(ModelCategory::General)
            .with_message(ChatMessage::user(prompt));

        let resp = self.router.generate(&request).await?;
        Ok(resp.text)
    }

    /// Multi-turn conversation request.
    #[instrument(skip(self, messages))]
    pub async fn chat(&self, messages: Vec<ChatMessage>) -> Result<ModelResponse, ModelError> {
        let mut request = ModelRequest::new().with_category(ModelCategory::General);
        request.messages = messages;

        self.router.generate(&request).await
    }

    /// Streaming conversation request.
    pub async fn chat_stream(&self, messages: Vec<ChatMessage>) -> Result<ModelStream, ModelError> {
        let mut request = ModelRequest::new().with_category(ModelCategory::General);
        request.messages = messages;

        self.router.stream(&request).await
    }

    /// Extract structured intent and tool execution plan from natural language.
    #[instrument(skip(self, tools), fields(command = %raw_command))]
    pub async fn plan_action(
        &self,
        raw_command: &str,
        tools: Vec<ToolSchema>,
    ) -> Result<Vec<ModelToolCall>, ModelError> {
        let system_prompt = "You are JARVIS, an autonomous personal assistant. Analyze the user's command and propose the appropriate tool calls. Do not execute anything yourself; output structured tool calls.";

        let request = ModelRequest::new()
            .with_category(ModelCategory::ToolCalling)
            .with_message(ChatMessage::system(system_prompt))
            .with_message(ChatMessage::user(raw_command))
            .with_tools(tools);

        let resp = self.router.generate(&request).await?;
        Ok(resp.tool_calls)
    }

    /// Request structured JSON extraction conforming to a specified schema.
    #[instrument(skip(self, schema))]
    pub async fn extract_json(
        &self,
        prompt: &str,
        schema: Option<Value>,
    ) -> Result<String, ModelError> {
        let mut request = ModelRequest::new()
            .with_category(ModelCategory::Fast)
            .with_message(ChatMessage::user(prompt));

        if let Some(s) = schema {
            request.response_format = Some(ResponseFormat::JsonSchema(s));
        } else {
            request.response_format = Some(ResponseFormat::JsonObject);
        }

        let resp = self.router.generate(&request).await?;
        Ok(resp.text)
    }

    /// Analyze an image or screenshot using a VisionModelProvider.
    #[instrument(skip(self, vision_provider, request))]
    pub async fn analyze_image(
        &self,
        vision_provider: &dyn crate::vision::VisionModelProvider,
        request: &crate::vision::VisionRequest,
    ) -> Result<crate::vision::VisionResponse, ModelError> {
        vision_provider.analyze_image(request).await
    }
}
