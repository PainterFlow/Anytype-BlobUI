//! BlobUI operations are separate from Anytype content writes.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlobuiRequest {
    pub tool: String,
    pub arguments: Value,
}
pub const NAMES: [&str; 12] = [
    "list_kanban_views",
    "get_kanban_calculation",
    "preview_kanban_calculation",
    "set_kanban_calculation",
    "remove_kanban_calculation",
    "refresh_kanban_calculation",
    "list_calendar_color_targets",
    "get_calendar_color_rule",
    "preview_calendar_color_rule",
    "set_calendar_color_rule",
    "remove_calendar_color_rule",
    "refresh_calendar_colors",
];
impl BlobuiRequest {
    pub fn mutates(&self) -> bool {
        self.tool.starts_with("set_")
            || self.tool.starts_with("remove_")
            || self.tool.starts_with("refresh_")
    }
    pub fn calendar(&self) -> bool {
        self.tool.contains("calendar")
    }
    pub fn validate(&self) -> Result<(), String> {
        let spec = tools()
            .into_iter()
            .find(|s| s["name"] == self.tool)
            .ok_or("Unknown BlobUI tool.")?;
        crate::content::validate_schema(&spec["inputSchema"], &self.arguments, "arguments")?;
        fn ids(v: &Value, key: &str) -> Result<(), String> {
            if let Some(s) = v.as_str() {
                if key.ends_with("id") || key.ends_with("Id") || key == "type_key" {
                    if s.is_empty()
                        || s.len() > 512
                        || !s
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
                        || [".", ".."].contains(&s)
                    {
                        return Err("Invalid explicit BlobUI target ID.".into());
                    }
                }
            }
            if let Some(o) = v.as_object() {
                for (k, v) in o {
                    ids(v, k)?;
                }
            }
            Ok(())
        }
        ids(&self.arguments, "")?;
        if let Some(target) = self.arguments.get("target") {
            if target["spaceId"] != self.arguments["space_id"] {
                return Err("Target belongs to a different space.".into());
            }
        }
        Ok(())
    }
}
pub fn token_schema() -> Value {
    let text = json!({"type":"string","minLength":1,"maxLength":256});
    let mut forms = Vec::new();
    for kind in ["count", "open", "close", "comma"] {
        forms.push(json!({"type":"object","properties":{"kind":{"const":kind}},"required":["kind"],"additionalProperties":false}));
    }
    for kind in ["number", "boolean", "operator", "function"] {
        let value = if kind == "boolean" {
            json!({"type":"boolean"})
        } else {
            text.clone()
        };
        forms.push(json!({"type":"object","properties":{"kind":{"const":kind},"value":value},"required":["kind","value"],"additionalProperties":false}));
    }
    forms.push(json!({"type":"object","properties":{"kind":{"const":"property"},"property":text},"required":["kind","property"],"additionalProperties":false}));
    forms.push(json!({"type":"object","properties":{"kind":{"const":"aggregate"},"property":text,"operation":{"enum":["total","sum","average","avg","median","minimum","min","maximum","max"]},"calculation":{"oneOf":[{"type":"null"},{"type":"array","minItems":1,"maxItems":256,"items":{"type":"object"}}]}},"required":["kind","property","operation"],"additionalProperties":false}));
    json!({"oneOf":forms})
}
pub fn tools() -> Vec<Value> {
    let id = json!({"type":"string","minLength":1,"maxLength":512});
    let target = json!({"type":"object","properties":{"spaceId":id,"objectId":id,"blockId":id,"viewId":id},"required":["spaceId","objectId","blockId","viewId"],"additionalProperties":false});
    NAMES.iter().map(|name| {
        let request=BlobuiRequest{tool:(*name).into(),arguments:Value::Null};
        let mut props=json!({"space_id":id}); let mut required=vec!["space_id"];
        if name.starts_with("list_") {props["object_id"]=id.clone();}
        else {
            if request.calendar() { props["type_key"]=id.clone(); required.push("type_key"); props["target"]=target.clone(); }
            else { props["target"]=target.clone(); required.push("target"); }
            if name.starts_with("set_") || name.starts_with("preview_") {
                if request.calendar() {props["rule"]=json!({"oneOf":[{"type":"object","properties":{"mode":{"enum":["inherit","none"]}},"required":["mode"],"additionalProperties":false},{"type":"object","properties":{"mode":{"const":"property"},"key":id},"required":["mode","key"],"additionalProperties":false}]});required.push("rule");}
                else {props["tokens"]=json!({"type":"array","minItems":1,"maxItems":256,"items":token_schema()});required.push("tokens");}
            }
            if request.mutates() {props["revision"]=json!({"type":"string","minLength":1,"maxLength":128});required.push("revision");props["dry_run"]=json!({"type":"boolean","default":false});}
        }
        json!({"name":name,"description":format!("{} in one permitted space. Kanban calculations support only single-select and checkbox grouping; multi-select returns unsupported_grouping_multi_select. Each single-select category identifies one option; several separate categories are valid. These tools do not create or edit board categories or grouping. Read current revision, preview changes, then publish. Publication requires a manual Anytype reload. Never retry unknown outcomes.",name.replace('_'," ")),
            "inputSchema":{"type":"object","properties":props,"required":required,"additionalProperties":false},
            "annotations":{"readOnlyHint":!request.mutates(),"destructiveHint":false,"idempotentHint":!request.mutates(),"openWorldHint":false}})
    }).collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strict_targets_revisions_and_annotations() {
        for spec in tools() {
            let r = BlobuiRequest {
                tool: spec["name"].as_str().unwrap().into(),
                arguments: json!({"space_id":"space"}),
            };
            assert_eq!(spec["annotations"]["readOnlyHint"], !r.mutates());
            assert_eq!(r.validate().is_ok(), r.tool.starts_with("list_"));
        }
        let args = json!({"space_id":"s","target":{"spaceId":"s","objectId":"o","blockId":"b","viewId":"v"},"revision":"r","tokens":[{"kind":"count"}],"dry_run":true});
        assert!(crate::operation("set_kanban_calculation", args.clone()).is_ok());
        for key in ["threadId", "consent", "path"] {
            let mut a = args.clone();
            a[key] = json!("forged");
            assert!(crate::operation("set_kanban_calculation", a).is_err());
        }
        let mut a = args;
        a["target"]["spaceId"] = json!("other");
        assert!(crate::operation("set_kanban_calculation", a).is_err());
    }
}
