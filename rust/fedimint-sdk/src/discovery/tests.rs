use super::*;

const CODE: &str = "fed11qgqpqrnhwden5te0vehk7tnzv9ez7qqpyq4z52329g4z52329g4z52329g4z52329g4z52329g4z52329g4z5wa8phk";
const CODE_FEDERATION_ID: &str = "2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a";

#[tokio::test]
async fn test_discover_federations_dev() {
    let feds = discover_federations(DiscoverySource::Dev)
        .await
        .expect("dev mode should succeed");
    assert_eq!(feds.len(), 1);
    assert_eq!(
        feds[0].id,
        CODE_FEDERATION_ID.parse::<FederationId>().unwrap()
    );
    assert_eq!(
        feds[0].metadata.get("federation_name").unwrap(),
        "Dev Test Federation"
    );
}

#[tokio::test]
async fn test_discover_federations_empty_relays_fails() {
    let res = discover_federations(DiscoverySource::Nostr { relays: vec![] }).await;
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().code, ErrorCode::InvalidInput);
}

#[test]
fn test_parse_relay_event_message() {
    let raw_msg = format!(
        r#"["EVENT", "{SUB_ID}", {{
            "kind": 38173,
            "pubkey": "pubkey123",
            "content": "{{\"name\": \"Community Fed\"}}",
            "tags": [
                ["d", "{CODE_FEDERATION_ID}"],
                ["u", "{CODE}"]
            ]
        }}]"#
    );
    match parse_relay_message(&raw_msg) {
        RelayAction::Event(ev) => {
            assert_eq!(ev.kind, Some(38173));
            assert_eq!(ev.pubkey.as_deref(), Some("pubkey123"));
        }
        other => panic!("expected RelayAction::Event, got {other:?}"),
    }
}

#[test]
fn test_parse_relay_eose_message() {
    let raw_msg = format!(r#"["EOSE", "{SUB_ID}"]"#);
    assert_eq!(parse_relay_message(&raw_msg), RelayAction::Eose);
}

#[test]
fn test_parse_relay_ignore_message() {
    let notice_msg = r#"["NOTICE", "rate-limited"]"#;
    assert_eq!(parse_relay_message(notice_msg), RelayAction::Ignore);

    let garbage_msg = r#"not json"#;
    assert_eq!(parse_relay_message(garbage_msg), RelayAction::Ignore);

    let wrong_sub_eose = r#"["EOSE", "wrong_sub_id"]"#;
    assert_eq!(parse_relay_message(wrong_sub_eose), RelayAction::Ignore);

    let bare_eose = r#"["EOSE"]"#;
    assert_eq!(parse_relay_message(bare_eose), RelayAction::Ignore);

    let wrong_sub_event = format!(
        r#"["EVENT", "wrong_sub_id", {{
            "kind": 38173,
            "pubkey": "pubkey123",
            "content": "{{\"name\": \"Community Fed\"}}",
            "tags": [
                ["d", "{CODE_FEDERATION_ID}"],
                ["u", "{CODE}"]
            ]
        }}]"#
    );
    assert_eq!(parse_relay_message(&wrong_sub_event), RelayAction::Ignore);

    let oversized_msg = format!(
        r#"["EVENT", "{SUB_ID}", {{ "content": "{}" }}]"#,
        "x".repeat(MAX_MESSAGE_BYTES + 1)
    );
    assert_eq!(parse_relay_message(&oversized_msg), RelayAction::Ignore);
}

#[test]
fn test_make_req_message_includes_limit() {
    let req_str = make_req_message();
    let val: serde_json::Value = serde_json::from_str(&req_str).unwrap();
    let arr = val.as_array().unwrap();
    assert_eq!(arr[0], "REQ");
    assert_eq!(arr[1], SUB_ID);
    assert_eq!(arr[2]["kinds"].as_array().unwrap()[0], 38173);
    assert_eq!(arr[2]["limit"], MAX_EVENTS);
}

fn make_test_raw_event(fed_id: &str, invite: &str) -> RawNip87Event {
    RawNip87Event {
        id: Some("ev1".to_string()),
        pubkey: Some("pub1".to_string()),
        created_at: Some(12345),
        kind: Some(38173),
        tags: vec![
            vec!["d".to_string(), fed_id.to_string()],
            vec!["u".to_string(), invite.to_string()],
        ],
        content: r#"{"name": "Test Fed"}"#.to_string(),
    }
}

#[test]
fn test_aggregate_discovered_events_deduplication() {
    let ev1 = make_test_raw_event(CODE_FEDERATION_ID, CODE);
    let ev2 = make_test_raw_event(CODE_FEDERATION_ID, CODE);

    let results = vec![Ok(vec![ev1]), Ok(vec![ev2])];
    let feds = aggregate_discovered_events(results).expect("aggregation should succeed");
    assert_eq!(
        feds.len(),
        1,
        "identical federations should be deduplicated"
    );
    assert_eq!(
        feds[0].id,
        CODE_FEDERATION_ID.parse::<FederationId>().unwrap()
    );
}

#[test]
fn test_aggregate_discovered_events_mixed_success_failure() {
    let ev = make_test_raw_event(CODE_FEDERATION_ID, CODE);
    let results = vec![
        Err(Error::new(
            ErrorCode::FederationUnreachable,
            "relay 1 unreachable",
        )),
        Ok(vec![ev]),
    ];
    let feds = aggregate_discovered_events(results)
        .expect("should succeed if at least one relay responds");
    assert_eq!(feds.len(), 1);
}

#[test]
fn test_aggregate_discovered_events_all_failures() {
    let results: Vec<Result<Vec<RawNip87Event>>> = vec![
        Err(Error::new(
            ErrorCode::FederationUnreachable,
            "relay 1 failed",
        )),
        Err(Error::new(
            ErrorCode::FederationUnreachable,
            "relay 2 failed",
        )),
    ];
    let err = aggregate_discovered_events(results).unwrap_err();
    assert_eq!(err.code, ErrorCode::FederationUnreachable);
}

#[test]
fn test_aggregate_discovered_events_empty_eose() {
    let results: Vec<Result<Vec<RawNip87Event>>> = vec![Ok(vec![]), Ok(vec![])];
    let feds = aggregate_discovered_events(results).expect("clean empty responses should succeed");
    assert!(feds.is_empty());
}
