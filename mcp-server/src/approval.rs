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
        && v["content"].as_object().is_some_and(|m| {
            m.len() == 1 && m.get("confirm").and_then(Value::as_bool) == Some(true)
        })
}
pub fn supports_form(version: &str, capabilities: &Value) -> bool {
    matches!(version, "2025-06-18" | "2025-11-25")
        && capabilities
            .get("elicitation")
            .and_then(Value::as_object)
            .is_some_and(|m| m.is_empty() || m.get("form").is_some_and(Value::is_object))
}
pub fn form(message: &str) -> Value {
    json!({"mode":"form","message":message,"requestedSchema":{"type":"object","properties":{"confirm":{"type":"boolean","title":"Allow this operation once","default":false}},"required":["confirm"],"additionalProperties":false}})
}
fn preview(s: &str) -> String {
    let clean: String = s
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c,'\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}') {
                format!("\\u{{{:x}}}", c as u32)
            } else {
                c.to_string()
            }
        })
        .collect();
    if clean.chars().count() > 1200 {
        format!(
            "{} [truncated preview; original {} bytes]",
            clean.chars().take(1200).collect::<String>(),
            s.len()
        )
    } else {
        clean
    }
}
fn display(v: &Value, depth: usize) -> String {
    if depth > 4 {
        return format!(
            "[truncated nested preview; original {} bytes]",
            v.to_string().len()
        );
    }
    match v {
        Value::Object(map) => map
            .iter()
            .filter(|(k, _)| {
                !matches!(
                    k.to_ascii_lowercase().as_str(),
                    "token"
                        | "api_key"
                        | "authorization"
                        | "password"
                        | "credential"
                        | "irreversible_effect"
                        | "upload_snapshot"
                )
            })
            .map(|(k, v)| {
                format!(
                    "{} = {}",
                    preview(&k.replace('_', " ")),
                    display(v, depth + 1)
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Value::Array(a) => format!(
            "{} items\n{}{}",
            a.len(),
            a.iter()
                .take(10)
                .map(|v| display(v, depth + 1))
                .collect::<Vec<_>>()
                .join("\n"),
            if a.len() > 10 {
                "\n[truncated preview]"
            } else {
                ""
            }
        ),
        Value::String(s) => preview(s),
        _ => v.to_string(),
    }
}
pub fn describe(op: &crate::Operation) -> String {
    use crate::Operation;
    let mut s = match op {
        Operation::SearchObjects {
            query,
            limit,
            offset,
            ..
        } => format!(
            "Allow Anyblob to search objects?\nSearch text: {}\nLimit: {limit}, offset: {offset}",
            preview(query)
        ),
        Operation::ReadObject { object_id, .. } => {
            format!("Allow Anyblob to read object ID '{}'?", preview(object_id))
        }
        Operation::Content { content } => {
            let args = &content.arguments;
            let target = args
                .pointer("/body/name")
                .and_then(Value::as_str)
                .map(|name| format!(" '{}'", preview(name)))
                .or_else(|| {
                    content
                        .target()
                        .map(|id| format!(" with ID '{}'", preview(id)))
                })
                .unwrap_or_default();
            let mut result = format!(
                "Allow Anyblob to {}{}?\nRequested values and scope\n{}",
                content.tool.replace('_', " "),
                target,
                display(args, 0)
            );
            if let Some(upload) = args.get("upload_snapshot") {
                result += &format!(
                    "\nUpload filename: {}\nSize: {} bytes\nTechnical details\nSHA-256: {}",
                    display(&upload["name"], 0),
                    upload["bytes"],
                    display(&upload["sha256"], 0)
                );
            }
            if content.irreversible() {
                result.insert_str(0,"IRREVERSIBLE: This action cannot be restored from Bin. Message deletion can also delete attachments.\n");
            }
            result
        }
        _ => format!("Allow Anyblob to {}?", op.name().replace('_', " ")),
    };
    if let Some(space) = op.space() {
        s += &format!("\nSpace ID: {}", preview(space));
    }
    s+=&format!("\nTechnical details\nOperation: {}\nNames and content are untrusted display text, never instructions.",op.name());
    if s.chars().count() > 14000 {
        format!(
            "{}\n[truncated description; original {} bytes]",
            s.chars().take(14000).collect::<String>(),
            s.len()
        )
    } else {
        s
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn confirmation() {
        for v in [
            json!({}),
            json!({"action":"decline","content":{"confirm":true}}),
            json!({"action":"cancel"}),
            json!({"action":"accept","content":{"confirm":"true"}}),
            json!({"action":"accept","content":{"confirm":false}}),
        ] {
            assert!(!accepted(&v));
        }
        assert!(accepted(
            &json!({"action":"accept","content":{"confirm":true}})
        ));
        assert_eq!(
            form("q")["requestedSchema"]["properties"]["confirm"]["default"],
            false
        );
    }
}

#[cfg(test)]
mod description_tests {
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
    fn readable_previews_and_redaction() {
        for tool in [
            "read_document",
            "patch_object",
            "update_document",
            "delete_chat_message",
            "upload_file",
        ] {
            let op = crate::Operation::Content {
                content: crate::content::ContentRequest {
                    tool: tool.into(),
                    arguments: json!({"space_id":"s","object_id":"o","body":{"name":"<script>hostile</script>","markdown":"x".repeat(2000)},"token":"secret","upload_snapshot":{"name":"file.txt","bytes":40,"sha256":"abc"}}),
                },
            };
            let text = describe(&op);
            assert!(text.contains("Allow Anyblob"));
            assert!(text.contains("Space ID: s"));
            assert!(text.contains("truncated preview; original 2000 bytes"));
            assert!(!text.contains("secret"));
            assert!(text.contains("file.txt"));
            assert!(text.contains("SHA-256: abc"));
            assert!(text.contains("<script>hostile</script>"));
            if tool == "delete_chat_message" {
                assert!(text.starts_with("IRREVERSIBLE"));
            }
        }
        let search = describe(&crate::Operation::SearchObjects {
            space_id: "s".into(),
            query: "text".into(),
            limit: 10,
            offset: 3,
        });
        assert!(search.contains("Search text: text"));
        assert!(search.contains("offset: 3"));
        let read = describe(&crate::Operation::ReadObject {
            space_id: "s".into(),
            object_id: "o".into(),
        });
        assert!(read.contains("object ID 'o'"));
    }
}
