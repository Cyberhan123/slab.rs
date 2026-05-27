pub mod input_image_content {
    use crate::models;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
    pub struct InputImageContent {
        /// The type of the input item. Always `input_image`.
        #[serde(rename = "type")]
        pub r#type: Type,
        /// The detail level of the image to be sent to the model. One of `high`, `low`, `auto`, or `original`. Defaults to `auto`.
        #[serde(rename = "detail")]
        pub detail: models::ImageDetail,
        /// The URL of the image to be sent to the model. A fully qualified URL or base64 encoded image in a data URL.
        #[serde(
            rename = "image_url",
            default,
            with = "::serde_with::rust::double_option",
            skip_serializing_if = "Option::is_none"
        )]
        pub image_url: Option<Option<String>>,
        /// The ID of the file to be sent to the model.
        #[serde(
            rename = "file_id",
            default,
            with = "::serde_with::rust::double_option",
            skip_serializing_if = "Option::is_none"
        )]
        pub file_id: Option<Option<String>>,
    }

    impl InputImageContent {
        /// An image input to the model. Learn about [image inputs](/docs/guides/vision).
        pub fn new(r#type: Type, detail: models::ImageDetail) -> InputImageContent {
            InputImageContent { r#type, detail, image_url: None, file_id: None }
        }
    }
    /// The type of the input item. Always `input_image`.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    #[derive(Default)]
    pub(crate) enum Type {
        #[serde(rename = "input_image")]
        #[default]
        InputImage,
    }

    
}

pub mod input_image_content_param_auto_param {
    use crate::models;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
    pub struct InputImageContentParamAutoParam {
        /// The type of the input item. Always `input_image`.
        #[serde(rename = "type")]
        pub r#type: Type,
        /// The URL of the image to be sent to the model. A fully qualified URL or base64 encoded image in a data URL.
        #[serde(
            rename = "image_url",
            default,
            with = "::serde_with::rust::double_option",
            skip_serializing_if = "Option::is_none"
        )]
        pub image_url: Option<Option<String>>,
        /// The ID of the file to be sent to the model.
        #[serde(
            rename = "file_id",
            default,
            with = "::serde_with::rust::double_option",
            skip_serializing_if = "Option::is_none"
        )]
        pub file_id: Option<Option<String>>,
        /// The detail level of the image to be sent to the model. One of `high`, `low`, `auto`, or `original`. Defaults to `auto`.
        #[serde(
            rename = "detail",
            default,
            with = "::serde_with::rust::double_option",
            skip_serializing_if = "Option::is_none"
        )]
        pub detail: Option<Option<models::DetailEnum>>,
    }

    impl InputImageContentParamAutoParam {
        /// An image input to the model. Learn about [image inputs](/docs/guides/vision)
        pub fn new(r#type: Type) -> InputImageContentParamAutoParam {
            InputImageContentParamAutoParam { r#type, image_url: None, file_id: None, detail: None }
        }
    }
    /// The type of the input item. Always `input_image`.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    #[derive(Default)]
    pub(crate) enum Type {
        #[serde(rename = "input_image")]
        #[default]
        InputImage,
    }

    
}

pub mod image_ref_param {
    
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
    pub struct ImageRefParam {
        /// A fully qualified URL or base64-encoded data URL.
        #[serde(rename = "image_url", skip_serializing_if = "Option::is_none")]
        pub image_url: Option<String>,
        /// The File API ID of an uploaded image to use as input.
        #[serde(rename = "file_id", skip_serializing_if = "Option::is_none")]
        pub file_id: Option<String>,
    }

    impl ImageRefParam {
        /// Reference an input image by either URL or uploaded file ID. Provide exactly one of `image_url` or `file_id`.
        pub fn new() -> ImageRefParam {
            ImageRefParam { image_url: None, file_id: None }
        }
    }
}

pub mod image_ref_param2 {
    
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
    pub struct ImageRefParam2 {
        /// A fully qualified URL or base64-encoded data URL.
        #[serde(rename = "image_url", skip_serializing_if = "Option::is_none")]
        pub image_url: Option<String>,
        #[serde(rename = "file_id", skip_serializing_if = "Option::is_none")]
        pub file_id: Option<String>,
    }

    impl ImageRefParam2 {
        pub fn new() -> ImageRefParam2 {
            ImageRefParam2 { image_url: None, file_id: None }
        }
    }
}

pub use image_ref_param::ImageRefParam;
pub use image_ref_param2::ImageRefParam2;
pub use input_image_content::InputImageContent;
pub use input_image_content_param_auto_param::InputImageContentParamAutoParam;
