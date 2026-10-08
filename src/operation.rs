//! Builds the exact `OperationWire` JSON shape `pollux-core`'s wire
//! protocol (`pollux-protocol/1`) already defines —
//! `{"capability":"read","resource_domain":"filesystem","resource_value":"..."}`
//! — the same shape already exercised by
//! `pollux-polyglot-native-bridge`'s own integration tests. This module
//! does not invent a parallel format; it is the one Core's own
//! `OperationWire::into_operation` expects.
//!
//! This PoC only ever asks for the `read` Capability: every intercepted
//! `open`/`openat`, regardless of its `flags`, is represented as a
//! `filesystem.read` request. See the crate-level README for why that is
//! a deliberate scope limitation, not an oversight.

/// Core's own answer to one Operation, read out of the
/// `EvaluationTraceWire` JSON `PolluxEngine::evaluate` returns — not a
/// type this crate invents independently of Core's vocabulary, just the
/// two outcome strings Core's wire format already uses.
pub enum Outcome {
    Allow,
    Deny,
}

pub fn build_read_operation(path: &str) -> String {
    format!(
        r#"{{"capability":"read","resource_domain":"filesystem","resource_value":"{}"}}"#,
        json_escape(path)
    )
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Reads the `outcome` field out of Core's trace JSON by substring
/// search — the same style `pollux-polyglot-native-bridge`'s own tests
/// already use (`trace_json.contains("\"outcome\":\"allow\"")`), not a
/// JSON parser this crate is trying to be. `None` means the shape was
/// not recognized at all; the caller must treat that as an
/// infrastructure failure (deny), never assume allow.
pub fn parse_outcome(trace_json: &str) -> Option<Outcome> {
    if trace_json.contains("\"outcome\":\"allow\"") {
        Some(Outcome::Allow)
    } else if trace_json.contains("\"outcome\":\"deny\"") {
        Some(Outcome::Deny)
    } else {
        None
    }
}
