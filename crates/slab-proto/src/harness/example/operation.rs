/// Submission operation
#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::large_enum_variant)]
#[non_exhaustive]
pub enum UserSubmissionOp {
    /// Abort current task without terminating background terminal processes.
    /// This server sends [`EventMsg::TurnAborted`] in response.
    Interrupt,

    UserInput {
        /// User input items, see `InputItem`
        items: Vec<UserInput>,
        /// Optional JSON Schema used to constrain the final assistant message for this turn.
        final_output_json_schema: Option<Value>,
        /// Optional turn-scoped Responses API `client_metadata`.
        responsesapi_client_metadata: Option<HashMap<String, String>>,
        /// Client-supplied context fragments keyed by an opaque source identifier.
        additional_context: BTreeMap<String, AdditionalContextEntry>,

        /// Persistent thread-settings overrides to apply before the input.
        thread_settings: ThreadSettingsOverrides,
    },
    // Additional variants reserved.
}
