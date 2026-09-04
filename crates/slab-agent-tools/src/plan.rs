//! Plan-and-execute tools: `plan`, `update_plan`, `present_plan`.
//!
//! In Plan interaction mode the agent explores read-only, drafts a durable plan
//! with `plan`, refines it with `update_plan`, then calls `present_plan` to
//! request user approval. The durable plan lives in the per-thread
//! [`slab_agent::PlanStorePort`] (injected into each [`ToolContext`]); these tools are thin
//! validate/normalize/persist shells over it. The approval gate itself is
//! driven by the agent turn loop (`slab_agent::turn_tool_call`), which detects
//! `present_plan` and reuses the existing approval channel — see
//! [`PRESENT_PLAN_METADATA_KEY`].

use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;
use slab_agent::{
    AgentError, Plan, PlanCounts, PlanItem, PlanStatus, ToolContext, ToolOutput, TypedTool,
    typed_input_schema,
};

/// Metadata key under which `present_plan` stashes the plan snapshot so the
/// turn loop can detect the call and drive the approval gate. Mirrored as a
/// `const` in `slab_agent::turn_tool_call` (slab-agent cannot depend on this crate).
pub const PRESENT_PLAN_METADATA_KEY: &str = "present_plan";

/// Tool names. Mirrored in `slab_agent::turn_tool_call` for the loop-side gate.
pub const PLAN_TOOL_NAME: &str = "plan";
pub const UPDATE_PLAN_TOOL_NAME: &str = "update_plan";
pub const PRESENT_PLAN_TOOL_NAME: &str = "present_plan";

static PLAN_SEQ: AtomicU64 = AtomicU64::new(0);

fn next_plan_id() -> String {
    format!("plan-{}", PLAN_SEQ.fetch_add(1, Ordering::Relaxed))
}

// ── Shared argument shape ────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PlanArgs {
    /// Optional short summary of what this plan is tracking.
    summary: Option<String>,
    #[serde(deserialize_with = "deserialize_items")]
    #[schemars(length(min = 1))]
    items: Vec<PlanItemInput>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(inline)]
struct PlanItemInput {
    /// A concrete task or checkpoint.
    step: String,
    status: PlanStatus,
    /// Optional step references this step depends on (lightweight, not enforced).
    depends_on: Option<Vec<String>>,
    /// Optional verify:<target>:<passed|failed> reference binding execution evidence to this step.
    result_ref: Option<String>,
}

/// Schema-only shape for `present_plan`'s no-argument call: the schema
/// declares a closed empty object, while execution keeps tolerating stray
/// arguments (see [`PresentPlanTool::parameters_schema`]).
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PresentPlanArgs {}

/// Tolerant deserializer for the plan `items` array. Smaller models sometimes
/// emit the array as a JSON-encoded string (e.g. Qwen3.5-9B sends
/// `"items": "[{\"step\": ...}]"`) instead of a real array. Accept both shapes:
/// a real array passes through; a string is parsed back into the typed vec so
/// plan mode works end-to-end without forcing every model to emit a perfect
/// array. Applies to both `plan` and `update_plan` (they share [`PlanArgs`]).
fn deserialize_items<'de, D>(deserializer: D) -> Result<Vec<PlanItemInput>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error as _;

    let value = Value::deserialize(deserializer)?;
    match value {
        Value::String(raw) => serde_json::from_str::<Vec<PlanItemInput>>(&raw).map_err(|error| {
            D::Error::custom(format!("items string is not a JSON array: {error}"))
        }),
        // Parse element-wise so a bad item reports its POSITION: a bare
        // "missing field `status`" (serde names the first missing declared
        // field) gave no hint of WHICH item failed and read as nonsense when
        // the real problem was a typo three items down.
        other => {
            let raw_items = serde_json::from_value::<Vec<Value>>(other).map_err(|error| {
                D::Error::custom(format!("items must be a JSON array of plan items: {error}"))
            })?;
            let mut items = Vec::with_capacity(raw_items.len());
            for (index, raw_item) in raw_items.into_iter().enumerate() {
                items.push(serde_json::from_value::<PlanItemInput>(raw_item).map_err(|error| {
                    D::Error::custom(format!("items[{index}] is not a valid plan item: {error}"))
                })?);
            }
            Ok(items)
        }
    }
}

/// Validate + normalize the raw args into a typed [`Plan`] with the given id.
///
/// Pure (no I/O): trims steps, collapses blank `result_ref`/`depends_on` to
/// `None`, counts statuses, rejects empty plans / blank steps / more than one
/// in-progress step, and records the current (in-progress) step index.
fn build_plan(args: PlanArgs, plan_id: String) -> Result<Plan, AgentError> {
    if args.items.is_empty() {
        return Err(AgentError::ToolExecution("plan requires at least one item".to_owned()));
    }

    let mut counts = PlanCounts::default();
    let mut current_step = None;
    let mut items = Vec::with_capacity(args.items.len());

    for item in args.items {
        let step = item.step.trim().to_owned();
        if step.is_empty() {
            return Err(AgentError::ToolExecution("plan item step must not be blank".to_owned()));
        }
        match item.status {
            PlanStatus::Pending => counts.pending += 1,
            PlanStatus::InProgress => {
                counts.in_progress += 1;
                if current_step.is_none() {
                    current_step = Some(items.len());
                }
            }
            PlanStatus::Completed => counts.completed += 1,
            PlanStatus::Blocked => counts.blocked += 1,
        }
        let result_ref = item
            .result_ref
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned);
        let depends_on = item.depends_on.filter(|deps| !deps.is_empty());
        items.push(PlanItem { step, status: item.status, depends_on, result_ref });
    }

    if counts.in_progress > 1 {
        return Err(AgentError::ToolExecution(
            "plan accepts at most one in_progress item".to_owned(),
        ));
    }

    Ok(Plan {
        plan_id,
        summary: args
            .summary
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned),
        items,
        counts,
        current_step,
    })
}

// ── plan ─────────────────────────────────────────────────────────────────────

/// Create the structured execution plan (Plan interaction mode).
#[derive(Default)]
pub struct PlanTool;

impl PlanTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TypedTool for PlanTool {
    type Input = PlanArgs;
    fn name(&self) -> &str {
        PLAN_TOOL_NAME
    }

    fn description(&self) -> &str {
        "Create a structured execution plan. Use in Plan mode to lay out the steps before \
executing. Once the plan is ready for the user, call present_plan; as you complete each step \
call update_plan to record progress."
    }

    fn parameters_schema(&self) -> Value {
        typed_input_schema::<PlanArgs>()
    }

    async fn execute(&self, ctx: &ToolContext, args: PlanArgs) -> Result<ToolOutput, AgentError> {
        let plan = build_plan(args, next_plan_id())?;
        let snapshot = serde_json::to_value(&plan)
            .map_err(|error| AgentError::ToolExecution(error.to_string()))?;
        ctx.plan_store
            .replace_plan(&ctx.thread_id, plan.clone())
            .await
            .map_err(|error| AgentError::ToolExecution(format!("plan store error: {error}")))?;
        Ok(ToolOutput {
            content: format!("plan created: {}", plan.summary_line()),
            metadata: Some(snapshot),
        })
    }
}

// ── update_plan ──────────────────────────────────────────────────────────────

/// Update the plan incrementally (mark a step done, replan, add steps). Sends
/// the full updated item list; the existing plan id is preserved.
#[derive(Default)]
pub struct UpdatePlanTool;

impl UpdatePlanTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TypedTool for UpdatePlanTool {
    type Input = PlanArgs;
    fn name(&self) -> &str {
        UPDATE_PLAN_TOOL_NAME
    }

    fn description(&self) -> &str {
        "Update the current execution plan: mark a step done, replan, or add steps. Pass the full \
updated item list. Preserve the plan id across updates."
    }

    fn parameters_schema(&self) -> Value {
        typed_input_schema::<PlanArgs>()
    }

    async fn execute(&self, ctx: &ToolContext, args: PlanArgs) -> Result<ToolOutput, AgentError> {
        // Preserve the existing plan id when a plan already exists; otherwise mint one.
        let plan_id = ctx
            .plan_store
            .current_plan(&ctx.thread_id)
            .await
            .map(|existing| existing.plan_id)
            .unwrap_or_else(next_plan_id);
        let plan = build_plan(args, plan_id)?;
        let snapshot = serde_json::to_value(&plan)
            .map_err(|error| AgentError::ToolExecution(error.to_string()))?;
        ctx.plan_store
            .replace_plan(&ctx.thread_id, plan.clone())
            .await
            .map_err(|error| AgentError::ToolExecution(format!("plan store error: {error}")))?;
        Ok(ToolOutput {
            content: format!("plan updated: {}", plan.summary_line()),
            metadata: Some(snapshot),
        })
    }
}

// ── present_plan ─────────────────────────────────────────────────────────────

/// Present the current plan for user approval. The turn loop detects this call
/// (via [`PRESENT_PLAN_METADATA_KEY`]) and drives the approval gate: on approval
/// the thread flips to Default interaction mode (mutation tools unlock); on
/// rejection it stays in Plan mode with feedback.
#[derive(Default)]
pub struct PresentPlanTool;

impl PresentPlanTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TypedTool for PresentPlanTool {
    type Input = serde_json::Value;
    fn name(&self) -> &str {
        PRESENT_PLAN_TOOL_NAME
    }

    fn description(&self) -> &str {
        "Present the current plan for user approval. Call this once the plan is ready. On approval \
mutation tools unlock for execution; if rejected, revise the plan and call present_plan again."
    }

    fn parameters_schema(&self) -> Value {
        // The closed-empty-object schema is advisory; execution ignores any
        // arguments, so parsing stays on the raw Value (not PresentPlanArgs,
        // whose deny_unknown_fields would newly reject stray keys).
        typed_input_schema::<PresentPlanArgs>()
    }

    async fn execute(
        &self,
        ctx: &ToolContext,
        _arguments: serde_json::Value,
    ) -> Result<ToolOutput, AgentError> {
        let plan = ctx.plan_store.current_plan(&ctx.thread_id).await.ok_or_else(|| {
            AgentError::ToolExecution(
                "present_plan: no plan created yet; call `plan` first".to_owned(),
            )
        })?;
        let snapshot = serde_json::to_value(&plan)
            .map_err(|error| AgentError::ToolExecution(error.to_string()))?;
        Ok(ToolOutput {
            content: format!("presenting plan for approval: {}", plan.summary_line()),
            metadata: Some(serde_json::json!({ PRESENT_PLAN_METADATA_KEY: snapshot })),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use serde_json::{Value, json};
    use slab_agent::{NoopPlanStore, PlanStorePort, ToolContext, ToolHandler};

    use super::*;

    /// Minimal recording plan store for unit tests (mirrors the app-core in-memory impl).
    #[derive(Default)]
    struct RecordingPlanStore {
        plan: Mutex<Option<Plan>>,
    }

    #[async_trait::async_trait]
    impl PlanStorePort for RecordingPlanStore {
        async fn replace_plan(&self, _thread_id: &str, plan: Plan) -> Result<(), AgentError> {
            *self.plan.lock().unwrap() = Some(plan);
            Ok(())
        }
        async fn current_plan(&self, _thread_id: &str) -> Option<Plan> {
            self.plan.lock().unwrap().clone()
        }
        async fn clear(&self, _thread_id: &str) {
            *self.plan.lock().unwrap() = None;
        }
    }

    fn ctx_with_store(store: Arc<dyn PlanStorePort>) -> ToolContext {
        ToolContext::for_thread("thread").plan_store(store).build()
    }

    fn noop_ctx() -> ToolContext {
        ToolContext::for_thread("thread").build()
    }

    #[test]
    fn plan_items_error_reports_the_failing_index() {
        let error = serde_json::from_value::<PlanArgs>(json!({
            "items": [
                { "step": "ok", "status": "completed", "result_ref": "", "depends_on": [] },
                { "step": "bad" }
            ]
        }))
        .expect_err("second item is missing its status");
        // The bare "missing field `status`" gave no hint of WHICH item failed;
        // the position must be in the message.
        assert!(error.to_string().contains("items[1]"), "{}", error);
    }

    #[test]
    fn plan_status_accepts_pascal_case_aliases() {
        let args: PlanArgs = serde_json::from_value(json!({
            "items": [{ "step": "s", "status": "Completed" }]
        }))
        .expect("PascalCase status accepted");
        assert_eq!(args.items[0].status, PlanStatus::Completed);
    }

    fn sample_args() -> Value {
        json!({
            "summary": "  code change  ",
            "items": [
                { "step": " inspect ", "status": "completed" },
                { "step": "implement", "status": "in_progress" },
                { "step": "verify", "status": "pending" }
            ]
        })
    }

    #[tokio::test]
    async fn plan_creates_durable_plan() {
        let store = Arc::new(RecordingPlanStore::default()) as Arc<dyn PlanStorePort>;
        let ctx = ctx_with_store(store.clone());
        let output = ToolHandler::execute(&PlanTool::new(), &ctx, &sample_args())
            .await
            .expect("plan output");

        // The store now holds the plan (durable + queryable).
        let stored = store.current_plan("thread").await.expect("plan persisted");
        assert_eq!(stored.summary.as_deref(), Some("code change"));
        assert_eq!(stored.counts.completed, 1);
        assert_eq!(stored.counts.in_progress, 1);
        assert_eq!(stored.counts.pending, 1);
        assert_eq!(stored.current_step, Some(1));
        assert!(stored.plan_id.starts_with("plan-"));
        // Metadata carries the same structured plan.
        let metadata = output.metadata.expect("metadata");
        assert_eq!(metadata["summary"], "code change");
        assert_eq!(metadata["counts"]["pending"], 1);
    }

    #[tokio::test]
    async fn plan_accepts_items_as_a_stringified_json_array() {
        // Smaller models (observed with Qwen3.5-9B in the Slice 5 e2e) sometimes
        // emit the `items` array as a JSON-encoded string instead of a real
        // array. The tolerant deserializer parses it back so plan mode completes
        // end-to-end instead of erroring on `invalid type: string`.
        let store = Arc::new(RecordingPlanStore::default()) as Arc<dyn PlanStorePort>;
        let ctx = ctx_with_store(store.clone());
        let stringified = json!([
            { "step": "inspect", "status": "completed" },
            { "step": "implement", "status": "in_progress" }
        ])
        .to_string();
        ToolHandler::execute(
            &PlanTool::new(),
            &ctx,
            &json!({ "summary": "stringified", "items": stringified }),
        )
        .await
        .expect("plan accepts stringified items");
        let stored = store.current_plan("thread").await.expect("plan persisted");
        assert_eq!(stored.summary.as_deref(), Some("stringified"));
        assert_eq!(stored.items.len(), 2);
        assert_eq!(stored.counts.completed, 1);
        assert_eq!(stored.counts.in_progress, 1);
    }

    #[tokio::test]
    async fn update_plan_preserves_plan_id_and_marks_done() {
        let store = Arc::new(RecordingPlanStore::default()) as Arc<dyn PlanStorePort>;
        let ctx = ctx_with_store(store.clone());
        ToolHandler::execute(&PlanTool::new(), &ctx, &sample_args()).await.unwrap();
        let original_id = store.current_plan("thread").await.unwrap().plan_id;

        // Mark implement completed, verify in progress.
        ToolHandler::execute(
            &UpdatePlanTool::new(),
            &ctx,
            &json!({
                "items": [
                    { "step": "inspect", "status": "completed" },
                    { "step": "implement", "status": "completed" },
                    { "step": "verify", "status": "in_progress" }
                ]
            }),
        )
        .await
        .expect("update output");

        let stored = store.current_plan("thread").await.expect("plan still present");
        assert_eq!(stored.plan_id, original_id, "plan id preserved across update");
        assert_eq!(stored.counts.completed, 2);
        assert_eq!(stored.counts.in_progress, 1);
    }

    #[tokio::test]
    async fn update_plan_works_without_prior_plan() {
        // update_plan mints a plan id if no plan exists yet.
        let store = Arc::new(RecordingPlanStore::default()) as Arc<dyn PlanStorePort>;
        let ctx = ctx_with_store(store.clone());
        ToolHandler::execute(
            &UpdatePlanTool::new(),
            &ctx,
            &json!({ "items": [{ "step": "solo", "status": "pending" }] }),
        )
        .await
        .expect("update output");
        assert!(store.current_plan("thread").await.is_some());
    }

    #[tokio::test]
    async fn present_plan_reads_current_plan() {
        let store = Arc::new(RecordingPlanStore::default()) as Arc<dyn PlanStorePort>;
        let ctx = ctx_with_store(store.clone());
        ToolHandler::execute(&PlanTool::new(), &ctx, &sample_args()).await.unwrap();

        let output = ToolHandler::execute(&PresentPlanTool::new(), &ctx, &json!({}))
            .await
            .expect("present output");
        let metadata = output.metadata.expect("metadata");
        assert_eq!(metadata[PRESENT_PLAN_METADATA_KEY]["summary"], "code change");
    }

    #[tokio::test]
    async fn present_plan_errors_without_a_plan() {
        let ctx = noop_ctx();
        let error = ToolHandler::execute(&PresentPlanTool::new(), &ctx, &json!({}))
            .await
            .expect_err("no plan");
        assert!(error.to_string().contains("no plan"));
    }

    #[tokio::test]
    async fn build_plan_rejects_multiple_in_progress() {
        let store = Arc::new(RecordingPlanStore::default()) as Arc<dyn PlanStorePort>;
        let ctx = ctx_with_store(store);
        let error = ToolHandler::execute(
            &PlanTool::new(),
            &ctx,
            &json!({
                "items": [
                    { "step": "one", "status": "in_progress" },
                    { "step": "two", "status": "in_progress" }
                ]
            }),
        )
        .await
        .expect_err("multiple in progress rejected");
        assert!(error.to_string().contains("at most one in_progress"));
    }

    #[tokio::test]
    async fn build_plan_rejects_blank_steps_and_empty_items() {
        let store = Arc::new(RecordingPlanStore::default()) as Arc<dyn PlanStorePort>;
        let ctx = ctx_with_store(store);
        let blank = ToolHandler::execute(
            &PlanTool::new(),
            &ctx,
            &json!({ "items": [{ "step": " ", "status": "pending" }] }),
        )
        .await
        .expect_err("blank step rejected");
        assert!(blank.to_string().contains("step must not be blank"));

        let store2 = Arc::new(RecordingPlanStore::default()) as Arc<dyn PlanStorePort>;
        let ctx2 = ctx_with_store(store2);
        let empty = ToolHandler::execute(&PlanTool::new(), &ctx2, &json!({ "items": [] }))
            .await
            .expect_err("empty items rejected");
        assert!(empty.to_string().contains("at least one item"));
    }

    #[tokio::test]
    async fn build_plan_preserves_trimmed_result_ref_and_depends_on() {
        let store = Arc::new(RecordingPlanStore::default()) as Arc<dyn PlanStorePort>;
        let ctx = ctx_with_store(store);
        ToolHandler::execute(
            &PlanTool::new(),
                &ctx,
                &json!({
                    "items": [
                        { "step": "verify", "status": "completed", "result_ref": "  verify:lint:passed  ", "depends_on": ["implement"] },
                        { "step": "ship", "status": "in_progress", "result_ref": "" }
                    ]
                }),
            )
            .await
            .unwrap();
        let plan = ctx.plan_store.current_plan("thread").await.unwrap();
        assert_eq!(plan.items[0].result_ref.as_deref(), Some("verify:lint:passed"));
        assert_eq!(plan.items[0].depends_on.as_deref(), Some(&["implement".to_owned()][..]));
        // Blank result_ref normalizes to None.
        assert!(plan.items[1].result_ref.is_none());
    }

    #[test]
    fn schema_declares_result_ref_and_depends_on() {
        let schema = ToolHandler::parameters_schema(&PlanTool::new());
        let item_props = &schema["properties"]["items"]["items"]["properties"];
        assert_eq!(schema["required"], json!(["items"]));
        assert_eq!(
            item_props["status"]["enum"],
            json!(["pending", "in_progress", "completed", "blocked"])
        );
        // The schema gap from the toy plan_update is fixed: both optional fields are declared
        // (optional = nullable in the generated schema).
        assert_eq!(item_props["result_ref"]["type"], json!(["string", "null"]));
        assert_eq!(item_props["depends_on"]["type"], json!(["array", "null"]));
    }

    #[tokio::test]
    async fn noop_plan_store_is_the_default_context() {
        // For non-plan tool paths the builder default is a no-op store (None reads).
        let ctx = ToolContext::for_thread("t").build();
        let none = ctx.plan_store.current_plan("t").await;
        assert!(none.is_none());
        // NoopPlanStore is also reachable directly for explicit wiring.
        let _: Arc<dyn PlanStorePort> = Arc::new(NoopPlanStore);
    }
}
