use crate::{calculator, kanban::Snapshot};
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
    if !matches!(snapshot.version, 1 | 2) || snapshot.results.len() > 4096 {
        return Err("The saved Kanban result snapshot is unsupported.".into());
    }
    let mut css = String::new();
    let mut seen = std::collections::BTreeSet::new();
    for result in snapshot.results {
        result.target.validate()?;
        crate::kanban::validate_id(&result.category_id)?;
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
        let head=format!("[id={}]:has([data-root-id={}] > [id={}].viewItem.active) .viewContent.viewBoard #columns > [id={}] > .head",quote(&format!("block-{}",t.block_id)),quote(&t.object_id),quote(&format!("view-item-{}-{}",t.block_id,t.view_id)),quote(&format!("column-{}",result.category_id)));
        css.push_str(&format!("\n{head} {{ --blobui-kanban-result: {}; }}\n{head} > .sides {{ height: auto; min-height: 28px; align-items: center; }}\n{head} .side.left {{ display: flex; min-width: 0; flex: 1; flex-wrap: wrap; align-items: center; gap: 2px 6px; }}\n{head} .side.left > :first-child {{ width: auto; max-width: 100%; min-width: 0; flex: 0 1 auto; overflow: hidden; }}\n{head} .side.left::after {{ content: var(--blobui-kanban-result); flex: 0 0 auto; box-sizing: border-box; min-width: 24px; min-height: 24px; max-width: 100%; padding: 3px 7px; border-radius: 999px; background: rgb(0 0 0 / 18%); text-align: center; line-height: 18px; font-size: 12px; opacity: var(--blobui-kanban-result-opacity, .7); transition: opacity var(--blobui-kanban-motion, 0ms) ease; white-space: normal; overflow-wrap: anywhere; }}\n{head} .side.right {{ position: relative; }}\n",quote(&text)));
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
        value["version"] = json!(3);
        assert!(render(Some(&value)).is_err());
    }
}
