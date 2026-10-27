//! Classification of read warnings relevant to Kanban calculations.
//!
//! Sources: anytype-heart core/api/v2/service/{search,list_read,object}.go
//! and any-block codec/anyblockjson/{validate,refs,dataview}.go.
//! Current v2 Issue responses omit codec codes, so older responses need
//! narrowly matched message families. A supplied code takes precedence.
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Impact {
    Informational,
    IncompleteResults,
    MissingProperties,
    ChangedFilters,
    UnresolvedIdentity,
    MetadataLoss,
    Unrecognized,
}

impl Impact {
    pub(super) fn explanation(self) -> &'static str {
        match self {
            Self::Informational => "Informational API notice",
            Self::IncompleteResults => "Anytype returned incomplete results",
            Self::MissingProperties => "Anytype omitted properties needed for the calculation",
            Self::ChangedFilters => "Anytype could not apply the requested filters exactly",
            Self::UnresolvedIdentity => "Anytype could not resolve identities needed for the calculation",
            Self::MetadataLoss => "Anytype could not represent the board metadata completely",
            Self::Unrecognized => "Anytype returned an unrecognized warning whose effect on calculation accuracy is unknown",
        }
    }
}

pub(super) fn classify(warning: &Value) -> Impact {
    let Some(message) = warning.get("message").and_then(Value::as_str).filter(|value| !value.is_empty()) else {
        return Impact::Unrecognized;
    };
    if let Some(code) = warning.get("code").filter(|value| !value.is_null()) {
        match code.as_str() {
            // Tombstones are ordinary state. Remote file bytes are not used
            // by numeric, boolean, or card-count calculations.
            Some("deleted_target" | "file_remote_ignored") => return Impact::Informational,
            Some("omitted_target") => return Impact::IncompleteResults,
            Some("unresolved_target" | "unresolved_type" | "type_identity_mismatch"
                | "folded_participants_without_space" | "folded_types_without_resolver") => return Impact::UnresolvedIdentity,
            Some("") => {}
            _ => return Impact::Unrecognized,
        }
    }
    let path = warning.get("path").and_then(Value::as_str).unwrap_or("");
    // These findings change which rows or values a calculation would use.
    if message.starts_with("space \"") && message.contains(" was skipped:")
        || message.contains("still loading") || message.contains("not fully loaded") {
        return Impact::IncompleteResults;
    }
    if path.starts_with("/fields/") && message.contains("omitted from those rows") {
        return Impact::MissingProperties;
    }
    if message.contains("the filter carrying it was ignored")
        || message.contains("a filter names no property and is dropped")
        || (filter_path(path) && message.contains("day count") && message.contains("is written instead")) {
        return Impact::ChangedFilters;
    }
    if path == "/query_source" && message.contains("names neither a type nor a property")
        || message.contains("the remaining types move up")
        || message.contains("the envelope carries one type") {
        return Impact::UnresolvedIdentity;
    }
    // Notices describe the semantics of the filter that actually ran; they
    // do not mean any data was omitted or any requested filter was ignored.
    if filter_path(path)
        && (message.starts_with("\"less\" on date \"") || message.starts_with("\"less_or_equal\" on date \""))
        && message.contains(" also matches objects with no ")
        && message.ends_with("; pair it with a \"not_empty\" leaf in an \"and\" group to exclude them") {
        return Impact::Informational;
    }
    if path == "view" && message.starts_with("query \"")
        && message.contains(" views and none was requested, so its first view ")
        && message.contains(" was applied; the others are ") {
        return Impact::Informational;
    }
    if path == "/file_remote" && message.starts_with("remote file metadata ignored:") {
        return Impact::Informational;
    }
    if (path == "/blocks" || path.starts_with("/blocks/")) && message.starts_with("block ")
        && message.ends_with(" is a transparent container: it is lifted, and the attributes on it are dropped") {
        return Impact::Informational;
    }
    if (path == "/blocks" || path.starts_with("/blocks/") || path.starts_with("/type_settings/"))
        && (message.contains("dropped") || message.contains("clamped") || message.contains("written instead")) {
        return Impact::MetadataLoss;
    }
    if (path == "/properties" || path.starts_with("/properties/"))
        && (message.contains("dropped") || message.contains("omitted")) {
        return Impact::MissingProperties;
    }
    Impact::Unrecognized
}

pub(super) fn check(
    response: &Value,
    explicit_view: bool,
    mut on_info: impl FnMut(&str),
) -> Result<(), String> {
    let Some(warnings) = response.get("warnings").filter(|value| !value.is_null()) else { return Ok(()); };
    let warnings = warnings.as_array().ok_or("Anytype returned malformed API warnings.")?;
    for warning in warnings {
        let mut impact = classify(warning);
        // A default-view notice is informational only when the caller did
        // not request a specific view. Otherwise the wrong filter scope ran.
        if explicit_view && impact == Impact::Informational && warning["path"] == "view" {
            impact = Impact::ChangedFilters;
        }
        let reason = warning.get("message").and_then(Value::as_str)
            .unwrap_or("The API did not explain the warning.");
        if impact == Impact::Informational {
            on_info(reason);
        } else {
            return Err(format!("{}. {reason}", impact.explanation()));
        }
    }
    Ok(())
}

fn filter_path(path: &str) -> bool {
    path == "/filter" || path.starts_with("/filters/")
}

pub(super) fn contains_kanban(document: &Value) -> bool {
    document["blocks"].as_array().into_iter().flatten().any(|block| {
        block["views"].as_array().into_iter().flatten().any(|view| {
            view["type"] == "kanban" || view["layout"] == "kanban"
        })
    })
}

// Discovery reads ordinary pages to find embedded boards. Their unrelated
// content is not a calculation input. Apply strict checks only after finding
// a board, and exclude warnings confined to prose, mentions, icons or covers.
pub(super) fn check_board_document(
    document: &Value,
    on_info: impl FnMut(&str),
) -> Result<(), String> {
    if !contains_kanban(document) { return Ok(()); }
    let Some(raw) = document.get("warnings").filter(|value| !value.is_null()) else { return Ok(()); };
    let warnings = raw.as_array().ok_or("Anytype returned malformed API warnings.")?;
    let relevant = warnings.iter().filter(|warning| {
        let path = warning.get("path").and_then(Value::as_str).unwrap_or("");
        let text_content = path.starts_with("/blocks/") && path.split('/').nth(3) == Some("text");
        !(text_content || path == "/cover" || path.starts_with("/cover/")
            || path == "/icon" || path.starts_with("/icon/"))
    }).cloned().collect::<Vec<_>>();
    check(&serde_json::json!({"warnings":relevant}), false, on_info)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn unrelated_documents_and_text_mentions_do_not_make_discovery_incomplete() {
        let removed_type = json!({"path":"/type","message":"the type \"weekly_planner\" of this object was removed from the space"});
        let mention = json!({"path":"/blocks/text-id/text/marks/0/param","code":"unresolved_target","message":"mention target names no object in this space"});
        let mut notices = Vec::new();
        for warning in [removed_type.clone(), mention.clone()] {
            check_board_document(&json!({"blocks":[{"id":"text-id","type":"text"}],"warnings":[warning]}), |message| notices.push(message.to_string())).unwrap();
        }
        assert!(notices.is_empty());
        let board = json!({"id":"board","views":[{"id":"view","type":"kanban"}]});
        check_board_document(&json!({"blocks":[board.clone()],"warnings":[mention]}), |_| {}).unwrap();
        // Source identity and view-definition losses still block real boards.
        assert!(check_board_document(&json!({"blocks":[board.clone()],"warnings":[removed_type]}), |_| {}).is_err());
        assert!(check_board_document(&json!({"blocks":[board],"warnings":[{"path":"/blocks/board/views/0/filters/0","code":"unresolved_target","message":"A filter reference could not be resolved."}]}), |_| {}).is_err());
    }

    #[test]
    fn classifies_documented_warning_families_by_accuracy_impact() {
        let cases = [
            (json!({"path":"view","message":"query \"q\" has 2 views and none was requested, so its first view \"All\" (a) was applied; the others are \"Today\" (b)"}), Impact::Informational),
            (json!({"path":"/file_remote","message":"remote file metadata ignored: unsupported version"}), Impact::Informational),
            (json!({"path":"/blocks","message":"block abc is a transparent container: it is lifted, and the attributes on it are dropped"}), Impact::Informational),
            (json!({"code":"deleted_target","message":"A referenced object was deleted."}), Impact::Informational),
            (json!({"code":"file_remote_ignored","message":"Remote metadata unavailable."}), Impact::Informational),
            (json!({"message":"space \"Work\" was skipped: store unavailable"}), Impact::IncompleteResults),
            (json!({"message":"Some stores are still loading."}), Impact::IncompleteResults),
            (json!({"code":"omitted_target","message":"A live target was omitted."}), Impact::IncompleteResults),
            (json!({"path":"/fields/0","message":"field \"points\" is not a property of space \"s\" — omitted from those rows"}), Impact::MissingProperties),
            (json!({"path":"view","message":"the current-user placeholder could not be resolved — the filter carrying it was ignored"}), Impact::ChangedFilters),
            (json!({"path":"/filters/0","message":"a filter names no property and is dropped; a filter has to name the property it filters on"}), Impact::ChangedFilters),
            (json!({"path":"/filters/0","message":"the number_of_days_ago filter carries an invalid day count; 0 is written instead"}), Impact::ChangedFilters),
            (json!({"code":"unresolved_target","message":"A reference is absent from the space."}), Impact::UnresolvedIdentity),
            (json!({"code":"unresolved_type","message":"The type cannot be resolved."}), Impact::UnresolvedIdentity),
            (json!({"code":"type_identity_mismatch","message":"Type IDs disagree."}), Impact::UnresolvedIdentity),
            (json!({"path":"/blocks/0","message":"An unsupported block was dropped."}), Impact::MetadataLoss),
            (json!({"path":"/properties/points","message":"The value was dropped."}), Impact::MissingProperties),
            (json!({"path":"/query_source","message":"query source \"id\" names neither a type nor a property this export can resolve"}), Impact::UnresolvedIdentity),
            (json!({"code":"new_future_code","message":"Something changed."}), Impact::Unrecognized),
            (json!({"message":"A new warning."}), Impact::Unrecognized),
            (json!({}), Impact::Unrecognized),
        ];
        for (warning, expected) in cases {
            assert_eq!(classify(&warning), expected, "{warning}");
        }
    }

    #[test]
    fn compact_and_structured_date_notices_are_informational_but_codes_take_precedence() {
        let message = "\"less_or_equal\" on date \"dueDate\" also matches objects with no dueDate; pair it with a \"not_empty\" leaf in an \"and\" group to exclude them";
        for path in ["/filter", "/filters/0", "/filters/0/filters/1"] {
            assert_eq!(classify(&json!({"path":path,"message":message})), Impact::Informational);
        }
        assert_eq!(classify(&json!({"path":"/filters/0","message":message,"code":"unresolved_target"})), Impact::UnresolvedIdentity);
        assert_eq!(classify(&json!({"path":"/filters/0","message":message,"code":"new_code"})), Impact::Unrecognized);
        assert_eq!(classify(&json!({"path":"/other","message":message})), Impact::Unrecognized);
    }

    #[test]
    fn default_view_notices_require_the_correct_request_context() {
        let response = json!({"warnings":[{"path":"view","message":"query \"q\" has 2 views and none was requested, so its first view \"All\" (a) was applied; the others are \"Today\" (b)"}]});
        let mut info = Vec::new();
        check(&response, false, |message| info.push(message.to_string())).unwrap();
        assert_eq!(info.len(), 1);
        assert!(check(&response, true, |_| {}).unwrap_err().contains("requested filters"));
        for invalid in [json!({"warnings":{}}), json!({"warnings":[null]}), json!({"warnings":[{"code":"deleted_target"}]})] {
            assert!(check(&invalid, false, |_| {}).is_err());
        }
    }
}
