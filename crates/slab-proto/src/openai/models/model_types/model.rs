use crate::models;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct Model {
    /// The model identifier, which can be referenced in the API endpoints.
    #[serde(rename = "id")]
    pub id: String,
    /// The Unix timestamp (in seconds) when the model was created.
    #[serde(rename = "created")]
    pub created: i32,
    /// The object type, which is always \"model\".
    #[serde(rename = "object")]
    pub object: ModelObject,
    /// The organization that owns the model.
    #[serde(rename = "owned_by")]
    pub owned_by: String,
}

impl Model {
    /// Describes an OpenAI model offering that can be used with the API.
    pub fn new(id: String, created: i32, object: ModelObject, owned_by: String) -> Model {
        Model { id, created, object, owned_by }
    }
}
pub mod model_object {
    use serde::{Deserialize, Serialize};
    /// The object type, which is always \"model\".
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    #[derive(Default)]
    pub enum Object {
        #[serde(rename = "model")]
        #[default]
        Model,
    }

    
}
pub use model_object::Object as ModelObject;

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelIds {}

impl ModelIds {
    pub fn new() -> ModelIds {
        ModelIds {}
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelIdsCompaction {}

impl ModelIdsCompaction {
    /// Model ID used to generate the response, like `gpt-5` or `o3`. OpenAI offers a wide range of models with different capabilities, performance characteristics, and price points. Refer to the [model guide](/docs/models) to browse and compare available models.
    pub fn new() -> ModelIdsCompaction {
        ModelIdsCompaction {}
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelIdsResponses {}

impl ModelIdsResponses {
    pub fn new() -> ModelIdsResponses {
        ModelIdsResponses {}
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelIdsShared {}

impl ModelIdsShared {
    pub fn new() -> ModelIdsShared {
        ModelIdsShared {}
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ListModelsResponse {
    #[serde(rename = "object")]
    pub object: ListModelsResponseObject,
    #[serde(rename = "data")]
    pub data: Vec<models::Model>,
}

impl ListModelsResponse {
    pub fn new(object: ListModelsResponseObject, data: Vec<models::Model>) -> ListModelsResponse {
        ListModelsResponse { object, data }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum ListModelsResponseObject {
    #[serde(rename = "list")]
    #[default]
    List,
}
