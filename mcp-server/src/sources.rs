//! Resource links describe completed activity. They never authorize resource reads.
use crate::{object_uri, resource_operation, Operation};
use serde_json::{json, Value};

pub fn tool_result(operation: &Operation, value: Value) -> Value {
    let mut content = vec![json!({"type":"text","text":value.to_string()})];
    if let Some(link) = object_link(operation, &value) {
        content.push(link);
    }
    json!({"content":content,"structuredContent":value,
        "isError":matches!(value["outcome"].as_str(),Some("rejected"|"unknown"))})
}

fn object_link(operation: &Operation, value: &Value) -> Option<Value> {
    let (space, object, document) = match operation {
        Operation::ReadObject { space_id, object_id } =>
            (space_id.as_str(), object_id.as_str(), value),
        Operation::Content { content } => {
            let args = &content.arguments;
            if args["dry_run"] == true { return None; }
            let space = args["space_id"].as_str()?;
            let document = if content.mutates() {
                if value["outcome"] != "confirmed" { return None; }
                &value["receipt"]
            } else { value };
            let object = match content.tool.as_str() {
                "read_document" | "patch_object" | "create_discussion" => args["object_id"].as_str()?,
                "add_chat_message" | "edit_chat_message" | "toggle_chat_reaction" | "read_chat" => args["chat_id"].as_str()?,
                "create_object" | "create_chat" | "create_collection" | "create_query" | "create_template" => document["id"].as_str()?,
                _ => return None,
            };
            // Message and discussion receipts describe the child, not its parent.
            let document = if matches!(content.tool.as_str(), "create_discussion" | "add_chat_message" | "edit_chat_message" | "toggle_chat_reaction" | "read_chat") { &Value::Null } else { document };
            (space, object, document)
        }
        _ => return None,
    };
    // Validate even server-supplied creation IDs before constructing an openable URI.
    let uri = object_uri(space, object);
    resource_operation(&json!({"uri":uri})).ok()?;
    let title = document["name"].as_str().or_else(|| document["title"].as_str())
        .filter(|title| !title.trim().is_empty()).unwrap_or(object);
    let space_name = value["space_name"].as_str().filter(|name| !name.trim().is_empty()).unwrap_or(space);
    Some(json!({"type":"resource_link","uri":uri,"name":format!("{title} · {space_name}"),
        "title":format!("{title} · {space_name}"),"mimeType":"application/json",
        "description":format!("Anyblob object {object} in space {space}. Opening requires current read access.")}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn links_preserve_results_and_exclude_unconfirmed_activity() {
        let op = Operation::Content { content: crate::content::ContentRequest { tool:"patch_object".into(), arguments:json!({"space_id":"s","object_id":"o"}) }};
        for outcome in ["validated", "unknown", "rejected", "confirmed"] {
            let value = json!({"outcome":outcome,"receipt":{"name":"Title","etag":"e","warnings":["kept"]}});
            let result = tool_result(&op, value.clone());
            assert_eq!(result["structuredContent"], value);
            assert_eq!(result["content"][0]["text"], value.to_string());
            assert_eq!(result["content"].as_array().unwrap().len(), if outcome=="confirmed" {2} else {1});
        }
    }
    #[test]
    fn chat_edits_link_parent_and_creation_ids_are_checked() {
        let op = Operation::Content { content: crate::content::ContentRequest { tool:"edit_chat_message".into(), arguments:json!({"space_id":"s","chat_id":"parent","message_id":"message"}) }};
        assert_eq!(tool_result(&op,json!({"outcome":"confirmed","receipt":{}}))["content"][1]["uri"],"anyblob://objects/s/parent");
        let create = Operation::Content { content: crate::content::ContentRequest { tool:"create_object".into(), arguments:json!({"space_id":"s"}) }};
        for id in ["../bad", "a/b", "", "%20"] {
            assert_eq!(tool_result(&create,json!({"outcome":"confirmed","receipt":{"id":id}}))["content"].as_array().unwrap().len(),1);
        }
        assert_eq!(tool_result(&create,json!({"outcome":"confirmed","receipt":{"id":"created","name":"Created"}}))["content"][1]["uri"],"anyblob://objects/s/created");
        let dry = Operation::Content { content: crate::content::ContentRequest { tool:"create_object".into(), arguments:json!({"space_id":"s","dry_run":true}) }};
        assert_eq!(tool_result(&dry,json!({"outcome":"confirmed","receipt":{"id":"created"}}))["content"].as_array().unwrap().len(),1);
    }
    #[test]
    fn reads_distinguish_spaces_without_linking_search_members() {
        for space in ["a", "b"] {
            let op = Operation::ReadObject { space_id:space.into(), object_id:"o".into() };
            let result = tool_result(&op,json!({"name":"Same title","space_name":format!("Space {space}")}));
            assert_eq!(result["content"][1]["uri"],object_uri(space,"o"));
            assert_eq!(result["content"][1]["title"],format!("Same title · Space {space}"));
        }
        for name in ["list_objects","get_collection_objects","get_query_objects","list_schemas","get_chat_messages"] {
            let op = Operation::Content { content: crate::content::ContentRequest { tool:name.into(), arguments:json!({"space_id":"s","object_id":"o"}) }};
            assert_eq!(tool_result(&op,json!({"id":"o","name":"Hit"}))["content"].as_array().unwrap().len(),1);
        }
    }
}
