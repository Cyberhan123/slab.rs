use serde::{Deserialize, Serialize};

/// ApplyPatchCreateFileOperationParam : Instruction for creating a new file via the apply_patch tool.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ApplyPatchCreateFileOperationParam {
    /// The operation type. Always `create_file`.
    #[serde(rename = "type")]
    pub r#type: CreateFileOperationParamType,
    /// Path of the file to create relative to the workspace root.
    #[serde(rename = "path")]
    pub path: String,
    /// Unified diff content to apply when creating the file.
    #[serde(rename = "diff")]
    pub diff: String,
}

impl ApplyPatchCreateFileOperationParam {
    /// Instruction for creating a new file via the apply_patch tool.
    pub fn new(
        r#type: CreateFileOperationParamType,
        path: String,
        diff: String,
    ) -> ApplyPatchCreateFileOperationParam {
        ApplyPatchCreateFileOperationParam { r#type, path, diff }
    }
}
/// The operation type. Always `create_file`.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum CreateFileOperationParamType {
    #[serde(rename = "create_file")]
    #[default]
    CreateFile,
}

/// ApplyPatchCreateFileOperation : Instruction describing how to create a file via the apply_patch tool.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ApplyPatchCreateFileOperation {
    /// Create a new file with the provided diff.
    #[serde(rename = "type")]
    pub r#type: CreateFileOperationType,
    /// Path of the file to create.
    #[serde(rename = "path")]
    pub path: String,
    /// Diff to apply.
    #[serde(rename = "diff")]
    pub diff: String,
}

impl ApplyPatchCreateFileOperation {
    /// Instruction describing how to create a file via the apply_patch tool.
    pub fn new(
        r#type: CreateFileOperationType,
        path: String,
        diff: String,
    ) -> ApplyPatchCreateFileOperation {
        ApplyPatchCreateFileOperation { r#type, path, diff }
    }
}
/// Create a new file with the provided diff.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum CreateFileOperationType {
    #[serde(rename = "create_file")]
    #[default]
    CreateFile,
}

/// ApplyPatchDeleteFileOperationParam : Instruction for deleting an existing file via the apply_patch tool.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ApplyPatchDeleteFileOperationParam {
    /// The operation type. Always `delete_file`.
    #[serde(rename = "type")]
    pub r#type: DeleteFileOperationParamType,
    /// Path of the file to delete relative to the workspace root.
    #[serde(rename = "path")]
    pub path: String,
}

impl ApplyPatchDeleteFileOperationParam {
    /// Instruction for deleting an existing file via the apply_patch tool.
    pub fn new(
        r#type: DeleteFileOperationParamType,
        path: String,
    ) -> ApplyPatchDeleteFileOperationParam {
        ApplyPatchDeleteFileOperationParam { r#type, path }
    }
}
/// The operation type. Always `delete_file`.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum DeleteFileOperationParamType {
    #[serde(rename = "delete_file")]
    #[default]
    DeleteFile,
}

/// ApplyPatchDeleteFileOperation : Instruction describing how to delete a file via the apply_patch tool.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ApplyPatchDeleteFileOperation {
    /// Delete the specified file.
    #[serde(rename = "type")]
    pub r#type: DeleteFileOperationType,
    /// Path of the file to delete.
    #[serde(rename = "path")]
    pub path: String,
}

impl ApplyPatchDeleteFileOperation {
    /// Instruction describing how to delete a file via the apply_patch tool.
    pub fn new(r#type: DeleteFileOperationType, path: String) -> ApplyPatchDeleteFileOperation {
        ApplyPatchDeleteFileOperation { r#type, path }
    }
}

/// Delete the specified file.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum DeleteFileOperationType {
    #[serde(rename = "delete_file")]
    #[default]
    DeleteFile,
}

/// ApplyPatchUpdateFileOperationParam : Instruction for updating an existing file via the apply_patch tool.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ApplyPatchUpdateFileOperationParam {
    /// The operation type. Always `update_file`.
    #[serde(rename = "type")]
    pub r#type: UpdateFileOperationParamType,
    /// Path of the file to update relative to the workspace root.
    #[serde(rename = "path")]
    pub path: String,
    /// Unified diff content to apply to the existing file.
    #[serde(rename = "diff")]
    pub diff: String,
}

impl ApplyPatchUpdateFileOperationParam {
    /// Instruction for updating an existing file via the apply_patch tool.
    pub fn new(
        r#type: UpdateFileOperationParamType,
        path: String,
        diff: String,
    ) -> ApplyPatchUpdateFileOperationParam {
        ApplyPatchUpdateFileOperationParam { r#type, path, diff }
    }
}
/// The operation type. Always `update_file`.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum UpdateFileOperationParamType {
    #[serde(rename = "update_file")]
    #[default]
    UpdateFile,
}

/// ApplyPatchUpdateFileOperation : Instruction describing how to update a file via the apply_patch tool.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ApplyPatchUpdateFileOperation {
    /// Update an existing file with the provided diff.
    #[serde(rename = "type")]
    pub r#type: UpdateFileOperationType,
    /// Path of the file to update.
    #[serde(rename = "path")]
    pub path: String,
    /// Diff to apply.
    #[serde(rename = "diff")]
    pub diff: String,
}

impl ApplyPatchUpdateFileOperation {
    /// Instruction describing how to update a file via the apply_patch tool.
    pub fn new(
        r#type: UpdateFileOperationType,
        path: String,
        diff: String,
    ) -> ApplyPatchUpdateFileOperation {
        ApplyPatchUpdateFileOperation { r#type, path, diff }
    }
}
/// Update an existing file with the provided diff.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum UpdateFileOperationType {
    #[serde(rename = "update_file")]
    #[default]
    UpdateFile,
}
