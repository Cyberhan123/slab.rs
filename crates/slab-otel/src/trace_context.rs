use std::collections::BTreeMap;

use anyhow::{Context, bail};
use uuid::Uuid;

pub const TRACEPARENT_HEADER: &str = "traceparent";
pub const TRACESTATE_HEADER: &str = "tracestate";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceContext {
    pub trace_id: String,
    pub span_id: String,
    pub trace_flags: u8,
    pub tracestate: BTreeMap<String, String>,
}

impl TraceContext {
    pub fn new_sampled() -> Self {
        Self {
            trace_id: new_trace_id(),
            span_id: new_span_id(),
            trace_flags: 1,
            tracestate: BTreeMap::new(),
        }
    }

    pub fn traceparent(&self) -> String {
        format!("00-{}-{}-{:02x}", self.trace_id, self.span_id, self.trace_flags)
    }

    pub fn tracestate_header(&self) -> Option<String> {
        (!self.tracestate.is_empty()).then(|| {
            self.tracestate
                .iter()
                .map(|(key, value)| format!("{key}={value}"))
                .collect::<Vec<_>>()
                .join(",")
        })
    }
}

pub fn parse_traceparent(value: &str) -> anyhow::Result<TraceContext> {
    let parts = value.trim().split('-').collect::<Vec<_>>();
    if parts.len() != 4 {
        bail!("traceparent must have 4 dash-separated parts");
    }
    if parts[0] != "00" {
        bail!("unsupported traceparent version '{}'", parts[0]);
    }
    let trace_id = parts[1];
    let span_id = parts[2];
    let flags = parts[3];
    validate_hex(trace_id, 32, "trace id")?;
    validate_hex(span_id, 16, "span id")?;
    validate_hex(flags, 2, "trace flags")?;
    if trace_id.chars().all(|ch| ch == '0') {
        bail!("trace id must not be all zeroes");
    }
    if span_id.chars().all(|ch| ch == '0') {
        bail!("span id must not be all zeroes");
    }
    Ok(TraceContext {
        trace_id: trace_id.to_owned(),
        span_id: span_id.to_owned(),
        trace_flags: u8::from_str_radix(flags, 16).context("invalid trace flags")?,
        tracestate: BTreeMap::new(),
    })
}

pub fn parse_tracestate(value: &str) -> BTreeMap<String, String> {
    value
        .split(',')
        .filter_map(|entry| {
            let (key, value) = entry.trim().split_once('=')?;
            let key = key.trim();
            let value = value.trim();
            (!key.is_empty() && !value.is_empty()).then(|| (key.to_owned(), value.to_owned()))
        })
        .collect()
}

pub fn inject_w3c_headers(headers: &mut BTreeMap<String, String>, context: &TraceContext) {
    headers.insert(TRACEPARENT_HEADER.to_owned(), context.traceparent());
    if let Some(tracestate) = context.tracestate_header() {
        headers.insert(TRACESTATE_HEADER.to_owned(), tracestate);
    }
}

pub fn extract_w3c_headers(
    headers: &BTreeMap<String, String>,
) -> anyhow::Result<Option<TraceContext>> {
    let Some(traceparent) = get_header(headers, TRACEPARENT_HEADER) else {
        return Ok(None);
    };
    let mut context = parse_traceparent(traceparent)?;
    if let Some(tracestate) = get_header(headers, TRACESTATE_HEADER) {
        context.tracestate = parse_tracestate(tracestate);
    }
    Ok(Some(context))
}

fn get_header<'a>(headers: &'a BTreeMap<String, String>, name: &str) -> Option<&'a str> {
    headers
        .get(name)
        .or_else(|| headers.get(&name.to_ascii_lowercase()))
        .or_else(|| headers.get(&name.to_ascii_uppercase()))
        .map(String::as_str)
}

fn validate_hex(value: &str, len: usize, label: &str) -> anyhow::Result<()> {
    if value.len() != len {
        bail!("{label} must be {len} hex chars");
    }
    if !value.chars().all(|ch| ch.is_ascii_digit() || matches!(ch, 'a'..='f')) {
        bail!("{label} must be lowercase hex");
    }
    Ok(())
}

fn new_trace_id() -> String {
    Uuid::new_v4().simple().to_string()
}

fn new_span_id() -> String {
    Uuid::new_v4().simple().to_string()[..16].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_render_traceparent() {
        let raw = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
        let context = parse_traceparent(raw).expect("traceparent");

        assert_eq!(context.trace_id, "4bf92f3577b34da6a3ce929d0e0e4736");
        assert_eq!(context.span_id, "00f067aa0ba902b7");
        assert_eq!(context.trace_flags, 1);
        assert_eq!(context.traceparent(), raw);
    }

    #[test]
    fn inject_and_extract_headers() {
        let mut context = TraceContext::new_sampled();
        context.tracestate.insert("slab".to_owned(), "dev".to_owned());
        let mut headers = BTreeMap::new();

        inject_w3c_headers(&mut headers, &context);
        let extracted = extract_w3c_headers(&headers).expect("extract").expect("context");

        assert_eq!(extracted, context);
    }

    #[test]
    fn invalid_traceparent_is_rejected() {
        assert!(parse_traceparent("00-0-0-01").is_err());
    }
}
