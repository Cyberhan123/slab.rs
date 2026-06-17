use crate::openai::models;
use serde::{Deserialize, Serialize};

/// ApplyPatchOperationParam : One of the create_file, delete_file, or update_file operations supplied to the apply_patch tool.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ApplyPatchOperationParam {
    ApplyPatchCreateFileOperationParam(Box<models::ApplyPatchCreateFileOperationParam>),
    ApplyPatchDeleteFileOperationParam(Box<models::ApplyPatchDeleteFileOperationParam>),
    ApplyPatchUpdateFileOperationParam(Box<models::ApplyPatchUpdateFileOperationParam>),
}

impl Default for ApplyPatchOperationParam {
    fn default() -> Self {
        Self::ApplyPatchCreateFileOperationParam(Default::default())
    }
}

/// ApplyPatchOperation : One of the create_file, delete_file, or update_file operations applied via apply_patch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ApplyPatchOperation {
    ApplyPatchCreateFileOperation(Box<models::ApplyPatchCreateFileOperation>),
    ApplyPatchDeleteFileOperation(Box<models::ApplyPatchDeleteFileOperation>),
    ApplyPatchUpdateFileOperation(Box<models::ApplyPatchUpdateFileOperation>),
}

impl Default for ApplyPatchOperation {
    fn default() -> Self {
        Self::ApplyPatchCreateFileOperation(Default::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn operation_round_trips_create_delete_and_update_wire_shapes() {
        let cases = [
            ApplyPatchOperation::ApplyPatchCreateFileOperation(Box::new(
                models::ApplyPatchCreateFileOperation::new(
                    models::CreateFileOperationType::CreateFile,
                    "src/new.rs".to_owned(),
                    "*** Begin Patch\n*** Add File: src/new.rs\n+test\n*** End Patch\n".to_owned(),
                ),
            )),
            ApplyPatchOperation::ApplyPatchDeleteFileOperation(Box::new(
                models::ApplyPatchDeleteFileOperation::new(
                    models::DeleteFileOperationType::DeleteFile,
                    "src/old.rs".to_owned(),
                ),
            )),
            ApplyPatchOperation::ApplyPatchUpdateFileOperation(Box::new(
                models::ApplyPatchUpdateFileOperation::new(
                    models::UpdateFileOperationType::UpdateFile,
                    "src/lib.rs".to_owned(),
                    "*** Begin Patch\n*** Update File: src/lib.rs\n@@\n-old\n+new\n*** End Patch\n"
                        .to_owned(),
                ),
            )),
        ];

        for operation in cases {
            let value = serde_json::to_value(&operation).expect("serialize operation");
            let decoded: ApplyPatchOperation =
                serde_json::from_value(value.clone()).expect("deserialize operation");
            assert_eq!(decoded, operation);
            assert!(matches!(
                value.get("type").and_then(serde_json::Value::as_str),
                Some("create_file" | "delete_file" | "update_file")
            ));
        }
    }

    #[test]
    fn operation_param_round_trips_wire_shapes() {
        let cases = [
            ApplyPatchOperationParam::ApplyPatchCreateFileOperationParam(Box::new(
                models::ApplyPatchCreateFileOperationParam::new(
                    models::CreateFileOperationParamType::CreateFile,
                    "src/new.rs".to_owned(),
                    "*** Begin Patch\n*** Add File: src/new.rs\n+test\n*** End Patch\n".to_owned(),
                ),
            )),
            ApplyPatchOperationParam::ApplyPatchDeleteFileOperationParam(Box::new(
                models::ApplyPatchDeleteFileOperationParam::new(
                    models::DeleteFileOperationParamType::DeleteFile,
                    "src/old.rs".to_owned(),
                ),
            )),
            ApplyPatchOperationParam::ApplyPatchUpdateFileOperationParam(Box::new(
                models::ApplyPatchUpdateFileOperationParam::new(
                    models::UpdateFileOperationParamType::UpdateFile,
                    "src/lib.rs".to_owned(),
                    "*** Begin Patch\n*** Update File: src/lib.rs\n@@\n-old\n+new\n*** End Patch\n"
                        .to_owned(),
                ),
            )),
        ];

        for operation in cases {
            let value = serde_json::to_value(&operation).expect("serialize operation param");
            let decoded: ApplyPatchOperationParam =
                serde_json::from_value(value.clone()).expect("deserialize operation param");
            assert_eq!(decoded, operation);
            assert!(matches!(
                value.get("type").and_then(serde_json::Value::as_str),
                Some("create_file" | "delete_file" | "update_file")
            ));
        }
    }

    #[test]
    fn operation_rejects_unknown_missing_and_mismatched_shapes() {
        for value in [
            json!({"type":"rename_file","path":"src/lib.rs","diff":"diff"}),
            json!({"type":"create_file","path":"src/lib.rs"}),
            json!({"type":"delete_file","path":123}),
        ] {
            let result = serde_json::from_value::<ApplyPatchOperation>(value);
            assert!(result.is_err());
        }
    }
}
