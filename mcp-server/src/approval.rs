//! Connection-local, one-shot approvals.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
pub const VERSION: u32 = 2;
pub const UPDATE: &str = "Update both BlobUI and the Anyblob plugin, then reconnect Codex.";
#[derive(Serialize, Deserialize)]
#[serde(tag = "frame", rename_all = "snake_case", deny_unknown_fields)]
pub enum Frame {
    ApprovalRequest {
        version: u32,
        operation_id: String,
        approval_id: String,
        message: String,
    },
    ApprovalDecision {
        version: u32,
        operation_id: String,
        approval_id: String,
        accepted: bool,
    },
    Response {
        version: u32,
        response: crate::Response,
    },
}
pub fn accepted(v: &Value) -> bool {
    v["action"] == "accept"
        && v.get("content")
            .is_none_or(|c| c.as_object().is_some_and(|m| m.is_empty()))
}
pub fn supports_form(version: &str, capabilities: &Value) -> bool {
    matches!(version, "2025-06-18" | "2025-11-25")
        && capabilities
            .get("elicitation")
            .and_then(Value::as_object)
            .is_some_and(|m| m.is_empty() || m.get("form").is_some_and(Value::is_object))
}

pub fn form(message: &str) -> Value {
    json!({"mode":"form","message":message,"requestedSchema":{"type":"object","properties":{},"additionalProperties":false}})
}
fn preview(s: &str) -> String {
    s.chars()
        .take(240)
        .map(|c| {
            if c.is_control() || matches!(c,'\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}') {
                ' '
            } else {
                c
            }
        })
        .collect()
}
pub fn describe(op: &crate::Operation) -> String {
    use crate::Operation;
    match op {
        Operation::SearchObjects {
            space_id, query, ..
        } => format!(
            "An agent wants to search for '{}' in {}.",
            preview(query),
            preview(space_id)
        ),
        Operation::ReadObject {
            space_id,
            object_id,
        } => format!(
            "An agent wants to read object '{}' in {}.",
            preview(object_id),
            preview(space_id)
        ),
        Operation::Content { content } => {
            let args = &content.arguments;
            let context = &args["_approval_context"];
            let space = context["space"]
                .as_str()
                .or_else(|| args["space_id"].as_str())
                .unwrap_or("the selected space");
            let name = context["name"]
                .as_str()
                .or_else(|| args.pointer("/body/name").and_then(Value::as_str))
                .or_else(|| content.target());
            let kind = context["type"].as_str();
            let mut text = format!(
                "An agent wants to {}{}{} in {}.",
                content.tool.replace('_', " "),
                name.map(|n| format!(" '{}'", preview(n)))
                    .unwrap_or_default(),
                kind.map(|t| format!(" of the type '{}'", preview(t)))
                    .unwrap_or_default(),
                preview(space)
            );
            if content.irreversible() {
                text.push_str(" This action cannot be restored from Bin. Deleting a message can also delete its attachments.");
            }
            text
        }
        _ => format!("An agent wants to {}.", op.name().replace('_', " ")),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capability_negotiation() {
        for version in ["2024-11-05", "2025-03-26"] {
            assert!(!supports_form(version, &json!({"elicitation":{}})));
        }
        for cap in [
            json!({}),
            json!({"elicitation":null}),
            json!({"elicitation":{"url":{}}}),
            json!({"elicitation":{"form":false}}),
        ] {
            assert!(!supports_form("2025-11-25", &cap));
        }
        for cap in [
            json!({"elicitation":{}}),
            json!({"elicitation":{"form":{}}}),
        ] {
            assert!(supports_form("2025-06-18", &cap));
        }
    }
    #[test]
    fn confirmation() {
        for v in [
            json!({}),
            json!({"action":"decline"}),
            json!({"action":"cancel"}),
            json!({"action":"accept","content":{"confirm":true}}),
            json!({"action":"accept","content":null}),
        ] {
            assert!(!accepted(&v));
        }
        assert!(accepted(&json!({"action":"accept","content":{}})));
        assert!(accepted(&json!({"action":"accept"})));
        assert_eq!(form("q")["requestedSchema"]["properties"], json!({}));
    }
    #[test]
    fn readable_description() {
        let op = crate::Operation::Content {
            content: crate::content::ContentRequest {
                tool: "delete_object".into(),
                arguments: json!({"space_id":"s","object_id":"o","_approval_context":{"name":"Roadmap","type":"Page","space":"Work"},"token":"secret","body":{"markdown":"private body"}}),
            },
        };
        assert_eq!(
            describe(&op),
            "An agent wants to delete object 'Roadmap' of the type 'Page' in Work."
        );
    }
}
