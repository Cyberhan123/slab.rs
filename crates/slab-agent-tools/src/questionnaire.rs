//! `questionnaire` — ask the user a structured question.
//!
//! The tool itself is a thin validate/normalize shell: its `execute` returns a
//! snapshot of the question (metadata key [`QUESTIONNAIRE_METADATA_KEY`]) and
//! the turn loop (`slab_agent::turn_tool_call`) detects the call, notifies the
//! host via `EventMsg::QuestionnaireRequestAnswer`, and blocks on the
//! [`slab_agent::QuestionnairePort`] until the user answers (or the 300s
//! timeout elapses). The answers JSON is then returned to the LLM as the tool
//! result — mirroring how `present_plan` reuses the approval channel, but with
//! structured answers instead of a Copy approve/reject decision.

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use slab_agent::{AgentError, ToolContext, ToolOutput, TypedTool};

/// Tool name. Mirrored as a `const` in `slab_agent::turn_tool_call` for the
/// loop-side gate (slab-agent cannot depend on this crate).
pub const QUESTIONNAIRE_TOOL_NAME: &str = "questionnaire";

/// Metadata key under which the tool stashes the question snapshot so the turn
/// loop can detect the call and drive the answer gate. Mirrored in
/// `slab_agent::turn_tool_call`.
pub const QUESTIONNAIRE_METADATA_KEY: &str = "questionnaire";

/// Upper bound on choices: a questionnaire card with more options stops being
/// a quick question and becomes a survey the user has to read.
pub(crate) const MAX_CHOICES: usize = 12;

/// Arguments for the `questionnaire` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct QuestionnaireArgs {
    /// The self-contained question text the user sees (include any context the
    /// user needs — it may be shown without the surrounding conversation).
    question: String,
    /// Answer options. Keep the list short and clearly distinct.
    choices: Vec<QuestionnaireChoiceDef>,
    /// Allow selecting multiple choices (default single-select).
    #[serde(default)]
    allow_multiple: bool,
    /// Also let the user type a custom answer (default no free text).
    #[serde(default)]
    allow_custom_input: bool,
    /// Whether the user must answer (informational for the UI; a timeout is
    /// still a valid outcome for the model).
    #[serde(default)]
    required: bool,
}

/// One answer option of a questionnaire.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[schemars(inline)]
pub struct QuestionnaireChoiceDef {
    /// Short human-readable label shown as the option.
    label: String,
    /// Stable machine value for the choice; defaults to `label` when omitted.
    #[serde(default)]
    value: Option<String>,
    /// Optional one-line explanation shown under the label.
    #[serde(default)]
    description: Option<String>,
}

/// Ask the user a structured question (single/multiple choice, optional custom
/// input). The turn loop intercepts the call and awaits the user's answer.
#[derive(Default)]
pub struct QuestionnaireTool;

impl QuestionnaireTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TypedTool for QuestionnaireTool {
    type Input = QuestionnaireArgs;
    fn name(&self) -> &str {
        QUESTIONNAIRE_TOOL_NAME
    }

    /// The call blocks on the user — it must not enter a parallel batch.
    fn is_concurrency_safe(&self, _arguments: &serde_json::Value) -> bool {
        false
    }

    fn description(&self) -> &str {
        "Ask the user a structured question and wait for their answer. Use when a decision \
belongs to the user: requirements are ambiguous, multiple acceptable approaches trade off \
differently, or taste/risk acceptance is involved — do not decide these yourself. Offer a \
few (max 12), clearly distinct choices; set allow_multiple or allow_custom_input when the \
question needs them. The tool result is the user's answer (or a timeout notice if they did \
not answer within 300s — then proceed with your best judgment)."
    }

    /// Asking a question is a read-only interaction — it must not trip the
    /// exec-policy approval flow (the questionnaire itself IS the user gate).
    fn category(&self) -> slab_agent::OperationCategory {
        slab_agent::OperationCategory::ReadOnly
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        args: QuestionnaireArgs,
    ) -> Result<ToolOutput, AgentError> {
        let question = args.question.trim().to_owned();
        if question.is_empty() {
            return Err(AgentError::ToolExecution(
                "questionnaire question must not be blank".to_owned(),
            ));
        }
        if args.choices.is_empty() {
            return Err(AgentError::ToolExecution(
                "questionnaire requires at least one choice".to_owned(),
            ));
        }
        if args.choices.len() > MAX_CHOICES {
            return Err(AgentError::ToolExecution(format!(
                "questionnaire accepts at most {MAX_CHOICES} choices (got {})",
                args.choices.len()
            )));
        }

        let mut seen_labels: Vec<String> = Vec::with_capacity(args.choices.len());
        let mut choices: Vec<serde_json::Value> = Vec::with_capacity(args.choices.len());
        for choice in args.choices {
            let label = choice.label.trim().to_owned();
            if label.is_empty() {
                return Err(AgentError::ToolExecution(
                    "questionnaire choice label must not be blank".to_owned(),
                ));
            }
            if seen_labels.iter().any(|seen| seen.eq_ignore_ascii_case(&label)) {
                return Err(AgentError::ToolExecution(format!(
                    "questionnaire choice labels must be unique: '{label}'"
                )));
            }
            seen_labels.push(label.clone());
            let value = choice
                .value
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or(&label)
                .to_owned();
            let mut entry = serde_json::json!({ "label": label, "value": value });
            if let Some(description) = choice.description
                && !description.trim().is_empty()
            {
                entry["description"] = serde_json::Value::String(description.trim().to_owned());
            }
            choices.push(entry);
        }

        let snapshot = serde_json::json!({
            "question": question,
            "choices": choices,
            "allow_multiple": args.allow_multiple,
            "allow_custom_input": args.allow_custom_input,
            "required": args.required,
        });
        Ok(ToolOutput {
            content: "questionnaire presented to the user; awaiting response".to_owned(),
            metadata: Some(serde_json::json!({ QUESTIONNAIRE_METADATA_KEY: snapshot })),
        })
    }
}

#[cfg(test)]
mod tests {
    use slab_agent::ToolHandler;

    use super::*;

    fn ctx() -> ToolContext {
        ToolContext::for_thread("t1").build()
    }

    fn sample_args() -> serde_json::Value {
        serde_json::json!({
            "question": "Which database?",
            "choices": [
                { "label": "SQLite", "description": " zero-config embedded " },
                { "label": "Postgres", "value": "pg" }
            ],
            "allow_multiple": false,
            "allow_custom_input": true
        })
    }

    #[test]
    fn schema_requires_question_and_choices() {
        let schema = ToolHandler::parameters_schema(&QuestionnaireTool::new());
        assert_eq!(schema["required"], serde_json::json!(["question", "choices"]));
        assert_eq!(
            schema["properties"]["choices"]["items"]["properties"]["label"]["type"],
            "string"
        );
        // Optional choice fields are nullable; flags default to false.
        assert_eq!(schema["properties"]["allow_multiple"]["default"], false);
        assert_eq!(schema["properties"]["allow_custom_input"]["default"], false);
    }

    #[tokio::test]
    async fn execute_validates_choice_rules() {
        let cases = [
            (serde_json::json!({"question": "Q", "choices": []}), "at least one choice"),
            (
                serde_json::json!({"question": " ", "choices": [{"label": "a"}]}),
                "must not be blank",
            ),
            (
                serde_json::json!({"question": "Q", "choices": [{"label": " "}]}),
                "label must not be blank",
            ),
            (
                serde_json::json!({"question": "Q", "choices": [{"label": "a"}, {"label": "A"}]}),
                "labels must be unique",
            ),
            (
                serde_json::json!({
                    "question": "Q",
                    "choices": (0..13).map(|i| serde_json::json!({"label": format!("c{i}")})).collect::<Vec<_>>()
                }),
                "at most 12 choices",
            ),
        ];
        for (arguments, expected) in cases {
            let error = ToolHandler::execute(&QuestionnaireTool::new(), &ctx(), &arguments)
                .await
                .expect_err("invalid questionnaire arguments");
            assert!(error.to_string().contains(expected), "{error}");
        }
    }

    #[tokio::test]
    async fn snapshot_backfills_value_and_carries_flags() {
        let output = ToolHandler::execute(&QuestionnaireTool::new(), &ctx(), &sample_args())
            .await
            .expect("questionnaire output");
        assert_eq!(output.content, "questionnaire presented to the user; awaiting response");
        let snapshot = &output.metadata.expect("metadata")[QUESTIONNAIRE_METADATA_KEY];
        assert_eq!(snapshot["question"], "Which database?");
        // value defaults back to the label; an explicit value is trimmed and kept.
        assert_eq!(snapshot["choices"][0]["label"], "SQLite");
        assert_eq!(snapshot["choices"][0]["value"], "SQLite");
        assert_eq!(snapshot["choices"][0]["description"], "zero-config embedded");
        assert_eq!(snapshot["choices"][1]["value"], "pg");
        // An omitted description stays off the snapshot entirely.
        assert!(snapshot["choices"][1].get("description").is_none());
        assert_eq!(snapshot["allow_multiple"], false);
        assert_eq!(snapshot["allow_custom_input"], true);
        // `required` defaults to false when omitted.
        assert_eq!(snapshot["required"], false);
    }
}
