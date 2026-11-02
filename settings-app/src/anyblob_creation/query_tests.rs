use super::{tests::*, *};
use crate::anyblob_content::tests::fixture;

#[test]
fn derived_type_sources_resolve_and_property_alternatives_do_not_depend_on_them() {
    let def = json!({"document":{"query_source":{"types":["type-internal-page"]}},"views":[{"id":"all"}]});
    let (client, _, server) = fixture(vec![
        page(json!([{"key":"page","id":"page-type"}])),
        page(json!([])),
        json!({"id":"page-type","type_internal_key":"internal-page"}),
    ]);
    assert!(query::assess(
        &client,
        &source("query", "createDelete"),
        &def,
        &req("query"),
        &|| Ok(())
    )
    .unwrap()
    .is_none());
    server.join().unwrap();
    let mut def = def;
    def["document"]["query_source"]["properties"] = json!(["status"]);
    let mut request = req("query");
    request.arguments["body"]["properties"] = json!({"status":"present"});
    let (client, capture, server) = fixture(vec![
        page(json!([{"key":"page","id":"page-type"}])),
        page(json!([{"key":"status","format":"text"}])),
    ]);
    assert!(query::assess(
        &client,
        &source("query", "createDelete"),
        &def,
        &request,
        &|| Ok(())
    )
    .unwrap()
    .is_none());
    server.join().unwrap();
    assert_eq!(capture.lock().unwrap().len(), 2);
}

fn property_doc(types: Value) -> Value {
    json!({"id":"dest","type":"set","query_source":{"types":types,"properties":["stored-status"]},"property_internal_keys":{"status":"stored-status"}})
}
fn views() -> Value {
    page(
        json!([{"id":"ready","name":"Ready tasks","filters":[{"property":"status","condition":"equal","value":"Ready"}]}]),
    )
}
fn properties() -> Value {
    page(json!([{"key":"status","name":"Status","format":"select"}]))
}
fn options() -> Value {
    page(json!([{"id":"ready-option","name":"Ready"},{"id":"blocked-option","name":"Blocked"}]))
}
fn assessment_reads() -> Vec<Value> {
    vec![
        page(json!([{"key":"page","id":"type-page"}])),
        properties(),
        options(),
    ]
}
fn property_request() -> ContentRequest {
    let mut request = req("query");
    request.arguments["body"]["properties"] = json!({"Status":"Ready"});
    request
}

#[test]
fn requests_values_without_writes_then_creates_matching_property_query_object() {
    let request = req("query");
    let p = project(source("query", "createDelete"));
    let mut responses = vec![property_doc(json!([])), views()];
    responses.extend(assessment_reads());
    let (client, capture, server) = fixture(responses);
    let answer = dispatch(
        &client,
        "guided-properties",
        &p,
        &request,
        &|| Ok(()),
        &|_, _| panic!("no mutation approval yet"),
    )
    .unwrap();
    assert_eq!(answer["outcome"], "needs_input");
    assert_eq!(answer["creation"]["outcome"], "not_attempted");
    assert_eq!(
        answer["requirements"]["source"]["properties"][0]["property"],
        "status"
    );
    assert_eq!(
        answer["requirements"]["views"][0]["filters"]["filters"][0]["required_value"],
        "Ready"
    );
    assert_eq!(
        answer["requirements"]["views"][0]["filters"]["filters"][0]["existing_options"][1]["name"],
        "Blocked"
    );
    server.join().unwrap();
    assert!(capture
        .lock()
        .unwrap()
        .iter()
        .all(|r| String::from_utf8_lossy(r).starts_with("GET ")));
    assert!(!record_path("guided-properties", &request).unwrap().exists());

    let mut corrected = request.clone();
    corrected.arguments["body"]["properties"] = json!({"Status":"Ready"});
    let mut responses = vec![property_doc(json!([])), views()];
    responses.extend(assessment_reads());
    responses.push(properties()); // ordinary custom-file inspection
    responses.extend(vec![property_doc(json!([])), views()]);
    responses.extend(assessment_reads());
    responses.push(json!({"valid":true}));
    for _ in 0..2 {
        responses.extend(vec![property_doc(json!([])), views()]);
        responses.extend(assessment_reads());
    }
    responses.push(json!({"id":"created-in-query"}));
    responses.extend(vec![
        property_doc(json!([])),
        views(),
        page(json!([{"id":"created-in-query"}])),
    ]);
    responses.extend(vec![views(), page(json!([{"id":"created-in-query"}]))]);
    responses.extend(normal_access_responses("created-in-query"));
    let (client, capture, server) = fixture(responses);
    let answer = dispatch(
        &client,
        "guided-properties",
        &p,
        &corrected,
        &|| Ok(()),
        &|_, _| Ok(()),
    )
    .unwrap();
    assert_eq!(answer["placement"]["outcome"], "confirmed");
    let scope = crate::anyblob_scope::Scope::load(&client, &p, &|| Ok(())).unwrap();
    normal_read_and_edit(&client, &scope, "created-in-query");
    assert!(!scope.readable("s", "unrelated"));
    server.join().unwrap();
    let capture = capture.lock().unwrap();
    let creates = capture
        .iter()
        .map(|r| String::from_utf8_lossy(r))
        .filter(|s| s.starts_with("POST "))
        .collect::<Vec<_>>();
    assert_eq!(creates.len(), 2); // preflight and one committed creation
    for create in creates {
        assert!(create.contains("\"Status\":\"Ready\""));
    }
}

#[test]
fn conflicting_values_return_choices_without_silent_replacement() {
    let mut request = property_request();
    request.arguments["body"]["properties"]["Status"] = json!("Blocked");
    let original = request.arguments.clone();
    let mut responses = vec![property_doc(json!([])), views()];
    responses.extend(assessment_reads());
    let (client, capture, server) = fixture(responses);
    let answer = dispatch(
        &client,
        "conflict-values",
        &project(source("query", "createDelete")),
        &request,
        &|| Ok(()),
        &|_, _| Ok(()),
    )
    .unwrap();
    assert_eq!(answer["outcome"], "needs_input");
    assert_eq!(request.arguments, original);
    assert_eq!(
        answer["requirements"]["source"]["properties"][0]["supplied"],
        true
    );
    assert_eq!(answer["requirements"]["views"][0]["satisfied"], false);
    server.join().unwrap();
    assert!(capture
        .lock()
        .unwrap()
        .iter()
        .all(|r| !String::from_utf8_lossy(r).starts_with("POST ")));
}

#[test]
fn property_presence_accepts_empty_false_zero_and_null_but_not_omission() {
    for value in [json!(""), json!(false), json!(0), Value::Null, json!([])] {
        let mut request = req("query");
        request.arguments["body"]["properties"] = json!({"status":value});
        let (client, _, server) = fixture(vec![
            page(json!([{"key":"page","id":"page-type"}])),
            page(json!([{"key":"status","format":"text"}])),
        ]);
        assert!(query::assess(
            &client,
            &source("query", "createDelete"),
            &json!({"document":{"query_source":{"properties":["status"]}},"views":[{"id":"all"}]}),
            &request,
            &|| Ok(())
        )
        .unwrap()
        .is_none());
        server.join().unwrap();
    }
    let (client, _, server) = fixture(vec![
        page(json!([{"key":"page","id":"page-type"}])),
        page(json!([{"key":"status","format":"text"}])),
    ]);
    let answer = query::assess(
        &client,
        &source("query", "createDelete"),
        &json!({"document":{"query_source":{"properties":["status"]}},"views":[{"id":"all"}]}),
        &req("query"),
        &|| Ok(()),
    )
    .unwrap()
    .unwrap();
    assert_eq!(answer["outcome"], "needs_input");
    server.join().unwrap();
}

#[test]
fn source_types_and_properties_are_alternatives_while_views_still_apply() {
    for (matching_type, matching_property) in
        [(true, false), (false, true), (true, true), (false, false)]
    {
        let mut request = req("query");
        if matching_property {
            request.arguments["body"]["properties"] = json!({"status":"x"});
        }
        let def = json!({"document":{"query_source":{"types":[if matching_type {"page"} else {"task"}],"properties":["status"]}},"views":[{"id":"all"}]});
        let (client, _, server) = fixture(vec![
            page(json!([{"key":"page","id":"page-type"}])),
            page(json!([{"key":"status","format":"text"}])),
        ]);
        let answer = query::assess(
            &client,
            &source("query", "createDelete"),
            &def,
            &request,
            &|| Ok(()),
        )
        .unwrap();
        assert_eq!(answer.is_none(), matching_type || matching_property);
        server.join().unwrap();
    }
}

#[test]
fn requirements_preserve_nested_alternatives_and_distinguish_unsupported_views() {
    for has_supported_branch in [true, false] {
        let mut request = req("query");
        request.arguments["body"]["properties"] = json!({"status":"Blocked"});
        let filters = json!([{"operator":"or","filters":[{"property":"status","condition":"contains","value":"unknown"},{"operator":"and","filters":[{"property":"status","condition":if has_supported_branch {"equal"} else {"not_equal"},"value":"Ready"}]}]}]);
        let def = json!({"document":{"query_source":{"properties":["status"]}},"views":[{"id":"choices","filters":filters}]});
        let mut responses = assessment_reads();
        let (client, _, server) = fixture(std::mem::take(&mut responses));
        let answer = query::assess(
            &client,
            &source("query", "createDelete"),
            &def,
            &request,
            &|| Ok(()),
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            answer["outcome"],
            if has_supported_branch {
                "needs_input"
            } else {
                "unsupported_query"
            }
        );
        assert_eq!(
            answer["requirements"]["views"][0]["filters"]["filters"][0]["operator"],
            "or"
        );
        server.join().unwrap();
    }
}

#[test]
fn discovery_explains_property_sources_options_and_unsupported_queries() {
    for supported in [true, false] {
        let view = if supported {
            views()
        } else {
            page(
                json!([{"id":"all","filters":[{"property":"status","condition":"contains","value":"Ready"}]}]),
            )
        };
        let (client, capture, server) =
            fixture(vec![property_doc(json!([])), view, properties(), options()]);
        let request = ContentRequest {
            tool: "list_creation_targets".into(),
            arguments: json!({"space_id":"s"}),
        };
        let answer = dispatch(
            &client,
            "targets",
            &project(source("query", "createDelete")),
            &request,
            &|| Ok(()),
            &|_, _| Ok(()),
        )
        .unwrap();
        let requirements = &answer["targets"][0]["requirements"];
        assert_eq!(
            requirements["status"],
            if supported {
                "supported"
            } else {
                "unsupported"
            }
        );
        assert_eq!(
            requirements["source"]["properties"][0]["property"],
            "status"
        );
        server.join().unwrap();
        assert_eq!(capture.lock().unwrap().len(), 4);
    }
}

#[test]
fn internal_property_keys_and_ambiguous_aliases_are_not_confused() {
    let properties = vec![
        json!({"key":"status","name":"Status","format":"select","internal_key":"stored-status"}),
        json!({"key":"another","name":"Status","format":"text"}),
    ];
    assert_eq!(
        query::property("stored-status", &properties).unwrap()["key"],
        "status"
    );
    assert!(query::property("Status", &properties).is_err());
    let body = json!({"properties":{"status":"Ready","stored-status":"Blocked"}});
    assert!(query::supplied(&properties[0], &body, &properties).is_err());
    let mut request = req("query");
    request.arguments["body"]["properties"] = json!({"stored-status":"Ready"});
    let def = json!({"document":{"query_source":{"properties":["stored-status"]},"property_internal_keys":{"status":"stored-status"}},"views":[{"id":"all"}]});
    let (client, _, server) = fixture(assessment_reads());
    assert!(query::assess(
        &client,
        &source("query", "createDelete"),
        &def,
        &request,
        &|| Ok(())
    )
    .unwrap()
    .is_none());
    server.join().unwrap();
}

#[test]
fn no_input_guidance_bypasses_permissions_files_or_read_approval() {
    let request = req("query");
    let client = Client::with_connection("fixture", "127.0.0.1", 1);
    for access in ["none", "read", "readWrite"] {
        assert!(dispatch(
            &client,
            "denied",
            &project(source("query", access)),
            &request,
            &|| Ok(()),
            &|_, _| panic!("denied")
        )
        .is_err());
    }
    let mut s = source("query", "createDelete");
    s.permission.approval = "alwaysAsk".into();
    let mut responses = vec![property_doc(json!([])), views()];
    responses.extend(assessment_reads());
    let (client, capture, server) = fixture(responses);
    assert!(dispatch(
        &client,
        "denied",
        &project(s),
        &request,
        &|| Ok(()),
        &|_, _| Err("User denied requirements read".into())
    )
    .unwrap_err()
    .contains("denied"));
    server.join().unwrap();
    assert!(capture
        .lock()
        .unwrap()
        .iter()
        .all(|r| String::from_utf8_lossy(r).starts_with("GET ")));
    let mut request = req("query");
    request.arguments["body"]["properties"] = json!({"stored-file":"file-id"});
    let def = json!({"document":{"query_source":{"properties":["stored-file"]},"property_internal_keys":{"attachment":"stored-file"}},"views":[{"id":"all"}]});
    let (client, _, server) = fixture(vec![
        page(json!([{"key":"page","id":"page-type"}])),
        page(json!([{"key":"attachment","format":"files"}])),
    ]);
    assert!(query::assess(
        &client,
        &source("query", "createDelete"),
        &def,
        &request,
        &|| Ok(())
    )
    .unwrap_err()
    .contains("file permission"));
    server.join().unwrap();
}

#[test]
fn guided_followup_reloads_views_and_bound_keys_cannot_change_payloads() {
    let request = property_request();
    let p = project(source("query", "createDelete"));
    let changed_views = page(
        json!([{"id":"ready","filters":[{"property":"status","condition":"equal","value":"Blocked"}]}]),
    );
    let mut responses = vec![property_doc(json!([])), changed_views];
    responses.extend(assessment_reads());
    let (client, capture, server) = fixture(responses);
    let answer = dispatch(
        &client,
        "changed-guidance",
        &p,
        &request,
        &|| Ok(()),
        &|_, _| Ok(()),
    )
    .unwrap();
    assert_eq!(answer["outcome"], "needs_input");
    assert_eq!(
        answer["requirements"]["views"][0]["filters"]["filters"][0]["required_value"],
        "Blocked"
    );
    server.join().unwrap();
    assert_eq!(capture.lock().unwrap().len(), 5);
    let path = record_path("bound-guidance", &request).unwrap();
    save(
        &path,
        &json!({"state":"creating","fingerprint":"different-payload"}),
    )
    .unwrap();
    let mut responses = vec![
        property_doc(json!([])),
        page(
            json!([{"id":"all","filters":[{"property":"status","condition":"equal","value":"Blocked"}]}]),
        ),
    ];
    responses.extend(assessment_reads());
    let (client, _, server) = fixture(responses);
    assert!(dispatch(
        &client,
        "bound-guidance",
        &p,
        &request,
        &|| Ok(()),
        &|_, _| Ok(())
    )
    .unwrap_err()
    .contains("different payload"));
    server.join().unwrap();
}
