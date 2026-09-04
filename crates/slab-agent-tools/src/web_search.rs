//! Web search tool backed by the `websearch` crate.

use std::sync::Arc;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer};
use serde_json::Value;
use slab_agent::{
    AgentError, ToolCallRender, ToolContext, ToolHandler, ToolOutput, parse_tool_input,
    protocol::TurnItem, typed_input_schema,
};
use slab_config::secret_port::{EnvSecretAdapter, resolve_secret_or_plain};
use slab_config::{
    AgentWebSearchConfig, ProviderAuthConfig, WebSearchDuckDuckGoProviderConfig,
    WebSearchProviderId,
};
use websearch::{
    SearchOptions,
    providers::{
        ArxivProvider, BraveProvider, DuckDuckGoProvider, ExaProvider, GoogleProvider,
        SearxNGProvider, SerpApiProvider, TavilyProvider,
        duckduckgo::{DuckDuckGoConfig, SearchType as DuckDuckGoSearchType},
        google::GoogleConfig,
    },
    types::{SafeSearch, SearchProvider, SearchResult, SortBy, SortOrder},
};

// Schema-only mirrors of the enum-valued string arguments (plain comments on
// purpose — a doc comment would leak into the generated schema as a
// description). Runtime parsing keeps the wider lenient string handling with
// its exact error messages; only the schema enumerates the canonical values.

#[derive(JsonSchema)]
#[schemars(inline)]
#[serde(rename_all = "snake_case")]
enum WebSearchProviderSchema {
    Duckduckgo,
    Arxiv,
    Google,
    Tavily,
    Exa,
    Serpapi,
    Brave,
    Searxng,
}

#[derive(JsonSchema)]
#[schemars(inline)]
#[serde(rename_all = "snake_case")]
enum SafeSearchSchema {
    Off,
    Moderate,
    Strict,
}

#[derive(JsonSchema)]
#[schemars(inline)]
#[serde(rename_all = "snake_case")]
enum SortBySchema {
    Relevance,
    LastUpdatedDate,
    SubmittedDate,
}

#[derive(JsonSchema)]
#[schemars(inline)]
#[serde(rename_all = "snake_case")]
enum SortOrderSchema {
    Ascending,
    Descending,
}

/// Arguments for the `web_search` tool.
#[derive(Debug, Deserialize, JsonSchema)]
struct WebSearchArgs {
    /// Search query text.
    query: String,
    /// Search provider. An explicit choice is used strictly; the default falls back through the other configured providers on failure. Defaults to agent.tools.websearch.default_provider.
    #[schemars(with = "Option<WebSearchProviderSchema>")]
    provider: Option<String>,
    /// Maximum number of results to return.
    #[serde(default, deserialize_with = "deserialize_max_results")]
    #[schemars(range(min = 1))]
    max_results: Option<u64>,
    language: Option<String>,
    region: Option<String>,
    #[schemars(with = "Option<SafeSearchSchema>")]
    safe_search: Option<String>,
    /// Result page number (1-based).
    #[serde(default, deserialize_with = "deserialize_page")]
    #[schemars(range(min = 1))]
    page: Option<u64>,
    /// Comma-delimited ArXiv IDs to fetch.
    id_list: Option<String>,
    /// ArXiv result offset.
    #[serde(default, deserialize_with = "deserialize_start")]
    #[schemars(range(min = 0))]
    start: Option<u64>,
    #[schemars(with = "Option<SortBySchema>")]
    sort_by: Option<String>,
    #[schemars(with = "Option<SortOrderSchema>")]
    sort_order: Option<String>,
    #[serde(default, deserialize_with = "deserialize_timeout_ms")]
    #[schemars(range(min = 1))]
    timeout_ms: Option<u64>,
    /// Include provider raw payloads when available.
    #[serde(default)]
    include_raw: bool,
}

// Integer fields keep the exact "'<name>' must be an integer" wording for
// non-numeric values instead of serde's generic invalid-type message.
fn optional_integer<'de, D>(deserializer: D, name: &str) -> Result<Option<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match Option::<Value>::deserialize(deserializer)? {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value.as_u64().map(Some).ok_or_else(|| {
            serde::de::Error::custom(format!("'{name}' must be an integer"))
        }),
    }
}

fn deserialize_max_results<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    optional_integer(deserializer, "max_results")
}

fn deserialize_page<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    optional_integer(deserializer, "page")
}

fn deserialize_start<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    optional_integer(deserializer, "start")
}

fn deserialize_timeout_ms<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    optional_integer(deserializer, "timeout_ms")
}

pub struct WebSearchTool {
    config: AgentWebSearchConfig,
    runner: Arc<dyn WebSearchRunner>,
}

impl WebSearchTool {
    pub fn new(config: AgentWebSearchConfig) -> Self {
        Self { config, runner: Arc::new(DefaultWebSearchRunner) }
    }

    #[cfg(test)]
    fn with_runner(config: AgentWebSearchConfig, runner: Arc<dyn WebSearchRunner>) -> Self {
        Self { config, runner }
    }
}

impl Default for WebSearchTool {
    fn default() -> Self {
        Self::new(AgentWebSearchConfig::default())
    }
}

#[async_trait]
impl ToolHandler for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }

    /// Pure read — safe to run concurrently with other read-only calls.
    fn is_concurrency_safe(&self, _arguments: &serde_json::Value) -> bool {
        true
    }

    fn description(&self) -> &str {
        "Search the web through configured providers. Credentials are read from settings, \
         not tool arguments. On failure the search falls back through every other \
         credential-resolvable provider before reporting an aggregated error."
    }

    fn parameters_schema(&self) -> Value {
        typed_input_schema::<WebSearchArgs>()
    }

    fn describe_operation(&self, arguments: &Value) -> Option<slab_agent::OperationDescriptor> {
        let query = arguments.get("query").and_then(Value::as_str)?.to_string();
        Some(slab_agent::OperationDescriptor::network(query))
    }

    fn category(&self) -> slab_agent::OperationCategory {
        slab_agent::OperationCategory::Network
    }

    fn render_turn_item(&self, render: &ToolCallRender<'_>) -> TurnItem {
        TurnItem::WebSearch {
            id: render.call.id.clone(),
            query: render.args.get("query").and_then(Value::as_str).unwrap_or("").to_owned(),
        }
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        arguments: &Value,
    ) -> Result<ToolOutput, AgentError> {
        let args = parse_tool_input::<WebSearchArgs>(arguments)?;
        let include_raw = args.include_raw;
        let request = WebSearchRequest::from_args(args, self.config.default_provider)?;
        let results = self.runner.search(&self.config, request.clone()).await?;
        let results = results
            .into_iter()
            .map(|result| search_result_to_value(result, include_raw))
            .collect::<Vec<_>>();

        Ok(ToolOutput {
            content: serde_json::json!({
                "provider": request.provider.as_str(),
                "query": request.query,
                "results": results,
                "total": results.len(),
            })
            .to_string(),
            metadata: None,
        })
    }
}

#[derive(Debug, Clone)]
struct WebSearchRequest {
    provider: WebSearchProviderId,
    /// `true` when the caller EXPLICITLY named the provider — an explicit
    /// choice is used strictly (no fallback); only the defaulted request
    /// falls through the provider chain on failure.
    explicit_provider: bool,
    query: String,
    id_list: Option<String>,
    max_results: Option<u32>,
    language: Option<String>,
    region: Option<String>,
    safe_search: Option<SafeSearch>,
    page: Option<u32>,
    start: Option<u32>,
    sort_by: Option<SortBy>,
    sort_order: Option<SortOrder>,
    timeout_ms: Option<u64>,
}

impl WebSearchRequest {
    fn from_args(
        args: WebSearchArgs,
        default_provider: WebSearchProviderId,
    ) -> Result<Self, AgentError> {
        let (provider, explicit_provider) = match args.provider.as_deref() {
            Some(value) => {
                let parsed =
                    value.parse::<WebSearchProviderId>().map_err(AgentError::ToolExecution)?;
                (parsed, true)
            }
            None => (default_provider, false),
        };

        Ok(Self {
            provider,
            explicit_provider,
            query: args.query,
            id_list: args.id_list,
            max_results: narrow_positive_u32("max_results", args.max_results)?,
            language: args.language,
            region: args.region,
            safe_search: parse_safe_search(args.safe_search.as_deref())?,
            page: narrow_positive_u32("page", args.page)?,
            start: narrow_u32("start", args.start)?,
            sort_by: parse_sort_by(args.sort_by.as_deref())?,
            sort_order: parse_sort_order(args.sort_order.as_deref())?,
            timeout_ms: positive_u64("timeout_ms", args.timeout_ms)?,
        })
    }
}

/// Runs a parsed web search request. Tests replace this to avoid live network calls.
#[async_trait]
trait WebSearchRunner: Send + Sync {
    async fn search(
        &self,
        config: &AgentWebSearchConfig,
        request: WebSearchRequest,
    ) -> Result<Vec<SearchResult>, AgentError>;
}

struct DefaultWebSearchRunner;

#[async_trait]
impl WebSearchRunner for DefaultWebSearchRunner {
    async fn search(
        &self,
        config: &AgentWebSearchConfig,
        request: WebSearchRequest,
    ) -> Result<Vec<SearchResult>, AgentError> {
        // Fallback chain: the defaulted request walks the requested provider
        // first, then every OTHER credential-resolvable provider. Without
        // this, a DuckDuckGo captcha (the default provider's hard error) or a
        // missing api key left web_search with NO usable path; with it, a
        // keyless deployment still reaches arxiv and a keyed one tries each
        // configured provider in turn. An EXPLICIT provider choice is used
        // strictly — the model asked for that one and can re-request another
        // itself. The final error aggregates every attempt so the model sees
        // what was tried and why it failed.
        let chain = if request.explicit_provider {
            vec![request.provider]
        } else {
            provider_chain(config, request.provider)
        };
        let mut attempts: Vec<(WebSearchProviderId, String)> = Vec::new();
        for provider in chain {
            match build_provider(config, provider) {
                Ok(provider_impl) => {
                    let options = SearchOptions {
                        query: request.query.clone(),
                        id_list: request.id_list.clone(),
                        max_results: request.max_results,
                        language: request.language.clone(),
                        region: request.region.clone(),
                        safe_search: request.safe_search.clone(),
                        page: request.page,
                        start: request.start,
                        sort_by: request.sort_by.clone(),
                        sort_order: request.sort_order.clone(),
                        timeout: request.timeout_ms,
                        provider: provider_impl,
                        ..Default::default()
                    };
                    match websearch::web_search(options).await {
                        Ok(results) => return Ok(results),
                        Err(error) => attempts.push((provider, error.to_string())),
                    }
                }
                Err(error) => attempts.push((provider, error.to_string())),
            }
        }
        Err(AgentError::ToolExecution(format_web_search_failure(&attempts)))
    }
}

/// All known providers, in fallback preference order (credential-free ones
/// lead the tail so a keyless deployment still has a path after the head).
const PROVIDER_PREFERENCE: [WebSearchProviderId; 8] = [
    WebSearchProviderId::Duckduckgo,
    WebSearchProviderId::Arxiv,
    WebSearchProviderId::Brave,
    WebSearchProviderId::Tavily,
    WebSearchProviderId::Exa,
    WebSearchProviderId::Google,
    WebSearchProviderId::Serpapi,
    WebSearchProviderId::Searxng,
];

/// Whether `provider` has everything it needs under `config` (credentials +
/// required settings). Mirrors [`build_provider`]'s requirements without
/// constructing the provider.
fn provider_usable(config: &AgentWebSearchConfig, provider: WebSearchProviderId) -> bool {
    match provider {
        WebSearchProviderId::Duckduckgo | WebSearchProviderId::Arxiv => true,
        WebSearchProviderId::Google => {
            resolve_api_key("google", &config.providers.google.auth).is_ok()
                && trimmed(config.providers.google.cx.as_deref()).is_some()
        }
        WebSearchProviderId::Tavily => {
            resolve_api_key("tavily", &config.providers.tavily.auth).is_ok()
        }
        WebSearchProviderId::Exa => resolve_api_key("exa", &config.providers.exa.auth).is_ok(),
        WebSearchProviderId::Serpapi => {
            resolve_api_key("serpapi", &config.providers.serpapi.auth).is_ok()
        }
        WebSearchProviderId::Brave => {
            resolve_api_key("brave", &config.providers.brave.auth).is_ok()
        }
        WebSearchProviderId::Searxng => {
            trimmed(config.providers.searxng.base_url.as_deref()).is_some()
        }
    }
}

/// The fallback chain for a request: `requested` first, then the remaining
/// providers in [`PROVIDER_PREFERENCE`] order filtered to the ones whose
/// credentials actually resolve under `config`.
pub(crate) fn provider_chain(
    config: &AgentWebSearchConfig,
    requested: WebSearchProviderId,
) -> Vec<WebSearchProviderId> {
    let mut chain = vec![requested];
    for candidate in PROVIDER_PREFERENCE {
        if candidate == requested || !provider_usable(config, candidate) {
            continue;
        }
        chain.push(candidate);
    }
    chain
}

/// Aggregate error for a fully-failed chain: one provider(reason) pair per
/// attempt, each reason clipped so a stack of verbose provider errors stays
/// readable.
fn format_web_search_failure(attempts: &[(WebSearchProviderId, String)]) -> String {
    let rendered = attempts
        .iter()
        .map(|(provider, reason)| {
            let reason = reason.trim();
            let clipped: String = reason.chars().take(160).collect();
            if reason.chars().count() > 160 {
                format!("{}({clipped}…)", provider.as_str())
            } else {
                format!("{}({clipped})", provider.as_str())
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "web_search failed on all {} provider(s) tried: {rendered}; configure another provider's api key under agent.tools.websearch.providers.<name>.auth",
        attempts.len()
    )
}

fn build_provider(
    config: &AgentWebSearchConfig,
    provider: WebSearchProviderId,
) -> Result<Box<dyn SearchProvider>, AgentError> {
    match provider {
        WebSearchProviderId::Duckduckgo => {
            Ok(Box::new(duckduckgo_provider(&config.providers.duckduckgo)))
        }
        WebSearchProviderId::Arxiv => Ok(Box::new(ArxivProvider::new())),
        WebSearchProviderId::Google => {
            let provider_config = &config.providers.google;
            let api_key = resolve_api_key("google", &provider_config.auth)?;
            let cx = required_text(
                provider_config.cx.as_deref(),
                "agent.tools.websearch.providers.google.cx",
            )?;
            let mut google = GoogleConfig { api_key, cx: cx.to_owned(), ..Default::default() };
            if let Some(base_url) = trimmed(provider_config.base_url.as_deref()) {
                google.base_url = base_url.to_owned();
            }
            GoogleProvider::with_config(google)
                .map(|provider| Box::new(provider) as Box<dyn SearchProvider>)
                .map_err(tool_error)
        }
        WebSearchProviderId::Tavily => {
            let provider_config = &config.providers.tavily;
            let api_key = resolve_api_key("tavily", &provider_config.auth)?;
            let mut provider = if provider_config.include_raw_content == Some(true) {
                TavilyProvider::new_advanced(&api_key)
            } else {
                TavilyProvider::new(&api_key)
            }
            .map_err(tool_error)?;
            if let Some(depth) = trimmed(provider_config.search_depth.as_deref()) {
                provider = provider.with_search_depth(depth).map_err(tool_error)?;
            }
            if let Some(include_answer) = provider_config.include_answer {
                provider = provider.with_answer(include_answer);
            }
            if let Some(include_images) = provider_config.include_images {
                provider = provider.with_images(include_images);
            }
            if let Some(base_url) = trimmed(provider_config.base_url.as_deref()) {
                provider = provider.with_base_url(base_url);
            }
            Ok(Box::new(provider))
        }
        WebSearchProviderId::Exa => {
            let provider_config = &config.providers.exa;
            let api_key = resolve_api_key("exa", &provider_config.auth)?;
            let mut provider = ExaProvider::new(&api_key).map_err(tool_error)?;
            if let Some(include_contents) = provider_config.include_contents {
                provider = provider.with_contents(include_contents);
            }
            if let Some(model) = trimmed(provider_config.model.as_deref()) {
                provider = provider.with_model(model).map_err(tool_error)?;
            }
            if let Some(base_url) = trimmed(provider_config.base_url.as_deref()) {
                provider = provider.with_base_url(base_url);
            }
            Ok(Box::new(provider))
        }
        WebSearchProviderId::Serpapi => {
            let provider_config = &config.providers.serpapi;
            let api_key = resolve_api_key("serpapi", &provider_config.auth)?;
            let mut provider = SerpApiProvider::new(&api_key).map_err(tool_error)?;
            if let Some(engine) = trimmed(provider_config.engine.as_deref()) {
                provider = provider.with_engine(engine);
            }
            if let Some(base_url) = trimmed(provider_config.base_url.as_deref()) {
                provider = provider.with_base_url(base_url);
            }
            Ok(Box::new(provider))
        }
        WebSearchProviderId::Brave => {
            let provider_config = &config.providers.brave;
            let api_key = resolve_api_key("brave", &provider_config.auth)?;
            BraveProvider::new(&api_key)
                .map(|provider| Box::new(provider) as Box<dyn SearchProvider>)
                .map_err(tool_error)
        }
        WebSearchProviderId::Searxng => {
            let base_url = required_text(
                config.providers.searxng.base_url.as_deref(),
                "agent.tools.websearch.providers.searxng.base_url",
            )?;
            SearxNGProvider::new(base_url)
                .map(|provider| Box::new(provider) as Box<dyn SearchProvider>)
                .map_err(tool_error)
        }
    }
}

fn duckduckgo_provider(config: &WebSearchDuckDuckGoProviderConfig) -> DuckDuckGoProvider {
    let mut duck_config =
        DuckDuckGoConfig { search_type: DuckDuckGoSearchType::Text, ..Default::default() };
    if let Some(base_url) = trimmed(config.base_url.as_deref()) {
        duck_config.base_url = base_url.to_owned();
    }
    if let Some(user_agent) = trimmed(config.user_agent.as_deref()) {
        duck_config.user_agent = user_agent.to_owned();
    }
    // `use_lite` was never honored by the library (the lite endpoint has its
    // own fragile markup that is not implemented); it is deprecated and
    // ignored rather than half-wired to a second untested code path.
    if config.use_lite == Some(true) {
        tracing::warn!(
            "agent.tools.websearch.providers.duckduckgo.use_lite is deprecated and ignored"
        );
    }
    DuckDuckGoProvider::with_config(duck_config)
}

fn search_result_to_value(result: SearchResult, include_raw: bool) -> Value {
    let mut value = serde_json::json!({
        "title": result.title,
        "url": result.url,
        "snippet": result.snippet,
        "domain": result.domain,
        "published_date": result.published_date,
        "provider": result.provider,
    });
    if include_raw && let Some(raw) = result.raw {
        value["raw"] = raw;
    }
    value
}

fn resolve_api_key(provider: &str, auth: &ProviderAuthConfig) -> Result<String, AgentError> {
    if let Some(api_key) = trimmed(auth.api_key.as_deref()) {
        // Plaintext passes through unchanged; a `secret://env/<VAR>` handle
        // resolves in-process so config files need not store plaintext keys.
        return resolve_secret_or_plain(&EnvSecretAdapter::default(), api_key)
            .map_err(AgentError::ToolExecution);
    }

    if let Some(env_key) = trimmed(auth.api_key_env.as_deref()) {
        if let Ok(value) = std::env::var(env_key)
            && let Some(api_key) = trimmed(Some(value.as_str()))
        {
            return Ok(api_key.to_owned());
        }
        if !looks_like_env_var_name(env_key) {
            return Ok(env_key.to_owned());
        }
    }

    Err(AgentError::ToolExecution(format!(
        "web search provider '{provider}' is missing api key; set agent.tools.websearch.providers.{provider}.auth.api_key or api_key_env"
    )))
}

fn looks_like_env_var_name(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some(first) if first == '_' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn required_text<'a>(value: Option<&'a str>, path: &str) -> Result<&'a str, AgentError> {
    trimmed(value).ok_or_else(|| AgentError::ToolExecution(format!("missing setting '{path}'")))
}

fn trimmed(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

/// Narrow a non-negative integer to `u32` with the historical wording.
fn narrow_u32(name: &str, value: Option<u64>) -> Result<Option<u32>, AgentError> {
    value
        .map(|value| {
            u32::try_from(value)
                .map_err(|_| AgentError::ToolExecution(format!("'{name}' is too large")))
        })
        .transpose()
}

/// Narrow to `u32` and require at least 1.
fn narrow_positive_u32(name: &str, value: Option<u64>) -> Result<Option<u32>, AgentError> {
    narrow_u32(name, positive_u64(name, value)?)?
        .map(|value| {
            if value == 0 {
                Err(AgentError::ToolExecution(format!("'{name}' must be at least 1")))
            } else {
                Ok(value)
            }
        })
        .transpose()
}

/// Require at least 1 when present.
fn positive_u64(name: &str, value: Option<u64>) -> Result<Option<u64>, AgentError> {
    value
        .map(|value| {
            if value == 0 {
                Err(AgentError::ToolExecution(format!("'{name}' must be at least 1")))
            } else {
                Ok(value)
            }
        })
        .transpose()
}

fn parse_safe_search(value: Option<&str>) -> Result<Option<SafeSearch>, AgentError> {
    match value {
        Some("off") => Ok(Some(SafeSearch::Off)),
        Some("moderate") => Ok(Some(SafeSearch::Moderate)),
        Some("strict") => Ok(Some(SafeSearch::Strict)),
        Some(value) => Err(AgentError::ToolExecution(format!("unsupported safe_search '{value}'"))),
        None => Ok(None),
    }
}

fn parse_sort_by(value: Option<&str>) -> Result<Option<SortBy>, AgentError> {
    match value {
        Some("relevance") => Ok(Some(SortBy::Relevance)),
        Some("last_updated_date" | "lastUpdatedDate") => Ok(Some(SortBy::LastUpdatedDate)),
        Some("submitted_date" | "submittedDate") => Ok(Some(SortBy::SubmittedDate)),
        Some(value) => Err(AgentError::ToolExecution(format!("unsupported sort_by '{value}'"))),
        None => Ok(None),
    }
}

fn parse_sort_order(value: Option<&str>) -> Result<Option<SortOrder>, AgentError> {
    match value {
        Some("ascending") => Ok(Some(SortOrder::Ascending)),
        Some("descending") => Ok(Some(SortOrder::Descending)),
        Some(value) => Err(AgentError::ToolExecution(format!("unsupported sort_order '{value}'"))),
        None => Ok(None),
    }
}

fn tool_error(error: impl std::fmt::Display) -> AgentError {
    AgentError::ToolExecution(error.to_string())
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    struct FakeRunner {
        requests: Mutex<Vec<WebSearchRequest>>,
    }

    #[async_trait]
    impl WebSearchRunner for FakeRunner {
        async fn search(
            &self,
            _config: &AgentWebSearchConfig,
            request: WebSearchRequest,
        ) -> Result<Vec<SearchResult>, AgentError> {
            self.requests.lock().expect("requests").push(request);
            Ok(vec![SearchResult {
                url: "https://example.com".to_owned(),
                title: "Example".to_owned(),
                snippet: Some("Snippet".to_owned()),
                domain: Some("example.com".to_owned()),
                published_date: Some("2026-05-21".to_owned()),
                provider: Some("duckduckgo".to_owned()),
                raw: Some(serde_json::json!({"hidden": true})),
            }])
        }
    }

    fn ctx() -> ToolContext {
        ToolContext::for_thread("t1").build()
    }

    #[test]
    fn schema_includes_provider_enum() {
        let schema = WebSearchTool::default().parameters_schema();
        let providers = schema["properties"]["provider"]["enum"].as_array().expect("provider enum");

        assert!(providers.contains(&Value::String("duckduckgo".to_owned())));
        assert!(providers.contains(&Value::String("searxng".to_owned())));
        assert_eq!(schema["properties"]["max_results"]["minimum"], 1);
        assert_eq!(schema["properties"]["page"]["minimum"], 1);
        assert_eq!(schema["properties"]["timeout_ms"]["minimum"], 1);
        assert_eq!(schema["required"], serde_json::json!(["query"]));
    }

    #[tokio::test]
    async fn missing_provider_credentials_fail_before_network() {
        let mut config = AgentWebSearchConfig {
            default_provider: WebSearchProviderId::Google,
            ..AgentWebSearchConfig::default()
        };
        config.providers.google.cx = Some("cx".to_owned());
        let tool = WebSearchTool::new(config);
        // EXPLICIT provider: used strictly — the missing key must fail without
        // the fallback chain firing (and without touching the network).
        let error = tool
            .execute(&ctx(), &serde_json::json!({"query": "rust", "provider": "google"}))
            .await
            .expect_err("missing credentials should fail");

        assert!(error.to_string().contains("missing api key"));
    }

    #[tokio::test]
    async fn provider_settings_are_validated_before_network() {
        let mut google = AgentWebSearchConfig {
            default_provider: WebSearchProviderId::Google,
            ..AgentWebSearchConfig::default()
        };
        google.providers.google.auth.api_key = Some("key".to_owned());
        let error = WebSearchTool::new(google)
            .execute(&ctx(), &serde_json::json!({"query": "rust", "provider": "google"}))
            .await
            .expect_err("missing cx should fail");
        assert!(error.to_string().contains("agent.tools.websearch.providers.google.cx"));

        let searxng = AgentWebSearchConfig {
            default_provider: WebSearchProviderId::Searxng,
            ..AgentWebSearchConfig::default()
        };
        let error = WebSearchTool::new(searxng)
            .execute(&ctx(), &serde_json::json!({"query": "rust", "provider": "searxng"}))
            .await
            .expect_err("missing base url should fail");
        assert!(error.to_string().contains("agent.tools.websearch.providers.searxng.base_url"));
    }

    #[test]
    fn provider_chain_heads_with_request_and_skips_unconfigured() {
        // Keyless default: the head plus arxiv (the only other credential-free
        // provider) — a DuckDuckGo captcha still has a fallback path.
        let chain =
            provider_chain(&AgentWebSearchConfig::default(), WebSearchProviderId::Duckduckgo);
        assert_eq!(chain, vec![WebSearchProviderId::Duckduckgo, WebSearchProviderId::Arxiv]);

        // A keyed provider joins the chain right after the credential-free
        // ones, in preference order.
        let mut config = AgentWebSearchConfig::default();
        config.providers.brave.auth.api_key = Some("key".to_owned());
        let chain = provider_chain(&config, WebSearchProviderId::Duckduckgo);
        assert_eq!(
            chain,
            vec![
                WebSearchProviderId::Duckduckgo,
                WebSearchProviderId::Arxiv,
                WebSearchProviderId::Brave
            ]
        );

        // The head is never duplicated into the tail.
        let chain = provider_chain(&config, WebSearchProviderId::Brave);
        assert_eq!(
            chain,
            vec![
                WebSearchProviderId::Brave,
                WebSearchProviderId::Duckduckgo,
                WebSearchProviderId::Arxiv
            ]
        );
    }

    #[test]
    fn web_search_failure_aggregates_attempts_with_reasons() {
        let rendered = format_web_search_failure(&[
            (WebSearchProviderId::Duckduckgo, "captcha page blocked".to_owned()),
            (WebSearchProviderId::Arxiv, "network error".to_owned()),
        ]);
        assert!(rendered.contains("failed on all 2 provider(s)"), "{rendered}");
        assert!(rendered.contains("duckduckgo(captcha page blocked)"), "{rendered}");
        assert!(rendered.contains("arxiv(network error)"), "{rendered}");
        assert!(rendered.contains("agent.tools.websearch.providers"), "{rendered}");

        // Long reasons are clipped so a chain of verbose errors stays readable.
        let long = "x".repeat(500);
        let rendered = format_web_search_failure(&[(WebSearchProviderId::Brave, long)]);
        assert!(rendered.contains('…'), "{rendered}");
        assert!(rendered.chars().count() < 400, "{rendered}");
    }

    #[tokio::test]
    async fn fake_runner_shapes_output_without_raw_by_default() {
        let runner = Arc::new(FakeRunner { requests: Mutex::new(Vec::new()) });
        let tool = WebSearchTool::with_runner(AgentWebSearchConfig::default(), runner);
        let output = tool
            .execute(&ctx(), &serde_json::json!({"query": "rust", "max_results": 1}))
            .await
            .expect("search output");
        let value: Value = serde_json::from_str(&output.content).expect("json output");

        assert_eq!(value["provider"], "duckduckgo");
        assert_eq!(value["total"], 1);
        assert_eq!(value["results"][0]["title"], "Example");
        assert!(value["results"][0].get("raw").is_none());
    }

    #[tokio::test]
    async fn fake_runner_includes_raw_when_requested() {
        let runner = Arc::new(FakeRunner { requests: Mutex::new(Vec::new()) });
        let tool = WebSearchTool::with_runner(AgentWebSearchConfig::default(), runner);
        let output = tool
            .execute(&ctx(), &serde_json::json!({"query": "rust", "include_raw": true}))
            .await
            .expect("search output");
        let value: Value = serde_json::from_str(&output.content).expect("json output");

        assert_eq!(value["results"][0]["raw"]["hidden"], true);
    }

    #[tokio::test]
    async fn parsed_request_options_are_forwarded_to_runner() {
        let runner = Arc::new(FakeRunner { requests: Mutex::new(Vec::new()) });
        let tool = WebSearchTool::with_runner(AgentWebSearchConfig::default(), runner.clone());

        tool.execute(
            &ctx(),
            &serde_json::json!({
                "query": "rust",
                "provider": "arxiv",
                "id_list": "2401.00001,2401.00002",
                "max_results": 5,
                "language": "en",
                "region": "us",
                "safe_search": "strict",
                "page": 2,
                "start": 10,
                "sort_by": "last_updated_date",
                "sort_order": "ascending",
                "timeout_ms": 1500
            }),
        )
        .await
        .expect("search output");

        let requests = runner.requests.lock().expect("requests");
        let request = requests.first().expect("captured request");
        assert_eq!(request.provider, WebSearchProviderId::Arxiv);
        assert_eq!(request.query, "rust");
        assert_eq!(request.id_list.as_deref(), Some("2401.00001,2401.00002"));
        assert_eq!(request.max_results, Some(5));
        assert_eq!(request.language.as_deref(), Some("en"));
        assert_eq!(request.region.as_deref(), Some("us"));
        assert_eq!(
            request.safe_search.as_ref().map(ToString::to_string).as_deref(),
            Some("strict")
        );
        assert_eq!(request.page, Some(2));
        assert_eq!(request.start, Some(10));
        assert_eq!(
            request.sort_by.as_ref().map(ToString::to_string).as_deref(),
            Some("lastUpdatedDate")
        );
        assert_eq!(
            request.sort_order.as_ref().map(ToString::to_string).as_deref(),
            Some("ascending")
        );
        assert_eq!(request.timeout_ms, Some(1500));
    }

    #[tokio::test]
    async fn invalid_arguments_fail_before_runner_is_called() {
        let cases = [
            (serde_json::json!({}), "missing 'query' argument"),
            (
                serde_json::json!({"query": "rust", "provider": "missing"}),
                "unsupported web search provider",
            ),
            (
                serde_json::json!({"query": "rust", "max_results": 0}),
                "'max_results' must be at least 1",
            ),
            (serde_json::json!({"query": "rust", "page": 0}), "'page' must be at least 1"),
            (
                serde_json::json!({"query": "rust", "timeout_ms": 0}),
                "'timeout_ms' must be at least 1",
            ),
            (serde_json::json!({"query": "rust", "start": false}), "'start' must be an integer"),
            (
                serde_json::json!({"query": "rust", "safe_search": "maximum"}),
                "unsupported safe_search",
            ),
            (serde_json::json!({"query": "rust", "sort_by": "newest"}), "unsupported sort_by"),
            (
                serde_json::json!({"query": "rust", "sort_order": "sideways"}),
                "unsupported sort_order",
            ),
        ];

        for (arguments, expected) in cases {
            let runner = Arc::new(FakeRunner { requests: Mutex::new(Vec::new()) });
            let tool = WebSearchTool::with_runner(AgentWebSearchConfig::default(), runner.clone());
            let error = tool.execute(&ctx(), &arguments).await.expect_err("invalid arguments");

            assert!(error.to_string().contains(expected), "{error}");
            assert!(runner.requests.lock().expect("requests").is_empty());
        }
    }

    #[test]
    fn api_key_resolution_trims_literals_env_values_and_inline_fallbacks() {
        let literal =
            ProviderAuthConfig { api_key: Some(" literal ".to_owned()), api_key_env: None };
        assert_eq!(resolve_api_key("exa", &literal).expect("literal key"), "literal");

        unsafe {
            std::env::set_var("SLAB_WEB_SEARCH_TEST_KEY", " env-value ");
        }
        let env = ProviderAuthConfig {
            api_key: None,
            api_key_env: Some("SLAB_WEB_SEARCH_TEST_KEY".to_owned()),
        };
        assert_eq!(resolve_api_key("exa", &env).expect("env key"), "env-value");
        unsafe {
            std::env::remove_var("SLAB_WEB_SEARCH_TEST_KEY");
        }

        let inline = ProviderAuthConfig {
            api_key: None,
            api_key_env: Some("inline-secret-with-dashes".to_owned()),
        };
        assert_eq!(
            resolve_api_key("exa", &inline).expect("inline fallback"),
            "inline-secret-with-dashes"
        );
    }
}
