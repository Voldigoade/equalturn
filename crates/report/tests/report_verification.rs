use equalturn_protocol::{
    EndReason, EventBody, EventId, EventPayload, Hash32, ManifestHead, OsFamily, ParticipantEvent,
    ParticipantId, PublicKey, RelayNotice, RelayNoticeKind, RelayReceipt, SessionId,
    SessionManifest, SigningIdentity, fingerprint,
};
use equalturn_report::{
    EvidenceBundle, ParticipantSummary, RelayIdentity, RelayIdentityStatus, VerificationStatus,
    verify_report_json, verify_report_json_with_relay_key,
};

struct Fixture {
    report: EvidenceBundle,
    relay_key: PublicKey,
}

fn fixture() -> Fixture {
    let relay = SigningIdentity::from_seed([70; 32]);
    let alice = SigningIdentity::from_seed([71; 32]);
    let bob = SigningIdentity::from_seed([72; 32]);
    let session_id = SessionId::try_new_v4().expect("session ID");
    let alice_id = ParticipantId::try_new_v4().expect("alice ID");
    let bob_id = ParticipantId::try_new_v4().expect("bob ID");

    let alice_join = signed_event(
        session_id,
        alice_id,
        &alice,
        1,
        Hash32::ZERO,
        EventPayload::ParticipantJoined {
            nickname: "alice_1".to_owned(),
            public_key: alice.public_key(),
            client_version: "0.1.0".to_owned(),
            os_family: OsFamily::Linux,
            os_bucket: "6.x".to_owned(),
        },
    );
    let start = signed_event(
        session_id,
        alice_id,
        &alice,
        2,
        alice_join.event_hash().expect("alice join hash"),
        EventPayload::SessionStarted,
    );
    let bob_join = signed_event(
        session_id,
        bob_id,
        &bob,
        1,
        Hash32::ZERO,
        EventPayload::ParticipantJoined {
            nickname: "bob".to_owned(),
            public_key: bob.public_key(),
            client_version: "0.1.0".to_owned(),
            os_family: OsFamily::Windows,
            os_bucket: "11".to_owned(),
        },
    );
    let synthetic = signed_event(
        session_id,
        alice_id,
        &alice,
        3,
        start.event_hash().expect("start hash"),
        EventPayload::SyntheticAiAccessAttempt {
            target: Some("chatgpt.com".to_owned()),
        },
    );
    let end = signed_event(
        session_id,
        alice_id,
        &alice,
        4,
        synthetic.event_hash().expect("synthetic hash"),
        EventPayload::SessionEnded {
            reason: EndReason::ParticipantEnded,
        },
    );

    let events = vec![
        alice_join.clone(),
        bob_join.clone(),
        start.clone(),
        synthetic.clone(),
        end.clone(),
    ];
    let event_times = [1_000, 1_500, 2_000, 3_000, 5_000];
    let receipts = events
        .iter()
        .zip(event_times)
        .enumerate()
        .map(|(index, (event, timestamp))| {
            RelayReceipt::sign(
                session_id,
                event.body.participant_id,
                event.body.event_id,
                event.event_hash().expect("event hash"),
                u64::try_from(index + 1).expect("evidence order"),
                timestamp,
                &relay,
            )
            .expect("receipt signs")
        })
        .collect::<Vec<_>>();
    let session_end_notice = RelayNotice::sign(
        session_id,
        6,
        5_100,
        RelayNoticeKind::SessionEnded,
        None,
        0,
        0,
        &relay,
    )
    .expect("session end notice signs");

    let mut heads = vec![
        ManifestHead {
            participant_id: alice_id,
            final_sequence: 4,
            final_hash: end.event_hash().expect("end hash"),
        },
        ManifestHead {
            participant_id: bob_id,
            final_sequence: 1,
            final_hash: bob_join.event_hash().expect("bob join hash"),
        },
    ];
    heads.sort_by_key(|head| head.participant_id);
    let manifest = SessionManifest::sign(
        session_id,
        end.body.event_id,
        end.event_hash().expect("end hash"),
        6,
        2_000,
        5_000,
        heads,
        &relay,
    )
    .expect("manifest signs");

    let alice_key = alice.public_key();
    let bob_key = bob.public_key();
    let report = EvidenceBundle {
        report_schema_version: 1,
        protocol_version: 1,
        session_id,
        participants: vec![
            ParticipantSummary {
                participant_id: alice_id,
                nickname: "alice_1".to_owned(),
                public_key: alice_key,
                key_fingerprint: fingerprint(&alice_key),
                client_version: "0.1.0".to_owned(),
                os_family: OsFamily::Linux,
                os_bucket: "6.x".to_owned(),
            },
            ParticipantSummary {
                participant_id: bob_id,
                nickname: "bob".to_owned(),
                public_key: bob_key,
                key_fingerprint: fingerprint(&bob_key),
                client_version: "0.1.0".to_owned(),
                os_family: OsFamily::Windows,
                os_bucket: "11".to_owned(),
            },
        ],
        events,
        receipts,
        notices: vec![session_end_notice],
        relay: RelayIdentity {
            public_key: relay.public_key(),
            key_fingerprint: fingerprint(&relay.public_key()),
        },
        manifest: Some(manifest),
        local_only_events: Vec::new(),
    };

    Fixture {
        report,
        relay_key: relay.public_key(),
    }
}

fn signed_event(
    session_id: SessionId,
    participant_id: ParticipantId,
    identity: &SigningIdentity,
    sequence: u64,
    previous_hash: Hash32,
    payload: EventPayload,
) -> ParticipantEvent {
    ParticipantEvent::sign(
        EventBody {
            protocol_version: 1,
            session_id,
            participant_id,
            event_id: EventId::try_new_v4().expect("event ID"),
            sequence,
            previous_hash,
            client_timestamp_ms: 1_700_000_000_000 + sequence,
            payload,
        },
        identity,
    )
    .expect("event signs")
}

#[test]
fn complete_report_verifies_but_relay_identity_is_unanchored_without_pin() {
    let fixture = fixture();
    let result = fixture.report.verify();

    assert_eq!(result.status, VerificationStatus::Valid);
    assert_eq!(result.relay_identity, RelayIdentityStatus::Unanchored);
    assert!(result.findings.is_empty());
    assert_eq!(
        fixture
            .report
            .verify_with_relay_key(&fixture.relay_key)
            .status,
        VerificationStatus::Valid
    );
}

#[test]
fn event_mutation_invalidates_the_report() {
    let mut fixture = fixture();
    let synthetic = fixture
        .report
        .events
        .iter_mut()
        .find(|event| {
            matches!(
                event.body.payload,
                EventPayload::SyntheticAiAccessAttempt { .. }
            )
        })
        .expect("synthetic event");
    synthetic.body.payload = EventPayload::SyntheticAiAccessAttempt {
        target: Some("example.com".to_owned()),
    };

    assert_eq!(fixture.report.verify().status, VerificationStatus::Invalid);
}

#[test]
fn duplicate_event_ids_make_a_report_invalid() {
    let mut fixture = fixture();
    fixture.report.events.push(fixture.report.events[0].clone());

    let result = fixture.report.verify();
    assert_eq!(result.status, VerificationStatus::Invalid);
    assert!(
        result
            .findings
            .iter()
            .any(|finding| finding.code.as_str() == "duplicate_event_id")
    );
}

#[test]
fn receipt_notice_and_manifest_mutations_are_invalid() {
    let mut receipt_fixture = fixture();
    receipt_fixture.report.receipts[0].relay_timestamp_ms += 1;
    assert_eq!(
        receipt_fixture.report.verify().status,
        VerificationStatus::Invalid
    );

    let mut notice_fixture = fixture();
    notice_fixture.report.notices[0].relay_timestamp_ms += 1;
    assert_eq!(
        notice_fixture.report.verify().status,
        VerificationStatus::Invalid
    );

    let mut manifest_fixture = fixture();
    manifest_fixture
        .report
        .manifest
        .as_mut()
        .expect("manifest")
        .end_relay_timestamp_ms += 1;
    assert_eq!(
        manifest_fixture.report.verify().status,
        VerificationStatus::Invalid
    );
}

#[test]
fn valid_relay_signatures_cannot_reorder_a_participants_event_receipts() {
    let mut fixture = fixture();
    let relay = SigningIdentity::from_seed([70; 32]);
    let start_id = fixture
        .report
        .events
        .iter()
        .find(|event| matches!(event.body.payload, EventPayload::SessionStarted))
        .expect("session start event")
        .body
        .event_id;
    let synthetic_id = fixture
        .report
        .events
        .iter()
        .find(|event| {
            matches!(
                event.body.payload,
                EventPayload::SyntheticAiAccessAttempt { .. }
            )
        })
        .expect("synthetic event")
        .body
        .event_id;
    for receipt in &mut fixture.report.receipts {
        let evidence_order = match receipt.event_id {
            id if id == start_id => 4,
            id if id == synthetic_id => 3,
            _ => continue,
        };
        *receipt = RelayReceipt::sign(
            receipt.session_id,
            receipt.participant_id,
            receipt.event_id,
            receipt.event_hash,
            evidence_order,
            receipt.relay_timestamp_ms,
            &relay,
        )
        .expect("relay can sign the reordered receipt");
    }

    let result = fixture.report.verify();
    assert_eq!(result.status, VerificationStatus::Invalid);
    assert!(
        result
            .findings
            .iter()
            .any(|finding| { finding.code.as_str() == "event_receipt_sequence_order_mismatch" })
    );
}

#[test]
fn supplied_relay_pin_must_match_the_report_key() {
    let fixture = fixture();
    let wrong_key = SigningIdentity::from_seed([73; 32]).public_key();
    let result = fixture.report.verify_with_relay_key(&wrong_key);

    assert_eq!(result.status, VerificationStatus::Invalid);
    assert_eq!(result.relay_identity, RelayIdentityStatus::Mismatched);
    assert_eq!(
        verify_report_json_with_relay_key(
            &fixture.report.to_json_pretty().expect("report JSON"),
            &wrong_key
        )
        .status,
        VerificationStatus::Invalid
    );
}

#[test]
fn missing_interior_event_is_valid_with_gaps() {
    let mut fixture = fixture();
    fixture.report.events.retain(|event| {
        !matches!(
            event.body.payload,
            EventPayload::SyntheticAiAccessAttempt { .. }
        )
    });

    let result = fixture.report.verify();
    assert_eq!(result.status, VerificationStatus::ValidWithGaps);
    assert!(
        result
            .findings
            .iter()
            .any(|finding| finding.code.as_str() == "missing_participant_sequence")
    );
}

#[test]
fn missing_evidence_order_and_local_only_event_are_reported_as_gaps() {
    let mut fixture = fixture();
    fixture
        .report
        .receipts
        .retain(|receipt| receipt.evidence_order != 4);
    let alice = SigningIdentity::from_seed([71; 32]);
    let alice_id = fixture.report.participants[0].participant_id;
    let accepted_head = fixture
        .report
        .events
        .iter()
        .filter(|event| event.body.participant_id == alice_id)
        .max_by_key(|event| event.body.sequence)
        .expect("accepted head");
    let local_only = signed_event(
        fixture.report.session_id,
        alice_id,
        &alice,
        5,
        accepted_head.event_hash().expect("end hash"),
        EventPayload::SyntheticAiAccessAttempt { target: None },
    );
    fixture.report.local_only_events.push(local_only);

    let result = fixture.report.verify();
    assert_eq!(result.status, VerificationStatus::ValidWithGaps);
    assert!(
        result
            .findings
            .iter()
            .any(|finding| finding.code.as_str() == "missing_evidence_order")
    );
    assert!(
        result
            .findings
            .iter()
            .any(|finding| finding.code.as_str() == "local_only_unreceipted_event")
    );
}

#[test]
fn malformed_json_and_peer_address_fields_are_invalid() {
    assert_eq!(
        verify_report_json("{not json").status,
        VerificationStatus::Invalid
    );
    assert_eq!(
        verify_report_json(r#"{"report_schema_version":1,"report_schema_version":1}"#).status,
        VerificationStatus::Invalid
    );

    let fixture = fixture();
    let mut json = serde_json::to_value(&fixture.report).expect("report serializes");
    json["participants"][0]["peer_ip"] = serde_json::json!("192.0.2.1");
    assert_eq!(
        verify_report_json(&json.to_string()).status,
        VerificationStatus::Invalid
    );

    let mut json = serde_json::to_value(&fixture.report).expect("report serializes");
    json["session_id"] = serde_json::json!(
        fixture
            .report
            .session_id
            .as_uuid()
            .to_string()
            .to_uppercase()
    );
    assert_eq!(
        verify_report_json(&json.to_string()).status,
        VerificationStatus::Invalid
    );
}

#[test]
fn json_and_markdown_do_not_emit_peer_addresses_or_local_identifiers() {
    let fixture = fixture();
    let json = fixture.report.to_json_pretty().expect("report serializes");
    let markdown = fixture.report.render_markdown();

    assert!(!json.contains("peer_ip"));
    assert!(!json.contains("192.0.2.1"));
    assert!(!json.contains("hostname"));
    assert!(!markdown.to_ascii_lowercase().contains("peer ip"));
    assert!(!markdown.contains("192.0.2.1"));
    assert!(markdown.contains("Synthetic AI access attempt"));
    assert!(markdown.contains("Relay identity: unanchored"));
}
