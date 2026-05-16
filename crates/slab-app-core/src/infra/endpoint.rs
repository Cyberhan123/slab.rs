use anyhow::{Context, Result, anyhow, bail};
use reqwest::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TransportEndpoint {
    Http(String),
    Ipc(String),
}

impl TransportEndpoint {
    pub(crate) fn as_display(&self) -> &str {
        match self {
            Self::Http(url) | Self::Ipc(url) => url.as_str(),
        }
    }
}

pub(crate) fn ensure_http_base_url(raw: &str) -> Result<String> {
    let mut url = parse_absolute_http_url(raw)?;
    let next_path = match url.path().trim_end_matches('/') {
        "" => "/".to_owned(),
        path => format!("{path}/"),
    };
    url.set_path(&next_path);
    Ok(url.to_string())
}

pub(crate) fn join_http_url_path(raw: &str, suffix: &str) -> Result<String> {
    let mut url = parse_absolute_http_url(raw)?;
    let suffix = suffix.trim_matches('/');
    let base_path = url.path().trim_end_matches('/');
    let next_path = match (base_path.is_empty(), suffix.is_empty()) {
        (_, true) => "/".to_owned(),
        (true, false) => format!("/{suffix}"),
        (false, false) => format!("{base_path}/{suffix}"),
    };
    url.set_path(&next_path);
    Ok(url.to_string())
}

pub(crate) fn parse_transport_endpoint(raw: &str) -> Result<TransportEndpoint> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        bail!("gRPC endpoint is empty");
    }

    if let Some(path) = trimmed.strip_prefix("ipc://") {
        let path = path.trim();
        if path.is_empty() {
            bail!("invalid IPC endpoint '{trimmed}': missing socket/pipe path");
        }
        return Ok(TransportEndpoint::Ipc(path.to_owned()));
    }

    Ok(TransportEndpoint::Http(parse_http_connect_url(trimmed)?.to_string()))
}

pub(crate) fn http_probe_authority(raw: &str) -> Result<String> {
    let trimmed = raw.trim();
    let url = parse_http_connect_url(trimmed)?;
    let host = url
        .host_str()
        .ok_or_else(|| anyhow!("HTTP endpoint '{trimmed}' is missing a host"))?;
    let port = url.port_or_known_default();

    Ok(match port {
        Some(port) if host.contains(':') => format!("[{host}]:{port}"),
        Some(port) => format!("{host}:{port}"),
        None => host.to_owned(),
    })
}

pub(crate) fn normalize_ipc_endpoint_path(raw: &str) -> Result<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        bail!("IPC endpoint is empty");
    }

    let path = trimmed.strip_prefix("ipc://").unwrap_or(trimmed).trim();
    if path.is_empty() {
        bail!("invalid IPC endpoint '{trimmed}': missing socket/pipe path");
    }

    Ok(path.to_owned())
}

fn parse_absolute_http_url(raw: &str) -> Result<Url> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        bail!("URL is empty");
    }

    let url = Url::parse(trimmed).with_context(|| format!("invalid URL '{trimmed}'"))?;
    ensure_http_scheme(url, trimmed)
}

fn parse_http_connect_url(raw: &str) -> Result<Url> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        bail!("HTTP endpoint is empty");
    }

    let candidate = if trimmed.contains("://") {
        trimmed.to_owned()
    } else {
        format!("http://{trimmed}")
    };
    let url = Url::parse(&candidate)
        .with_context(|| format!("invalid HTTP endpoint '{trimmed}'"))?;
    ensure_http_scheme(url, trimmed)
}

fn ensure_http_scheme(url: Url, raw: &str) -> Result<Url> {
    match url.scheme() {
        "http" | "https" => Ok(url),
        other => bail!("unsupported URL scheme '{other}' in '{raw}'"),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        TransportEndpoint, ensure_http_base_url, http_probe_authority, join_http_url_path,
        normalize_ipc_endpoint_path, parse_transport_endpoint,
    };

    #[test]
    fn ensure_http_base_url_keeps_query_and_trailing_slash() {
        assert_eq!(
            ensure_http_base_url("https://api.openai.com/v1?api-version=2026-01-01").unwrap(),
            "https://api.openai.com/v1/?api-version=2026-01-01"
        );
    }

    #[test]
    fn join_http_url_path_keeps_query_and_base_path() {
        assert_eq!(
            join_http_url_path(
                "https://api.openai.com/v1?api-version=2026-01-01",
                "chat/completions"
            )
            .unwrap(),
            "https://api.openai.com/v1/chat/completions?api-version=2026-01-01"
        );
    }

    #[test]
    fn parse_transport_endpoint_defaults_bare_authority_to_http() {
        assert_eq!(
            parse_transport_endpoint("127.0.0.1:50052").unwrap(),
            TransportEndpoint::Http("http://127.0.0.1:50052/".to_owned())
        );
    }

    #[test]
    fn http_probe_authority_extracts_host_and_port() {
        assert_eq!(
            http_probe_authority("http://127.0.0.1:50052/v1").unwrap(),
            "127.0.0.1:50052"
        );
    }

    #[test]
    fn normalize_ipc_endpoint_path_accepts_prefixed_and_raw_paths() {
        let path = r"\\.\pipe\slab-runtime-llama";
        assert_eq!(normalize_ipc_endpoint_path(&format!("ipc://{path}")).unwrap(), path);
        assert_eq!(normalize_ipc_endpoint_path(path).unwrap(), path);
    }
}