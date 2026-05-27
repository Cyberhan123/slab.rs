use crate::models;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum SearchContentType {
    #[serde(rename = "text")]
    Text,
    #[serde(rename = "image")]
    Image,
}

impl std::fmt::Display for SearchContentType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Text => write!(f, "text"),
            Self::Image => write!(f, "image"),
        }
    }
}

impl Default for SearchContentType {
    fn default() -> SearchContentType {
        Self::Text
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum SearchContextSize {
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "high")]
    High,
}

impl std::fmt::Display for SearchContextSize {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
        }
    }
}

impl Default for SearchContextSize {
    fn default() -> SearchContextSize {
        Self::Low
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct Filters {
    /// Specifies the comparison operator: `eq`, `ne`, `gt`, `gte`, `lt`, `lte`, `in`, `nin`. - `eq`: equals - `ne`: not equal - `gt`: greater than - `gte`: greater than or equal - `lt`: less than - `lte`: less than or equal - `in`: in - `nin`: not in
    #[serde(rename = "type")]
    pub r#type: Type,
    /// The key to compare against the value.
    #[serde(rename = "key")]
    pub key: String,
    #[serde(rename = "value")]
    pub value: Box<models::ComparisonFilterValue>,
    /// Array of filters to combine. Items can be `ComparisonFilter` or `CompoundFilter`.
    #[serde(rename = "filters")]
    pub filters: Vec<models::ComparisonFilter>,
}

impl Filters {
    pub fn new(
        r#type: Type,
        key: String,
        value: models::ComparisonFilterValue,
        filters: Vec<models::ComparisonFilter>,
    ) -> Filters {
        Filters { r#type, key, value: Box::new(value), filters }
    }
}
/// Specifies the comparison operator: `eq`, `ne`, `gt`, `gte`, `lt`, `lte`, `in`, `nin`. - `eq`: equals - `ne`: not equal - `gt`: greater than - `gte`: greater than or equal - `lt`: less than - `lte`: less than or equal - `in`: in - `nin`: not in
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub(crate) enum Type {
    #[serde(rename = "eq")]
    Eq,
    #[serde(rename = "ne")]
    Ne,
    #[serde(rename = "gt")]
    Gt,
    #[serde(rename = "gte")]
    Gte,
    #[serde(rename = "lt")]
    Lt,
    #[serde(rename = "lte")]
    Lte,
    #[serde(rename = "in")]
    In,
    #[serde(rename = "nin")]
    Nin,
    #[serde(rename = "and")]
    And,
    #[serde(rename = "or")]
    Or,
}

impl Default for Type {
    fn default() -> Type {
        Self::Eq
    }
}
