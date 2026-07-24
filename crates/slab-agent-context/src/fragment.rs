//! The [`ContextFragment`] trait.
//!
//! A context fragment knows its chat `role`, the minijinja template name to
//! render through, and the serializable context to feed that template. The
//! default `render` produces a [`ConversationMessage`] (the slab wire type),
//! adapting the original `ResponseItem`-shaped pseudocode to the real message
//! type used across the agent pipeline.

use minijinja::Environment;
use slab_types::{ConversationMessage, ConversationMessageContent};

use crate::error::{ContextError, Result};

pub trait ContextFragment {
    /// `"system"`, `"developer"`, or `"user"`.
    fn role(&self) -> &'static str;
    /// Template name registered in the shared [`Environment`].
    fn template_name(&self) -> &'static str;
    /// Serializable context fed to the template.
    fn render_context(&self) -> serde_json::Value;

    /// Render the template body using the shared environment.
    fn render_body(&self, env: &Environment<'_>) -> Result<String> {
        let template = env
            .get_template(self.template_name())
            .map_err(|error| ContextError::Template(error.to_string()))?;
        template
            .render(self.render_context())
            .map_err(|error| ContextError::Template(error.to_string()))
    }

    /// Render the body and wrap it as a [`ConversationMessage`].
    fn render(&self, env: &Environment<'_>) -> Result<ConversationMessage> {
        let body = self.render_body(env)?;
        Ok(ConversationMessage {
            role: self.role().to_owned(),
            content: ConversationMessageContent::Text(body),
            name: None,
            tool_call_id: None,
            tool_calls: Vec::new(),
        })
    }
}
