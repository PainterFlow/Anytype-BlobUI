use crate::{calculator, kanban_snapshot::Snapshot};
use serde_json::Value;

fn quote(value: &str) -> String {
    let mut result = String::from("\"");
    for ch in value.chars() {
        result.push_str(&format!("\\{:x} ", ch as u32));
    }
    result.push('"');
    result
}
pub fn render(value: Option<&Value>) -> Result<String, String> {
    let Some(value) = value.filter(|v| !v.is_null()) else {
        return Ok(String::new());
    };
    let snapshot: Snapshot = serde_json::from_value(value.clone())
        .map_err(|_| "The saved Kanban result snapshot is malformed.".to_string())?;
    let display = match snapshot.display.as_str() {
        // Compact was a tooltip mode in older versions. Keep old snapshots
        // readable, but render them with the current inline layout.
        "compact" | "balanced" => "balanced",
        "spacious" => "spacious",
        _ => return Err("The saved Kanban result snapshot is unsupported.".into()),
    };
    if !matches!(snapshot.version, 1 | 2 | 3) || snapshot.results.len() > 4096 || snapshot.empty_results.len() > 4096 {
        return Err("The saved Kanban result snapshot is unsupported.".into());
    }
    let mut empty_seen = std::collections::BTreeSet::new();
    for empty in &snapshot.empty_results {
        empty.target.validate()?;
        if !empty_seen.insert(empty.target.clone()) || empty.value.is_some_and(|v| !v.is_finite()) { return Err("Invalid empty-input result.".into()); }
    }
    let mut css = String::new();
    let mut seen = std::collections::BTreeSet::new();
    let mut cards = std::collections::BTreeSet::new();
    let mut properties = std::collections::BTreeMap::new();
    for result in snapshot.results {
        result.target.validate()?;
        if snapshot.version < 3 || result.category_names.is_none() { crate::kanban_snapshot::validate_id(&result.category_id)?; }
        for id in &result.card_ids {
            crate::kanban_snapshot::validate_id(id)?;
            if !cards.insert((result.target.clone(), id.clone())) { return Err("Conflicting card category membership.".into()); }
        }
        if let Some(property) = &result.grouping_property {
            if properties.insert(result.target.clone(), property.clone()).is_some_and(|old| old != *property) { return Err("Conflicting grouping properties.".into()); }
        }
        if result.empty_value.is_some_and(|v| !v.is_finite()) { return Err("Invalid empty-input result.".into()); }
        if snapshot.version == 1
            && matches!(result.value, Some(calculator::CalculationValue::Boolean(_)))
        {
            return Err("Version 1 snapshots require numeric values.".into());
        }
        if result.value.is_some_and(|v| !v.is_finite()) {
            return Err("A Kanban result is outside the numeric range.".into());
        }
        if !seen.insert((result.target.clone(), result.category_id.clone())) {
            return Err("Duplicate Kanban result identities cannot be published.".into());
        }
        let text = calculator::format(result.value);
        // Anytype nests the view's drop target inside the dataview block.
        // Bind the object identity to the active view within that drop target,
        // rather than requiring data-root-id on an ancestor of the block.
        let t = &result.target;
        let legacy_head=format!("[id={}]:has([data-root-id={}] > [id={}].viewItem.active) .viewContent.viewBoard #columns > [id={}] > .head",quote(&format!("block-{}",t.block_id)),quote(&t.object_id),quote(&format!("view-item-{}-{}",t.block_id,t.view_id)),quote(&format!("column-{}",result.category_id)));
        let head = if snapshot.version == 3 && result.category_names.is_some() {
            let property = result.grouping_property.as_deref().ok_or("Missing grouping property.")?;
            crate::kanban_snapshot::validate_id(property)?;
            let names = result.category_names.as_ref().unwrap();
            let mut canonical = names.clone(); canonical.sort(); canonical.dedup();
            if &canonical != names || serde_json::to_string(names).unwrap() != result.category_id {
                return Err("Invalid category name identity.".into());
            }
            let scope = format!("[id={}]:has([data-root-id={}] > [id={}].viewItem.active) .viewContent.viewBoard #columns > .column", quote(&format!("block-{}",t.block_id)), quote(&t.object_id), quote(&format!("view-item-{}-{}",t.block_id,t.view_id)));
            let fallback = format!("{scope} > .head");
            let fallback_key = (result.target.clone(), String::from("__fallback"));
            if seen.insert(fallback_key) {
                css.push_str(&format!("\n{fallback} {{ --blobui-kanban-result: {}; }}\n", quote(&calculator::format(snapshot.empty_results.iter().find(|e| e.target == result.target).map(|e| e.value).unwrap_or(result.empty_value)))));
            }
            if !result.card_ids.is_empty() {
                let selector_bytes: usize = result.card_ids.iter().map(|id| 2 * quote(id).len() + 160).sum();
                if selector_bytes + css.len() > 2 * 1024 * 1024 { return Err("The Kanban result stylesheet is too large to publish safely.".into()); }
                let anchors = result.card_ids.iter().map(|id| format!(".card:not(.add)[data-id={}],.card:not(.add)[id={}]", quote(id), quote(&format!("record-{id}")))).collect::<Vec<_>>().join(",");
                let matched = format!("{scope}:has(> .body :is({anchors})) > .head");
                css.push_str(&format!("{matched} {{ --blobui-kanban-result: {}; }}\n", quote(&text)));
            }
            // Layout rules also cover unmatched headers. Category rules only set the variable.
            fallback
        } else { legacy_head };
        let assignment = if snapshot.version == 3 && result.category_names.is_some() { String::new() } else { format!("{head} {{ --blobui-kanban-result: {}; }}", quote(&text)) };
        css.push_str(&format!("\n{assignment}\n{head} > .sides {{ height: auto; min-height: 28px; align-items: center; }}\n{head} .side.left {{ display: flex; min-width: 0; flex: 1; flex-wrap: wrap; align-items: center; gap: 2px 6px; }}\n{head} .side.left > :first-child {{ width: auto; max-width: 100%; min-width: 0; flex: 0 1 auto; overflow: hidden; }}\n{head} .side.left::after {{ content: var(--blobui-kanban-result); flex: 0 0 auto; box-sizing: border-box; min-width: 24px; min-height: 24px; max-width: 100%; padding: 3px 7px; border-radius: 999px; background: rgb(0 0 0 / 18%); text-align: center; line-height: 18px; font-size: 12px; opacity: var(--blobui-kanban-result-opacity, .7); transition: opacity var(--blobui-kanban-motion, 0ms) ease; white-space: normal; overflow-wrap: anywhere; }}\n{head} .side.right {{ position: relative; }}\n"));
        css.push_str(&format!("{head} .side.right {{ display: flex; }}\n{head} .side.right > :is(.more, .add) {{ visibility: hidden; }}\n{head}:hover .side.right > :is(.more, .add), {head}:focus-within .side.right > :is(.more, .add) {{ visibility: visible; }}\n"));
        if display == "spacious" {
            css.push_str(&format!("{head} > .sides {{ align-items: flex-start; }}\n"));
            css.push_str(&format!("{head} .side.left > :first-child {{ flex-basis: 100%; }}\n{head} .side.left::after {{ content: 'Result: ' var(--blobui-kanban-result); flex-basis: 100%; }}\n"));
        }
        if css.len() > 2 * 1024 * 1024 {
            return Err("The Kanban result stylesheet is too large to publish safely.".into());
        }
    }
    Ok(css)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn card_snapshot_exports_complete_anchors_and_rejects_oversize() {
        let mut value = json!({"version":3,"display":"balanced","calculatedAt":"now","emptyResults":[{"target":{"spaceId":"space","objectId":"object-a","blockId":"board","viewId":"view-a"},"value":0}],"results":[{"target":{"spaceId":"space","objectId":"object-a","blockId":"board","viewId":"view-a"},"groupingProperty":"status","categoryId":"[\"A\"]","categoryNames":["A"],"cardIds":["one","two"],"emptyValue":0,"value":12},{"target":{"spaceId":"space","objectId":"object-a","blockId":"board","viewId":"view-a"},"groupingProperty":"status","categoryId":"[]","categoryNames":[],"cardIds":["unselected"],"emptyValue":0,"value":3}]});
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join(".test-run/kanban-live");
        std::fs::create_dir_all(&root).unwrap();
        let css = render(Some(&value)).unwrap();
        assert!(css.contains(&quote("one")) && css.contains(&quote("two")));
        assert!(!css.contains("column-"));
        for display in ["balanced", "spacious"] {
            value["display"] = json!(display);
            std::fs::write(root.join(format!("v3-{display}.css")), render(Some(&value)).unwrap()).unwrap();
        }
        value["results"][0]["cardIds"] = json!((0..20000).map(|i| format!("card-{i}-{}", "x".repeat(100))).collect::<Vec<_>>());
        assert!(render(Some(&value)).unwrap_err().contains("too large"));
    }
    #[test]
    fn scopes_results_and_exports_actual_renderer_fixture() {
        let value = json!({"version":1,"display":"balanced","calculatedAt":"2026-10-23T00:00:00Z","results":[{"target":{"spaceId":"space","objectId":"object-a","blockId":"board","viewId":"view-a"},"categoryId":"option-a","value":12},{"target":{"spaceId":"space","objectId":"object-a","blockId":"board","viewId":"view-a"},"categoryId":"empty","value":null}]});
        let css = render(Some(&value)).unwrap();
        assert!(css.contains(":has("));
        assert!(css.contains("flex-wrap: wrap"));
        assert!(css.contains("opacity: var(--blobui-kanban-result-opacity, .7)"));
        assert!(!css.contains("@container"));
        assert!(!css.contains("f(x)"));
        assert!(!css.contains("side.right::after"));
        assert!(!css.contains("NaN"));
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join(".test-run/kanban-live");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("balanced.css"), &css).unwrap();
        for display in ["compact", "spacious"] {
            let mut value = value.clone();
            value["display"] = json!(display);
            std::fs::write(
                root.join(if display == "compact" { "legacy-compact.css" } else { "spacious.css" }),
                render(Some(&value)).unwrap(),
            )
            .unwrap();
        }
        let mut legacy_compact = value.clone();
        legacy_compact["display"] = json!("compact");
        assert_eq!(render(Some(&legacy_compact)).unwrap(), css);
        let mut invalid = value.clone();
        invalid["results"][0]["target"]["blockId"] = json!("bad\nidentity");
        assert!(render(Some(&invalid)).is_err());
        let mut escaped = value;
        escaped["results"][0]["categoryId"] = json!("x\"]{color:red}/*");
        let css = render(Some(&escaped)).unwrap();
        assert!(!css.contains("color:red"));
    }
    #[test]
    fn boolean_snapshots_format_and_validate_versions() {
        let mut value = json!({"version":2,"display":"balanced","calculatedAt":"now","results":[{"target":{"spaceId":"space","objectId":"object","blockId":"board","viewId":"view"},"categoryId":"empty","value":true}]});
        for boolean in [true, false] {
            value["results"][0]["value"] = json!(boolean);
            let css = render(Some(&value)).unwrap();
            assert!(css.contains(&quote(if boolean { "True" } else { "False" })));
        }
        value["version"] = json!(1);
        assert!(render(Some(&value)).is_err());
        value["results"][0]["value"] = json!(12);
        assert!(render(Some(&value)).is_ok());
        value["version"] = json!(4);
        assert!(render(Some(&value)).is_err());
    }
}
