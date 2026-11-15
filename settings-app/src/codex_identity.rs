//! Resolve a chat to the project it authenticated with through the Anyblob MCP tool.
use serde_json::Value;
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Membership {
    Project(String),
    Projectless,
}

fn valid_chat_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

fn valid_project_id(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub(crate) fn resolve_from_registry(
    thread: &str,
    projects: &[Value],
    discovered_ids: &BTreeSet<String>,
) -> Result<Membership, String> {
    if !valid_chat_id(thread) {
        return Err("Codex supplied invalid chat identity metadata.".into());
    }
    let matches = projects
        .iter()
        .filter_map(|project| {
            let id = project["id"].as_str()?;
            if !valid_project_id(id) || !discovered_ids.contains(id) {
                return None;
            }
            project["authenticatedChats"]
                .as_array()?
                .iter()
                .any(|chat| chat.as_str() == Some(thread))
                .then(|| id.to_string())
        })
        .collect::<BTreeSet<_>>();
    match matches.len() {
        0 => Ok(Membership::Projectless),
        1 => Ok(Membership::Project(matches.into_iter().next().unwrap())),
        _ => Err("This chat is authenticated to multiple Anyblob projects. Authenticate it again from the intended project.".into()),
    }
}

pub fn resolve(thread: &str) -> Result<Membership, String> {
    if !valid_chat_id(thread) {
        return Err("Codex supplied invalid chat identity metadata.".into());
    }
    let projects = crate::codex_projects::discover()?;
    crate::anyblob::synchronize_project_registry(&projects)?;
    let install = crate::resolve_install_directory(&crate::app_data_directory())?;
    let _ = crate::settings_data_path(&install)?;
    let root = crate::settings_root_for_install(&install)?;
    let records = crate::settings_store::load_anyblob_project_registry(&root)?;
    let discovered = projects
        .into_iter()
        .map(|project| project.id)
        .collect::<BTreeSet<_>>();
    resolve_from_registry(thread, &records, &discovered)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn resolves_only_chats_on_current_discovered_projects() {
        let records = vec![
            json!({"id":"a".repeat(64),"authenticatedChats":["chat-a"]}),
            json!({"id":"b".repeat(64),"authenticatedChats":["chat-b"]}),
            json!({"id":"c".repeat(64),"authenticatedChats":["deleted-chat"]}),
        ];
        let discovered = ["a".repeat(64), "b".repeat(64)]
            .into_iter()
            .collect::<BTreeSet<_>>();
        assert_eq!(
            resolve_from_registry("chat-a", &records, &discovered).unwrap(),
            Membership::Project("a".repeat(64))
        );
        assert_eq!(
            resolve_from_registry("chat-b", &records, &discovered).unwrap(),
            Membership::Project("b".repeat(64))
        );
        assert_eq!(
            resolve_from_registry("other-chat", &records, &discovered).unwrap(),
            Membership::Projectless
        );
        assert_eq!(
            resolve_from_registry("deleted-chat", &records, &discovered).unwrap(),
            Membership::Projectless
        );
    }

    #[test]
    fn rejects_ambiguous_or_invalid_chat_membership() {
        let records = vec![
            json!({"id":"a".repeat(64),"authenticatedChats":["chat-a"]}),
            json!({"id":"b".repeat(64),"authenticatedChats":["chat-a"]}),
        ];
        let discovered = ["a".repeat(64), "b".repeat(64)]
            .into_iter()
            .collect::<BTreeSet<_>>();
        assert!(resolve_from_registry("chat-a", &records, &discovered)
            .unwrap_err()
            .contains("multiple"));
        let too_long = "x".repeat(129);
        for invalid in ["", "../chat", too_long.as_str()] {
            assert!(resolve_from_registry(invalid, &records, &discovered).is_err());
        }
    }
}
