//! Web search tool backed by the `websearch` crate.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use slab_agent::{AgentError, ToolContext, ToolHandler, ToolOutput};
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

    fn description(&self) -> &str {
        "Search the web through configured providers. Credentials are read from settings, not tool arguments."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query text."
                },
                "provider": {
                    "type": "string",
                    "enum": [
                        "duckduckgo",
                        "arxiv",
                        "google",
                        "tavily",
                        "exa",
                        "serpapi",
                        "brave",
                        "searxng"
                    ],
                    "description": "Search provider. Defaults to agent.tools.websearch.default_provider."
                },
                "max_results": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Maximum number of results to return."
                },
                "language": { "type": "string" },
                "region": { "type": "string" },
                "safe_search": {
                    "type": "string",
                    "enum": ["off", "moderate", "strict"]
                },
                "page": {
                    "type": "integer",
                    "minimum": 1
                },
                "id_list": {
                    "type": "string",
                    "description": "Comma-delimited ArXiv IDs to fetch."
                },
                "start": {
                    "type": "integer",
                    "minimum": 0,
                    "description": "ArXiv result offset."
                },
                "sort_by": {
                    "type": "string",
                    "enum": ["relevance", "last_updated_date", "submitted_date"]
                },
                "sort_order": {
                    "type": "string",
                    "enum": ["ascending", "descending"]
                },
                "timeout_ms": {
                    "type": "integer",
                    "minimum": 1
                },
                "include_raw": {
                    "type": "boolean",
                    "default": false,
                    "description": "Include provider raw payloads when available."
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        arguments: &Value,
    ) -> Result<ToolOutput, AgentError> {
        let request = WebSearchRequest::from_arguments(arguments, self.config.default_provider)?;
        let include_raw = arguments.get("include_raw").and_then(Value::as_bool).unwrap_or(false);
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
    fn from_arguments(
        arguments: &Value,
        default_provider: WebSearchProviderId,
    ) -> Result<Self, AgentError> {
        let query = arguments
            .get("query")
            .and_then(Value::as_str)
            .ok_or_else(|| AgentError::ToolExecution("missing 'query' argument".into()))?
            .to_owned();
        let provider = match arguments.get("provider").and_then(Value::as_str) {
            Some(value) => {
                value.parse::<WebSearchProviderId>().map_err(AgentError::ToolExecution)?
            }
            None => default_provider,
        };

        Ok(Self {
            provider,
            query,
            id_list: optional_string(arguments, "id_list"),
            max_results: optional_positive_u32(arguments, "max_results")?,
            language: optional_string(arguments, "language"),
            region: optional_string(arguments, "region"),
            safe_search: optional_safe_search(arguments)?,
            page: optional_positive_u32(arguments, "page")?,
            start: optional_u32(arguments, "start")?,
            sort_by: optional_sort_by(arguments)?,
            sort_order: optional_sort_order(arguments)?,
            timeout_ms: optional_positive_u64(arguments, "timeout_ms")?,
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
        let provider = build_provider(config, request.provider)?;
        let options = SearchOptions {
            query: request.query,
            id_list: request.id_list,
            max_results: request.max_results,
            language: request.language,
            region: request.region,
            safe_search: request.safe_search,
            page: request.page,
            start: request.start,
            sort_by: request.sort_by,
            sort_order: request.sort_order,
            timeout: request.timeout_ms,
            provider,
            ..Default::default()
        };

        websearch::web_search(options)
            .await
            .map_err(|error| AgentError::ToolExecution(error.to_string()))
    }
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
    if let Some(use_lite) = config.use_lite {
        duck_config.use_lite = use_lite;
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

fn optional_string(arguments: &Value, name: &str) -> Option<String> {
    arguments.get(name).and_then(Value::as_str).map(str::to_owned)
}

fn optional_u32(arguments: &Value, name: &str) -> Result<Option<u32>, AgentError> {
    optional_u64(arguments, name)?
        .map(|value| {
            u32::try_from(value)
                .map_err(|_| AgentError::ToolExecution(format!("'{name}' is too large")))
        })
        .transpose()
}

fn optional_positive_u32(arguments: &Value, name: &str) -> Result<Option<u32>, AgentError> {
    optional_positive_u64(arguments, name)?
        .map(|value| {
            u32::try_from(value)
                .map_err(|_| AgentError::ToolExecution(format!("'{name}' is too large")))
        })
        .transpose()
}

fn optional_u64(arguments: &Value, name: &str) -> Result<Option<u64>, AgentError> {
    arguments
        .get(name)
        .map(|value| {
            value
                .as_u64()
                .ok_or_else(|| AgentError::ToolExecution(format!("'{name}' must be an integer")))
        })
        .transpose()
}

fn optional_positive_u64(arguments: &Value, name: &str) -> Result<Option<u64>, AgentError> {
    optional_u64(arguments, name)?
        .map(|value| {
            if value == 0 {
                Err(AgentError::ToolExecution(format!("'{name}' must be at least 1")))
            } else {
                Ok(value)
            }
        })
        .transpose()
}

fn optional_safe_search(arguments: &Value) -> Result<Option<SafeSearch>, AgentError> {
    match arguments.get("safe_search").and_then(Value::as_str) {
        Some("off") => Ok(Some(SafeSearch::Off)),
        Some("moderate") => Ok(Some(SafeSearch::Moderate)),
        Some("strict") => Ok(Some(SafeSearch::Strict)),
        Some(value) => Err(AgentError::ToolExecution(format!("unsupported safe_search '{value}'"))),
        None => Ok(None),
    }
}

fn optional_sort_by(arguments: &Value) -> Result<Option<SortBy>, AgentError> {
    match arguments.get("sort_by").and_then(Value::as_str) {
        Some("relevance") => Ok(Some(SortBy::Relevance)),
        Some("last_updated_date" | "lastUpdatedDate") => Ok(Some(SortBy::LastUpdatedDate)),
        Some("submitted_date" | "submittedDate") => Ok(Some(SortBy::SubmittedDate)),
        Some(value) => Err(AgentError::ToolExecution(format!("unsupported sort_by '{value}'"))),
        None => Ok(None),
    }
}

fn optional_sort_order(arguments: &Value) -> Result<Option<SortOrder>, AgentError> {
    match arguments.get("sort_order").and_then(Value::as_str) {
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
        let error = tool
            .execute(&ctx(), &serde_json::json!({"query": "rust"}))
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
            .execute(&ctx(), &serde_json::json!({"query": "rust"}))
            .await
            .expect_err("missing cx should fail");
        assert!(error.to_string().contains("agent.tools.websearch.providers.google.cx"));

        let searxng = AgentWebSearchConfig {
            default_provider: WebSearchProviderId::Searxng,
            ..AgentWebSearchConfig::default()
        };
        let error = WebSearchTool::new(searxng)
            .execute(&ctx(), &serde_json::json!({"query": "rust"}))
            .await
            .expect_err("missing base url should fail");
        assert!(error.to_string().contains("agent.tools.websearch.providers.searxng.base_url"));
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
