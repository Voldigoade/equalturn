use equalturn_protocol::{
    EventBody, EventId, EventPayload, Hash32, OsFamily, ParticipantEvent, ParticipantId,
    ProtocolError, SessionId, SignatureBytes, SigningIdentity, validate_event_chain,
};

fn vector_body() -> EventBody {
    EventBody {
        protocol_version: 1,
        session_id: SessionId::from_bytes([
            0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x40, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
            0xee, 0xff,
        ]),
        participant_id: ParticipantId::from_bytes([
            0x10, 0x21, 0x32, 0x43, 0x54, 0x65, 0x46, 0x87, 0x98, 0xa9, 0xba, 0xcb, 0xdc, 0xed,
            0xfe, 0x0f,
        ]),
        event_id: EventId::from_bytes([
            0xff, 0xee, 0xdd, 0xcc, 0xbb, 0xaa, 0x49, 0x88, 0xb7, 0x66, 0x55, 0x44, 0x33, 0x22,
            0x11, 0x00,
        ]),
        sequence: 1,
        previous_hash: Hash32::ZERO,
        client_timestamp_ms: 1_700_000_000_000,
        payload: EventPayload::SessionStarted,
    }
}

#[test]
fn session_started_v1_signing_bytes_match_golden_vector() {
    let bytes = vector_body().signing_bytes().expect("valid vector body");
    assert_eq!(
        equalturn_protocol::encode_hex(&bytes),
        concat!(
            "457175616c7475726e006576656e7400763100",
            "0001",
            "00112233445540778899aabbccddeeff",
            "102132435465468798a9bacbdcedfe0f",
            "ffeeddccbbaa4988b766554433221100",
            "0000000000000001",
            "0000000000000000000000000000000000000000000000000000000000000000",
            "0000018bcfe56800",
            "00000001",
            "02"
        )
    );
}

#[test]
fn signed_event_verifies_and_payload_mutation_fails() {
    let identity = SigningIdentity::from_seed([7; 32]);
    let event = ParticipantEvent::sign(vector_body(), &identity).expect("event signs");
    event
        .verify_with_key(&identity.public_key())
        .expect("signature verifies");

    let mut modified = event.clone();
    modified.body.payload = EventPayload::SessionEnded {
        reason: equalturn_protocol::EndReason::ParticipantEnded,
    };
    assert!(matches!(
        modified.verify_with_key(&identity.public_key()),
        Err(ProtocolError::InvalidSignature)
    ));

    let other = SigningIdentity::from_seed([8; 32]);
    assert!(matches!(
        event.verify_with_key(&other.public_key()),
        Err(ProtocolError::InvalidSignature)
    ));
}

#[test]
fn event_json_uses_hex_for_binary_material() {
    let identity = SigningIdentity::from_seed([9; 32]);
    let event = ParticipantEvent::sign(vector_body(), &identity).expect("event signs");
    let json = serde_json::to_value(&event).expect("event serializes");
    assert!(json["signature"].as_str().is_some_and(|s| s.len() == 128));
    assert!(
        json["body"]["previous_hash"]
            .as_str()
            .is_some_and(|s| s.len() == 64)
    );
    assert!(serde_json::from_value::<ParticipantEvent>(json).is_ok());
}

#[test]
fn truncated_and_random_signature_material_is_rejected() {
    let identity = SigningIdentity::from_seed([10; 32]);
    let event = ParticipantEvent::sign(vector_body(), &identity).expect("event signs");
    let mut json = serde_json::to_value(&event).expect("event serializes");
    json["signature"] = serde_json::Value::String("00".repeat(63));
    assert!(serde_json::from_value::<ParticipantEvent>(json).is_err());

    let mut random_signature = event;
    random_signature.signature = SignatureBytes::new([0xa5; 64]);
    assert!(matches!(
        random_signature.verify_with_key(&identity.public_key()),
        Err(ProtocolError::InvalidSignature)
    ));
}

#[test]
fn chain_validator_reports_interior_and_leading_gaps() {
    let identity = SigningIdentity::from_seed([11; 32]);
    let first = ParticipantEvent::sign(vector_body(), &identity).expect("event signs");
    let second_body = EventBody {
        sequence: 3,
        event_id: EventId::try_new_v4().expect("event ID generated"),
        previous_hash: first.event_hash().expect("event hash"),
        payload: EventPayload::SyntheticAiAccessAttempt {
            target: Some("chatgpt.com".to_owned()),
        },
        ..vector_body()
    };
    let third = ParticipantEvent::sign(second_body, &identity).expect("event signs");
    let result = validate_event_chain(&[first, third], &identity.public_key())
        .expect("included events are cryptographically valid");
    assert_eq!(result.missing_sequences, vec![2]);

    let leading_gap_body = EventBody {
        sequence: 4,
        event_id: EventId::try_new_v4().expect("event ID generated"),
        previous_hash: Hash32::new([42; 32]),
        payload: EventPayload::SyntheticAiAccessAttempt { target: None },
        ..vector_body()
    };
    let leading_gap = ParticipantEvent::sign(leading_gap_body, &identity).expect("event signs");
    let result = validate_event_chain(&[leading_gap], &identity.public_key())
        .expect("a trailing/leading omission is represented as a gap");
    assert_eq!(result.missing_sequences, vec![1, 2, 3]);
}

#[test]
fn chain_rejects_wrong_previous_hash_duplicate_and_rollback() {
    let identity = SigningIdentity::from_seed([12; 32]);
    let first = ParticipantEvent::sign(vector_body(), &identity).expect("event signs");
    let mut bad_link_body = EventBody {
        sequence: 2,
        event_id: EventId::try_new_v4().expect("event ID generated"),
        previous_hash: Hash32::new([42; 32]),
        payload: EventPayload::SyntheticAiAccessAttempt { target: None },
        ..vector_body()
    };
    bad_link_body.session_id = first.body.session_id;
    bad_link_body.participant_id = first.body.participant_id;
    let bad_link = ParticipantEvent::sign(bad_link_body, &identity).expect("event signs");
    assert!(matches!(
        validate_event_chain(&[first.clone(), bad_link], &identity.public_key()),
        Err(ProtocolError::PreviousHashMismatch)
    ));
    assert!(matches!(
        validate_event_chain(&[first.clone(), first.clone()], &identity.public_key()),
        Err(ProtocolError::DuplicateEvent)
    ));

    let rollback = ParticipantEvent::sign(
        EventBody {
            sequence: 1,
            event_id: EventId::try_new_v4().expect("event ID generated"),
            ..vector_body()
        },
        &identity,
    )
    .expect("event signs");
    let later = ParticipantEvent::sign(
        EventBody {
            sequence: 2,
            event_id: EventId::try_new_v4().expect("event ID generated"),
            previous_hash: first.event_hash().expect("event hash"),
            payload: EventPayload::SyntheticAiAccessAttempt { target: None },
            ..vector_body()
        },
        &identity,
    )
    .expect("event signs");
    assert!(matches!(
        validate_event_chain(&[later, rollback], &identity.public_key()),
        Err(ProtocolError::SequenceRollback)
    ));
}

#[test]
fn unsupported_version_nil_ids_and_invalid_payload_are_rejected() {
    let mut body = vector_body();
    body.protocol_version = 2;
    assert!(matches!(
        body.signing_bytes(),
        Err(ProtocolError::UnsupportedVersion(2))
    ));

    let mut body = vector_body();
    body.event_id = EventId::NIL;
    assert!(matches!(
        body.signing_bytes(),
        Err(ProtocolError::NilIdentifier)
    ));

    let mut body = vector_body();
    body.session_id = SessionId::NIL;
    assert!(matches!(
        body.signing_bytes(),
        Err(ProtocolError::NilIdentifier)
    ));

    let mut body = vector_body();
    body.participant_id = ParticipantId::NIL;
    assert!(matches!(
        body.signing_bytes(),
        Err(ProtocolError::NilIdentifier)
    ));

    let mut body = vector_body();
    body.sequence = equalturn_protocol::MAX_SESSION_EVENTS + 1;
    assert!(matches!(
        body.signing_bytes(),
        Err(ProtocolError::InvalidSequence)
    ));

    let mut body = vector_body();
    body.payload = EventPayload::ParticipantJoined {
        nickname: "bad name".to_owned(),
        public_key: SigningIdentity::from_seed([13; 32]).public_key(),
        client_version: String::new(),
        os_family: OsFamily::Linux,
        os_bucket: String::new(),
    };
    assert!(matches!(
        body.signing_bytes(),
        Err(ProtocolError::InvalidNickname)
    ));

    let mut body = vector_body();
    body.payload = EventPayload::SyntheticAiAccessAttempt {
        target: Some("192.0.2.1".to_owned()),
    };
    assert!(matches!(
        body.signing_bytes(),
        Err(ProtocolError::InvalidTarget)
    ));
}
