//! Synthetic production-serializer results for the isolated installed-host probe.
use blobui_mcp::{sources::tool_result, Operation, content::ContentRequest};
use serde_json::json;
fn main() {
    let read = |space: &str, object: &str, title: &str| tool_result(
        &Operation::ReadObject { space_id:space.into(), object_id:object.into() },
        json!({"id":object,"name":title,"space_name":format!("Space {space}")}));
    let mutation = |outcome: &str, dry: bool| tool_result(
        &Operation::Content { content:ContentRequest { tool:"create_object".into(), arguments:json!({"space_id":"a","dry_run":dry}) } },
        json!({"outcome":outcome,"receipt":{"id":"created","name":"Created"}}));
    println!("{}",json!({
        "read":read("a","one","Same title"),
        "renamed":read("a","one","Renamed"),
        "duplicateTitle":read("b","two","Same title"),
        "created":mutation("confirmed",false),
        "unknown":mutation("unknown",false),
        "dry":mutation("validated",true),
        "search":tool_result(&Operation::SearchObjects{space_id:"a".into(),query:"".into(),limit:20,offset:0},json!({"objects":[{"id":"hit","name":"Hit"}]}))
    }));
}
