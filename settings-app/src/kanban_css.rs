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
    if snapshot.version != 1
        || !matches!(
            snapshot.display.as_str(),
            "compact" | "balanced" | "spacious"
        )
        || snapshot.results.len() > 4096
    {
        return Err("The saved Kanban result snapshot is unsupported.".into());
    }
    let mut css = String::new();
    let mut seen = std::collections::BTreeSet::new();
    for result in snapshot.results {
        result.target.validate()?;
        crate::kanban::validate_id(&result.category_id)?;
        if result.value.is_some_and(|v| !v.is_finite()) {
            return Err("A Kanban result is outside the numeric range.".into());
        }
        if !seen.insert((result.target.clone(), result.category_id.clone())) {
            return Err("Duplicate Kanban result identities cannot be published.".into());
        }
        let text = calculator::format(result.value);
        // data-root-id comes from Anytype's drop target; IDs below come from
        // full AnyBlock documents and the core Kanban grouping rules.
        let t = &result.target;
        let head=format!("[data-root-id={}] [id={}]:has([id={}].viewItem.active) .viewContent.viewBoard #columns > [id={}] > .head",quote(&t.object_id),quote(&format!("block-{}",t.block_id)),quote(&format!("view-item-{}-{}",t.block_id,t.view_id)),quote(&format!("column-{}",result.category_id)));
        css.push_str(&format!("\n{head} {{ --blobui-kanban-result: {}; }}\n{head} > .sides {{ container: blobui-kanban / inline-size; }}\n{head} .side.left {{ display: flex; min-width: 0; flex: 1; gap: 6px; }}\n{head} .side.left > :first-child {{ min-width: 0; overflow: hidden; }}\n{head} .side.left::after {{ content: var(--blobui-kanban-result); flex: none; font-size: 12px; opacity: .7; white-space: nowrap; }}\n{head} .side.right {{ position: relative; }}\n{head} .side.right::before {{ content: 'f(x)'; display: none; font-size: 12px; margin-right: 6px; opacity: .7; pointer-events: none; }}\n{head} .side.right::after {{ content: var(--blobui-kanban-result); visibility: hidden; position: absolute; bottom: calc(100% + 6px); right: 0; z-index: 100; width: max-content; max-width: 240px; padding: 8px 12px; border-radius: 8px; background: var(--color-bg-primary, #202020); border: 1px solid var(--color-shape-secondary, #888); color: var(--color-text-primary, inherit); pointer-events: none; }}\n",quote(&text)));
        css.push_str(&format!("{head} .side.right {{ display: flex; }}\n{head} .side.right > :is(.more, .add) {{ visibility: hidden; }}\n{head}:hover .side.right > :is(.more, .add), {head}:focus-within .side.right > :is(.more, .add) {{ visibility: visible; }}\n"));
        let compact=format!("{head} .side.left::after {{ display: none; }}\n{head} .side.right::before {{ display: inline-block; }}\n{head}:hover .side.right::after, {head}:focus-within .side.right::after {{ visibility: visible; }}\n");
        if snapshot.display == "compact" {
            css.push_str(&compact);
        } else {
            if snapshot.display == "spacious" {
                css.push_str(&format!("{head} > .sides {{ height: auto; min-height: 28px; align-items: flex-start; }}\n"));
                css.push_str(&format!("{head} .side.left {{ flex-wrap: wrap; }}\n{head} .side.left > :first-child {{ flex-basis: 100%; }}\n{head} .side.left::after {{ content: 'Result: ' var(--blobui-kanban-result); flex-basis: 100%; }}\n"));
            }
            let threshold =
                text.chars().count() + if snapshot.display == "balanced" { 8 } else { 7 };
            css.push_str(&format!(
                "@container blobui-kanban (max-width: {threshold}em) {{\n{compact}}}\n"
            ));
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
        assert!(css.contains("@container"));
        assert!(!css.contains("NaN"));
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join(".test-run/kanban-live");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("balanced.css"), css).unwrap();
        for display in ["compact", "spacious"] {
            let mut value = value.clone();
            value["display"] = json!(display);
            std::fs::write(
                root.join(format!("{display}.css")),
                render(Some(&value)).unwrap(),
            )
            .unwrap();
        }
        let mut invalid = value.clone();
        invalid["results"][0]["target"]["blockId"] = json!("bad\nidentity");
        assert!(render(Some(&invalid)).is_err());
        let mut escaped = value;
        escaped["results"][0]["categoryId"] = json!("x\"]{color:red}/*");
        let css = render(Some(&escaped)).unwrap();
        assert!(!css.contains("color:red"));
    }
}
