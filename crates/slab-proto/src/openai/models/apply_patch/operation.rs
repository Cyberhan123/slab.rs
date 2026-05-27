use crate::models;
use serde::{Deserialize, Serialize};

/// ApplyPatchOperationParam : One of the create_file, delete_file, or update_file operations supplied to the apply_patch tool.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ApplyPatchOperationParam {
    #[serde(rename = "ApplyPatchCreateFileOperationParam")]
    ApplyPatchCreateFileOperationParam(Box<models::ApplyPatchCreateFileOperationParam>),
    #[serde(rename = "ApplyPatchDeleteFileOperationParam")]
    ApplyPatchDeleteFileOperationParam(Box<models::ApplyPatchDeleteFileOperationParam>),
    #[serde(rename = "ApplyPatchUpdateFileOperationParam")]
    ApplyPatchUpdateFileOperationParam(Box<models::ApplyPatchUpdateFileOperationParam>),
}

impl Default for ApplyPatchOperationParam {
    fn default() -> Self {
        Self::ApplyPatchCreateFileOperationParam(Default::default())
    }
}

/// ApplyPatchOperation : One of the create_file, delete_file, or update_file operations applied via apply_patch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ApplyPatchOperation {
    #[serde(rename = "ApplyPatchCreateFileOperation")]
    ApplyPatchCreateFileOperation(Box<models::ApplyPatchCreateFileOperation>),
    #[serde(rename = "ApplyPatchDeleteFileOperation")]
    ApplyPatchDeleteFileOperation(Box<models::ApplyPatchDeleteFileOperation>),
    #[serde(rename = "ApplyPatchUpdateFileOperation")]
    ApplyPatchUpdateFileOperation(Box<models::ApplyPatchUpdateFileOperation>),
}

impl Default for ApplyPatchOperation {
    fn default() -> Self {
        Self::ApplyPatchCreateFileOperation(Default::default())
    }
}
