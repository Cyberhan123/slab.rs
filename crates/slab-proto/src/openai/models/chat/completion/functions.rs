use serde::{Deserialize, Serialize};
/// ChatCompletionFunctionCallOption : Specifying a particular function via `{\"name\": \"my_function\"}` forces the model to call that function.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatCompletionFunctionCallOption {
    /// The name of the function to call.
    #[serde(rename = "name")]
    pub name: String,
}

impl ChatCompletionFunctionCallOption {
    /// Specifying a particular function via `{\"name\": \"my_function\"}` forces the model to call that function.
    pub fn new(name: String) -> ChatCompletionFunctionCallOption {
        ChatCompletionFunctionCallOption { name }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatCompletionFunctions {
    /// The name of the function to be called. Must be a-z, A-Z, 0-9, or contain underscores and dashes, with a maximum length of 64.
    #[serde(rename = "name")]
    pub name: String,
    /// A description of what the function does, used by the model to choose when and how to call the function.
    #[serde(rename = "description", skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The parameters the functions accepts, described as a JSON Schema object. See the [guide](/docs/guides/function-calling) for examples, and the [JSON Schema reference](https://json-schema.org/understanding-json-schema/) for documentation about the format.   Omitting `parameters` defines a function with an empty parameter list.
    #[serde(rename = "parameters", skip_serializing_if = "Option::is_none")]
    pub parameters: Option<std::collections::HashMap<String, serde_json::Value>>,
}

impl ChatCompletionFunctions {
    pub fn new(name: String) -> ChatCompletionFunctions {
        ChatCompletionFunctions { name, description: None, parameters: None }
    }
}
