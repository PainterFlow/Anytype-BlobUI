//! Resource links describe completed activity. They never authorize resource reads.
use crate::{object_uri, resource_operation, Operation};
use serde_json::{json, Value};

pub fn needs_display_context(operation: &Operation) -> bool {
    match operation {
        Operation::ReadObject { .. } => true,
        Operation::Content { content } => content.arguments["dry_run"] != true && matches!(content.tool.as_str(),
            "read_document" | "patch_object" | "create_discussion" | "add_chat_message" | "edit_chat_message" |
            "toggle_chat_reaction" | "read_chat" | "create_object" | "create_chat" | "create_collection" | "create_query" | "create_template"),
        _ => false,
    }
}

pub fn tool_result(operation: &Operation, value: Value) -> Value {
    let mut content = vec![json!({"type":"text","text":value.to_string()})];
    let mut resources = Vec::new();
    if let Some(link) = object_link(operation, &value) {
        let Operation::ReadObject { space_id, object_id } = resource_operation(&json!({"uri":link["uri"]})).expect("validated object link") else { unreachable!() };
        // This is source attribution for the returned object/receipt. Mutation
        // outcomes stay in structuredContent. The host hides `created` activities
        // from Sources, so attribute the returned reference as `read` here.
        resources.push(json!({"id":object_uri(&space_id,&object_id),
            "url":format!("https://object.any.coop/{object_id}?spaceId={space_id}"),
            "title":link["title"],"mimeType":"application/json","activities":["read"]}));
        content.push(link);
    }
    let mut result = json!({"content":content,"structuredContent":value,
        "isError":matches!(value["outcome"].as_str(),Some("rejected"|"unknown"))});
    if !resources.is_empty() {
        result["_meta"] = json!({"openai/resourceActivities":{"version":1,"resources":resources}});
    }
    result
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
    fn label(value: &Value) -> Option<&str> { value.as_str().map(str::trim).filter(|s| !s.is_empty()) }
    // Confirmed requested names may be omitted from mutation receipts. Never use
    // a chat message's body as the parent chat title.
    let mut confirmed_name = None;
    if let Operation::Content { content } = operation {
        if value["outcome"] == "confirmed" {
            if matches!(content.tool.as_str(), "create_object" | "create_chat" | "create_collection" | "create_query" | "create_template") {
                confirmed_name = label(&content.arguments["body"]["name"]).or_else(|| label(&content.arguments["body"]["properties"]["name"]));
            } else if content.tool == "patch_object" {
                for op in content.arguments["body"]["ops"].as_array().into_iter().flatten() {
                    if op["op"] == "set_properties" {
                        if let Some(name) = op["set"].get("name") { confirmed_name = Some(label(name).unwrap_or("Untitled object")); }
                        if op["unset"].as_array().is_some_and(|keys| keys.iter().any(|key| key == "name")) { confirmed_name = Some("Untitled object"); }
                    }
                }
            }
        }
    }
    let title = label(&document["name"])
        .or_else(|| label(&document["title"]))
        .or_else(|| document.pointer("/properties/name").and_then(label))
        .or(confirmed_name)
        .or_else(|| label(&value["_anyblob_source"]["name"]))
        .unwrap_or("Untitled object");
    let space_name = label(&value["space_name"]).or_else(|| label(&value["_anyblob_source"]["space"]))
        .unwrap_or("Unknown space");
    Some(json!({"type":"resource_link","uri":uri,"name":format!("{title} · {space_name}"),
        "title":format!("{title} · {space_name}"),"mimeType":"application/json",
        "description":format!("{title} in {space_name}. Opening requires current read access.")}))
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
            assert_eq!(result.get("_meta").is_some(), outcome=="confirmed");
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
            assert_eq!(result["_meta"]["openai/resourceActivities"]["resources"][0]["url"],format!("https://object.any.coop/o?spaceId={space}"));
        }
        for name in ["list_objects","get_collection_objects","get_query_objects","list_schemas","get_chat_messages"] {
            let op = Operation::Content { content: crate::content::ContentRequest { tool:name.into(), arguments:json!({"space_id":"s","object_id":"o"}) }};
            assert_eq!(tool_result(&op,json!({"id":"o","name":"Hit"}))["content"].as_array().unwrap().len(),1);
        }
    }
    #[test]
    fn display_names_cover_projected_reads_and_sparse_receipts_without_ids() {
        let make = |tool:&str, args:Value| Operation::Content { content:crate::content::ContentRequest {tool:tool.into(),arguments:args} };
        let display = |op:&Operation,value:Value| tool_result(op,value)["_meta"]["openai/resourceActivities"]["resources"][0]["title"].clone();
        let read=make("read_document",json!({"space_id":"s","object_id":"o","include":"blocks"}));
        assert_eq!(display(&read,json!({"id":"o","blocks":[],"_anyblob_source":{"name":"Project plan","space":"Work"}})),"Project plan · Work");
        assert_eq!(display(&read,json!({"id":"o"})),"Untitled object · Unknown space");
        let create=make("create_object",json!({"space_id":"s","body":{"properties":{"name":"New plan"}}}));
        assert_eq!(display(&create,json!({"outcome":"confirmed","receipt":{"id":"new"},"_anyblob_source":{"space":"Work"}})),"New plan · Work");
        let patch=make("patch_object",json!({"space_id":"s","object_id":"o","body":{"ops":[{"op":"set_properties","set":{"name":"Renamed"}}]}}));
        assert_eq!(display(&patch,json!({"outcome":"confirmed","receipt":{"etag":"kept"},"_anyblob_source":{"name":"Old name","space":"Work"}})),"Renamed · Work");
        let message=make("edit_chat_message",json!({"space_id":"s","chat_id":"parent","body":{"name":"Child"}}));
        assert_eq!(display(&message,json!({"outcome":"confirmed","receipt":{"name":"Child"},"_anyblob_source":{"name":"Team chat","space":"Work"}})),"Team chat · Work");
    }
}
