use crate::models;
use crate::openai::models::SearchContextSize;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct WebSearch {
    #[serde(rename = "user_location", skip_serializing_if = "Option::is_none")]
    pub user_location: Option<Box<models::WebSearchUserLocation>>,
    #[serde(rename = "search_context_size", skip_serializing_if = "Option::is_none")]
    pub search_context_size: Option<models::WebSearchContextSize>,
}

impl WebSearch {
    /// This tool searches the web for relevant results to use in a response. Learn more about the [web search tool](/docs/guides/tools-web-search?api-mode=chat).
    pub fn new() -> WebSearch {
        WebSearch { user_location: None, search_context_size: None }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct WebSearchActionFind {
    /// The action type.
    #[serde(rename = "type")]
    pub r#type: WebSearchActionFindType,
    /// The URL of the page searched for the pattern.
    #[serde(rename = "url")]
    pub url: String,
    /// The pattern or text to search for within the page.
    #[serde(rename = "pattern")]
    pub pattern: String,
}

impl WebSearchActionFind {
    /// Action type \"find_in_page\": Searches for a pattern within a loaded page.
    pub fn new(
        r#type: WebSearchActionFindType,
        url: String,
        pattern: String,
    ) -> WebSearchActionFind {
        WebSearchActionFind { r#type, url, pattern }
    }
}
/// The action type.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum WebSearchActionFindType {
    #[serde(rename = "find_in_page")]
    #[default]
    FindInPage,
}


#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct WebSearchActionOpenPage {
    /// The action type.
    #[serde(rename = "type")]
    pub r#type: WebSearchActionOpenPageType,
    /// The URL opened by the model.
    #[serde(
        rename = "url",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub url: Option<Option<String>>,
}

impl WebSearchActionOpenPage {
    /// Action type \"open_page\" - Opens a specific URL from search results.
    pub fn new(r#type: WebSearchActionOpenPageType) -> WebSearchActionOpenPage {
        WebSearchActionOpenPage { r#type, url: None }
    }
}
/// The action type.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum WebSearchActionOpenPageType {
    #[serde(rename = "open_page")]
    #[default]
    OpenPage,
}


#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct WebSearchActionSearch {
    /// The action type.
    #[serde(rename = "type")]
    pub r#type: WebSearchActionSearchType,
    /// [DEPRECATED] The search query.
    #[serde(rename = "query")]
    pub query: String,
    /// The search queries.
    #[serde(rename = "queries", skip_serializing_if = "Option::is_none")]
    pub queries: Option<Vec<String>>,
    /// The sources used in the search.
    #[serde(rename = "sources", skip_serializing_if = "Option::is_none")]
    pub sources: Option<Vec<models::WebSearchSource>>,
}

impl WebSearchActionSearch {
    /// Action type \"search\" - Performs a web search query.
    pub fn new(r#type: WebSearchActionSearchType, query: String) -> WebSearchActionSearch {
        WebSearchActionSearch { r#type, query, queries: None, sources: None }
    }
}
/// The action type.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum WebSearchActionSearchType {
    #[serde(rename = "search")]
    #[default]
    Search,
}


#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct WebSearchApproximateLocation {
    /// The type of location approximation. Always `approximate`.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub r#type: Option<WebSearchApproximateLocationType>,
    /// The two-letter [ISO country code](https://en.wikipedia.org/wiki/ISO_3166-1) of the user, e.g. `US`.
    #[serde(
        rename = "country",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub country: Option<Option<String>>,
    /// Free text input for the region of the user, e.g. `California`.
    #[serde(
        rename = "region",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub region: Option<Option<String>>,
    /// Free text input for the city of the user, e.g. `San Francisco`.
    #[serde(
        rename = "city",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub city: Option<Option<String>>,
    /// The [IANA timezone](https://timeapi.io/documentation/iana-timezones) of the user, e.g. `America/Los_Angeles`.
    #[serde(
        rename = "timezone",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub timezone: Option<Option<String>>,
}

impl WebSearchApproximateLocation {
    /// The approximate location of the user.
    pub fn new() -> WebSearchApproximateLocation {
        WebSearchApproximateLocation {
            r#type: None,
            country: None,
            region: None,
            city: None,
            timezone: None,
        }
    }
}
/// The type of location approximation. Always `approximate`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum WebSearchApproximateLocationType {
    #[serde(rename = "approximate")]
    #[default]
    Approximate,
}


#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum WebSearchContextSize {
    #[serde(rename = "low")]
    #[default]
    Low,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "high")]
    High,
}

impl std::fmt::Display for WebSearchContextSize {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
        }
    }
}


#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct WebSearchLocation {
    /// The two-letter  [ISO country code](https://en.wikipedia.org/wiki/ISO_3166-1) of the user, e.g. `US`.
    #[serde(rename = "country", skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    /// Free text input for the region of the user, e.g. `California`.
    #[serde(rename = "region", skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    /// Free text input for the city of the user, e.g. `San Francisco`.
    #[serde(rename = "city", skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    /// The [IANA timezone](https://timeapi.io/documentation/iana-timezones)  of the user, e.g. `America/Los_Angeles`.
    #[serde(rename = "timezone", skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
}

impl WebSearchLocation {
    /// Approximate location parameters for the search.
    pub fn new() -> WebSearchLocation {
        WebSearchLocation { country: None, region: None, city: None, timezone: None }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct WebSearchPreviewTool {
    /// The type of the web search tool. One of `web_search_preview` or `web_search_preview_2025_03_11`.
    #[serde(rename = "type")]
    pub r#type: WebSearchPreviewToolType,
    /// The user's location.
    #[serde(
        rename = "user_location",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub user_location: Option<Option<Box<models::ApproximateLocation>>>,
    /// High level guidance for the amount of context window space to use for the search. One of `low`, `medium`, or `high`. `medium` is the default.
    #[serde(rename = "search_context_size", skip_serializing_if = "Option::is_none")]
    pub search_context_size: Option<models::SearchContextSize>,
    #[serde(rename = "search_content_types", skip_serializing_if = "Option::is_none")]
    pub search_content_types: Option<Vec<models::SearchContentType>>,
}

impl WebSearchPreviewTool {
    /// This tool searches the web for relevant results to use in a response. Learn more about the [web search tool](https://platform.openai.com/docs/guides/tools-web-search).
    pub fn new(r#type: WebSearchPreviewToolType) -> WebSearchPreviewTool {
        WebSearchPreviewTool {
            r#type,
            user_location: None,
            search_context_size: None,
            search_content_types: None,
        }
    }
}
/// The type of the web search tool. One of `web_search_preview` or `web_search_preview_2025_03_11`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum WebSearchPreviewToolType {
    #[serde(rename = "web_search_preview")]
    #[default]
    WebSearchPreview,
    #[serde(rename = "web_search_preview_2025_03_11")]
    WebSearchPreview20250311,
}


#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct WebSearchSource {
    /// The type of source. Always `url`.
    #[serde(rename = "type")]
    pub r#type: WebSearchSourceType,
    /// The URL of the source.
    #[serde(rename = "url")]
    pub url: String,
}

impl WebSearchSource {
    /// A source used in the search.
    pub fn new(r#type: WebSearchSourceType, url: String) -> WebSearchSource {
        WebSearchSource { r#type, url }
    }
}
/// The type of source. Always `url`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum WebSearchSourceType {
    #[serde(rename = "url")]
    #[default]
    Url,
}


#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct WebSearchTool {
    /// The type of the web search tool. One of `web_search` or `web_search_2025_08_26`.
    #[serde(rename = "type")]
    pub r#type: WebSearchToolType,
    #[serde(
        rename = "filters",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub filters: Option<Option<Box<models::WebSearchToolFilters>>>,
    #[serde(
        rename = "user_location",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub user_location: Option<Option<Box<models::WebSearchApproximateLocation>>>,
    /// High level guidance for the amount of context window space to use for the search. One of `low`, `medium`, or `high`. `medium` is the default.
    #[serde(rename = "search_context_size", skip_serializing_if = "Option::is_none")]
    pub search_context_size: Option<SearchContextSize>,
}

impl WebSearchTool {
    /// Search the Internet for sources related to the prompt. Learn more about the [web search tool](/docs/guides/tools-web-search).
    pub fn new(r#type: WebSearchToolType) -> WebSearchTool {
        WebSearchTool { r#type, filters: None, user_location: None, search_context_size: None }
    }
}
/// The type of the web search tool. One of `web_search` or `web_search_2025_08_26`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum WebSearchToolType {
    #[serde(rename = "web_search")]
    #[default]
    WebSearch,
    #[serde(rename = "web_search_2025_08_26")]
    WebSearch20250826,
}


use super::misc::ToolStatus;
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct WebSearchToolCall {
    /// The unique ID of the web search tool call.
    #[serde(rename = "id")]
    pub id: String,
    /// The type of the web search tool call. Always `web_search_call`.
    #[serde(rename = "type")]
    pub r#type: WebSearchToolCallType,
    /// The status of the web search tool call.
    #[serde(rename = "status")]
    pub status: ToolStatus,
    #[serde(rename = "action")]
    pub action: Box<models::WebSearchToolCallAction>,
}

impl WebSearchToolCall {
    /// The results of a web search tool call. See the [web search guide](/docs/guides/tools-web-search) for more information.
    pub fn new(
        id: String,
        r#type: WebSearchToolCallType,
        status: ToolStatus,
        action: models::WebSearchToolCallAction,
    ) -> WebSearchToolCall {
        WebSearchToolCall { id, r#type, status, action: Box::new(action) }
    }
}
/// The type of the web search tool call. Always `web_search_call`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum WebSearchToolCallType {
    #[serde(rename = "web_search_call")]
    #[default]
    WebSearchCall,
}

// The status of the web search tool call.

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WebSearchToolCallAction {
    #[serde(rename = "WebSearchActionSearch")]
    WebSearchActionSearch(Box<models::WebSearchActionSearch>),
    #[serde(rename = "WebSearchActionOpenPage")]
    WebSearchActionOpenPage(Box<models::WebSearchActionOpenPage>),
    #[serde(rename = "WebSearchActionFind")]
    WebSearchActionFind(Box<models::WebSearchActionFind>),
}

impl Default for WebSearchToolCallAction {
    fn default() -> Self {
        Self::WebSearchActionSearch(Default::default())
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct WebSearchToolFilters {
    /// Allowed domains for the search. If not provided, all domains are allowed. Subdomains of the provided domains are allowed as well.  Example: `[\"pubmed.ncbi.nlm.nih.gov\"]`
    #[serde(
        rename = "allowed_domains",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub allowed_domains: Option<Option<Vec<String>>>,
}

impl WebSearchToolFilters {
    /// Filters for the search.
    pub fn new() -> WebSearchToolFilters {
        WebSearchToolFilters { allowed_domains: None }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum WebSearchToolSearchContextSize {
    #[serde(rename = "low")]
    #[default]
    Low,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "high")]
    High,
}


#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct WebSearchUserLocation {
    /// The type of location approximation. Always `approximate`.
    #[serde(rename = "type")]
    pub r#type: WebSearchUserLocationType,
    #[serde(rename = "approximate")]
    pub approximate: Box<models::WebSearchLocation>,
}

impl WebSearchUserLocation {
    /// Approximate location parameters for the search.
    pub fn new(
        r#type: WebSearchUserLocationType,
        approximate: models::WebSearchLocation,
    ) -> WebSearchUserLocation {
        WebSearchUserLocation { r#type, approximate: Box::new(approximate) }
    }
}
/// The type of location approximation. Always `approximate`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum WebSearchUserLocationType {
    #[serde(rename = "approximate")]
    #[default]
    Approximate,
}

