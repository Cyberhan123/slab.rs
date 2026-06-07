use slab_app_core::domain::services::PluginService;
use slab_jsonrpc::{
    APPLICATION_ERROR, INVALID_REQUEST, METHOD_NOT_FOUND, PARSE_ERROR, error_response,
    parse_message, serialize_response, success_response,
};

pub(super) async fn handle_payload(service: &PluginService, payload: &str) -> String {
    let request = match parse_message(payload) {
        Ok(request) => request,
        Err(error) => {
            return serialize_response(&error_response(
                serde_json::Value::Null,
                PARSE_ERROR,
                format!("invalid json-rpc payload: {error}"),
            ));
        }
    };

    let Some(id) = request.id.clone() else {
        return serialize_response(&error_response(
            serde_json::Value::Null,
            INVALID_REQUEST,
            "request missing id",
        ));
    };

    if !request.has_valid_version() {
        return serialize_response(&error_response(id, INVALID_REQUEST, "jsonrpc must be `2.0`"));
    }

    let Some(method) = request.method.as_deref() else {
        return serialize_response(&error_response(id, INVALID_REQUEST, "request missing method"));
    };

    let Some((plugin_id, function_name)) = parse_method(method) else {
        return serialize_response(&error_response(
            id,
            METHOD_NOT_FOUND,
            "method must use `plugin_id.function_name`",
        ));
    };

    match service.dispatch_rpc(plugin_id, function_name, request.params).await {
        Ok(result) => serialize_response(&success_response(id, result)),
        Err(error) => serialize_response(&error_response(id, APPLICATION_ERROR, error.to_string())),
    }
}

fn parse_method(method: &str) -> Option<(&str, &str)> {
    let (plugin_id, function_name) = method.split_once('.')?;
    if plugin_id.trim().is_empty() || function_name.trim().is_empty() {
        return None;
    }
    Some((plugin_id, function_name))
}

#[cfg(test)]
mod tests {
    use super::parse_method;

    #[test]
    fn parses_plugin_rpc_method_shape() {
        assert_eq!(parse_method("plugin-a.run"), Some(("plugin-a", "run")));
        assert_eq!(parse_method("plugin-a."), None);
        assert_eq!(parse_method(".run"), None);
        assert_eq!(parse_method("plugin-a"), None);
    }
}
