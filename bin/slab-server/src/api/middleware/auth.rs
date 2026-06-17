use axum::{
    extract::{Request, State},
    http::{StatusCode, header::AUTHORIZATION},
    middleware::Next,
    response::{IntoResponse, Response},
};
use slab_app_core::context::AppState;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Response {
    let provided = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    let is_authorized = is_authorized(
        state.context.config.admin_api_token.as_deref(),
        &state.context.config.bind_address,
        provided,
    );

    if is_authorized { next.run(req).await } else { StatusCode::UNAUTHORIZED.into_response() }
}

fn is_authorized(
    configured_token: Option<&str>,
    bind_address: &str,
    provided_token: Option<&str>,
) -> bool {
    match configured_token {
        Some(raw_expected) => {
            let expected = raw_expected.trim();
            !expected.is_empty()
                && provided_token
                    .map(|provided| constant_time_token_eq(provided, expected))
                    .unwrap_or(false)
        }
        None => is_loopback_bind_address(bind_address),
    }
}

fn constant_time_token_eq(provided: &str, expected: &str) -> bool {
    if provided.is_empty() || provided.len() != expected.len() {
        return false;
    }

    provided
        .as_bytes()
        .iter()
        .zip(expected.as_bytes())
        .fold(0u8, |diff, (provided, expected)| diff | (provided ^ expected))
        == 0
}

fn is_loopback_bind_address(bind_address: &str) -> bool {
    if let Ok(address) = bind_address.parse::<SocketAddr>() {
        return address.ip().is_loopback();
    }

    let Some((host, _port)) = bind_address.rsplit_once(':') else {
        return false;
    };
    let host = host.trim_matches(['[', ']']);

    host.eq_ignore_ascii_case("localhost")
        || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

#[cfg(test)]
mod tests {
    use super::is_authorized;

    #[test]
    fn unauthenticated_management_access_stays_local_only_without_configured_token() {
        assert!(is_authorized(None, "127.0.0.1:3000", None));
        assert!(is_authorized(None, "localhost:3000", None));
        assert!(is_authorized(None, "[::1]:3000", None));

        assert!(!is_authorized(None, "0.0.0.0:3000", None));
        assert!(!is_authorized(None, "[::]:3000", None));
        assert!(!is_authorized(None, "192.168.0.42:3000", None));
    }

    #[test]
    fn configured_admin_token_must_be_non_empty_and_match() {
        assert!(is_authorized(
            Some("vitest-admin-token"),
            "0.0.0.0:3000",
            Some("vitest-admin-token")
        ));
        assert!(is_authorized(
            Some(" vitest-admin-token "),
            "0.0.0.0:3000",
            Some("vitest-admin-token")
        ));

        assert!(!is_authorized(Some("vitest-admin-token"), "127.0.0.1:3000", None));
        assert!(!is_authorized(Some("vitest-admin-token"), "127.0.0.1:3000", Some("wrong")));
        assert!(!is_authorized(
            Some("vitest-admin-token"),
            "127.0.0.1:3000",
            Some("vitest-admin-token ")
        ));
        assert!(!is_authorized(Some(""), "127.0.0.1:3000", Some("anything")));
        assert!(!is_authorized(Some("   "), "127.0.0.1:3000", Some("anything")));
    }
}
