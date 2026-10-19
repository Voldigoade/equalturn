use std::collections::{HashMap, HashSet};

use ed25519_dalek::VerifyingKey;
use equalturn_protocol::{
    EventPayload, ParticipantEvent, ParticipantId, PublicKey, RelayNoticeKind, fingerprint,
    validate_event_chain_to_head,
};
use serde::{Deserialize, Serialize};

use crate::model::{
    EvidenceBundle, MAX_REPORT_EVENTS, MAX_REPORT_EVIDENCE_ORDER, MAX_REPORT_LOCAL_EVENTS,
    MAX_REPORT_NOTICES, MAX_REPORT_PARTICIPANTS, MAX_REPORT_RECEIPTS, ParticipantSummary,
    valid_client_metadata, valid_event_id, valid_id, valid_nickname, valid_notice_id,
    valid_participant_id,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VerificationStatus {
    Valid,
    ValidWithGaps,
    Invalid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelayIdentityStatus {
    Anchored,
    Unanchored,
    Mismatched,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingCode {
    MalformedReport,
    UnsupportedReportVersion,
    UnsupportedProtocolVersion,
    InvalidSessionId,
    InvalidRelayKey,
    RelayKeyFingerprintMismatch,
    RelayKeyMismatch,
    InvalidManifest,
    ManifestSessionMismatch,
    MissingManifest,
    TooManyReportItems,
    InvalidParticipant,
    DuplicateParticipant,
    DuplicateNickname,
    ParticipantNotInManifest,
    ParticipantMissingFromReport,
    MissingJoinEvent,
    JoinEventMismatch,
    UnknownEventParticipant,
    InvalidEvent,
    DuplicateEventId,
    InvalidEventChain,
    MissingParticipantSequence,
    MissingReceiptForEvent,
    MissingEventForReceipt,
    InvalidReceipt,
    EventReceiptSequenceOrderMismatch,
    ReceiptMismatch,
    DuplicateReceipt,
    UnknownNoticeParticipant,
    InvalidNotice,
    DuplicateNotice,
    DuplicateEvidenceOrder,
    MissingEvidenceOrder,
    EvidenceOrderBeyondManifest,
    MissingSessionStart,
    InvalidSessionStart,
    MissingSessionEndNotice,
    DuplicateSessionEndNotice,
    ManifestEndEventMissing,
    ManifestEndEventMismatch,
    LocalOnlyUnreceiptedEvent,
}

impl FindingCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::MalformedReport => "malformed_report",
            Self::UnsupportedReportVersion => "unsupported_report_version",
            Self::UnsupportedProtocolVersion => "unsupported_protocol_version",
            Self::InvalidSessionId => "invalid_session_id",
            Self::InvalidRelayKey => "invalid_relay_key",
            Self::RelayKeyFingerprintMismatch => "relay_key_fingerprint_mismatch",
            Self::RelayKeyMismatch => "relay_key_mismatch",
            Self::InvalidManifest => "invalid_manifest",
            Self::ManifestSessionMismatch => "manifest_session_mismatch",
            Self::MissingManifest => "missing_manifest",
            Self::TooManyReportItems => "too_many_report_items",
            Self::InvalidParticipant => "invalid_participant",
            Self::DuplicateParticipant => "duplicate_participant",
            Self::DuplicateNickname => "duplicate_nickname",
            Self::ParticipantNotInManifest => "participant_not_in_manifest",
            Self::ParticipantMissingFromReport => "participant_missing_from_report",
            Self::MissingJoinEvent => "missing_join_event",
            Self::JoinEventMismatch => "join_event_mismatch",
            Self::UnknownEventParticipant => "unknown_event_participant",
            Self::InvalidEvent => "invalid_event",
            Self::DuplicateEventId => "duplicate_event_id",
            Self::InvalidEventChain => "invalid_event_chain",
            Self::MissingParticipantSequence => "missing_participant_sequence",
            Self::MissingReceiptForEvent => "missing_receipt_for_event",
            Self::MissingEventForReceipt => "missing_event_for_receipt",
            Self::InvalidReceipt => "invalid_receipt",
            Self::EventReceiptSequenceOrderMismatch => "event_receipt_sequence_order_mismatch",
            Self::ReceiptMismatch => "receipt_mismatch",
            Self::DuplicateReceipt => "duplicate_receipt",
            Self::UnknownNoticeParticipant => "unknown_notice_participant",
            Self::InvalidNotice => "invalid_notice",
            Self::DuplicateNotice => "duplicate_notice",
            Self::DuplicateEvidenceOrder => "duplicate_evidence_order",
            Self::MissingEvidenceOrder => "missing_evidence_order",
            Self::EvidenceOrderBeyondManifest => "evidence_order_beyond_manifest",
            Self::MissingSessionStart => "missing_session_start",
            Self::InvalidSessionStart => "invalid_session_start",
            Self::MissingSessionEndNotice => "missing_session_end_notice",
            Self::DuplicateSessionEndNotice => "duplicate_session_end_notice",
            Self::ManifestEndEventMissing => "manifest_end_event_missing",
            Self::ManifestEndEventMismatch => "manifest_end_event_mismatch",
            Self::LocalOnlyUnreceiptedEvent => "local_only_unreceipted_event",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    pub code: FindingCode,
    pub detail: String,
}

impl Finding {
    pub fn code_str(&self) -> &'static str {
        self.code.as_str()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationResult {
    pub status: VerificationStatus,
    pub relay_identity: RelayIdentityStatus,
    pub findings: Vec<Finding>,
}

impl VerificationResult {
    pub(crate) fn malformed() -> Self {
        Self {
            status: VerificationStatus::Invalid,
            relay_identity: RelayIdentityStatus::Unanchored,
            findings: vec![Finding {
                code: FindingCode::MalformedReport,
                detail: "report JSON could not be parsed as canonical schema version 1".into(),
            }],
        }
    }
}

pub fn verify_report_json(json: &str) -> VerificationResult {
    match EvidenceBundle::from_json(json) {
        Ok(report) => report.verify(),
        Err(_) => VerificationResult::malformed(),
    }
}

pub fn verify_report_json_with_relay_key(
    json: &str,
    expected_relay_key: &PublicKey,
) -> VerificationResult {
    match EvidenceBundle::from_json(json) {
        Ok(report) => report.verify_with_relay_key(expected_relay_key),
        Err(_) => VerificationResult::malformed(),
    }
}

pub(crate) fn verify_bundle(
    report: &EvidenceBundle,
    expected_relay_key: Option<&PublicKey>,
) -> VerificationResult {
    let mut state = VerificationState::default();
    let relay_identity = match expected_relay_key {
        Some(expected) if *expected == report.relay.public_key => RelayIdentityStatus::Anchored,
        Some(_) => RelayIdentityStatus::Mismatched,
        None => RelayIdentityStatus::Unanchored,
    };

    if report.report_schema_version != 1 {
        state.invalid(
            FindingCode::UnsupportedReportVersion,
            format!(
                "report schema version {} is unsupported",
                report.report_schema_version
            ),
        );
    }
    if report.protocol_version != 1 {
        state.invalid(
            FindingCode::UnsupportedProtocolVersion,
            format!(
                "protocol version {} is unsupported",
                report.protocol_version
            ),
        );
    }
    if !valid_id(report.session_id) {
        state.invalid(
            FindingCode::InvalidSessionId,
            "session ID is not a non-nil UUIDv4".into(),
        );
    }
    if VerifyingKey::from_bytes(report.relay.public_key.as_bytes()).is_err() {
        state.invalid(
            FindingCode::InvalidRelayKey,
            "relay public key is malformed".into(),
        );
    }
    if report.relay.key_fingerprint != fingerprint(&report.relay.public_key) {
        state.invalid(
            FindingCode::RelayKeyFingerprintMismatch,
            "relay key fingerprint does not match the included key".into(),
        );
    }
    if expected_relay_key.is_some_and(|expected| *expected != report.relay.public_key) {
        state.invalid(
            FindingCode::RelayKeyMismatch,
            "included relay key does not match the supplied trust anchor".into(),
        );
    }

    if report.participants.len() > MAX_REPORT_PARTICIPANTS
        || report.events.len() > MAX_REPORT_EVENTS
        || report.local_only_events.len() > MAX_REPORT_LOCAL_EVENTS
        || report.receipts.len() > MAX_REPORT_RECEIPTS
        || report.notices.len() > MAX_REPORT_NOTICES
    {
        state.invalid(
            FindingCode::TooManyReportItems,
            "one or more report collections exceed their v1 bounds".into(),
        );
    }

    let manifest = match &report.manifest {
        Some(manifest) => {
            if manifest.participant_heads.len() > MAX_REPORT_PARTICIPANTS {
                state.invalid(
                    FindingCode::TooManyReportItems,
                    "manifest participant count exceeds the report limit".into(),
                );
            }
            if manifest.verify().is_err() {
                state.invalid(
                    FindingCode::InvalidManifest,
                    "manifest signature or structure is invalid".into(),
                );
            }
            if !valid_id(manifest.session_id)
                || !valid_event_id(manifest.end_event_id)
                || manifest
                    .participant_heads
                    .iter()
                    .any(|head| !valid_participant_id(head.participant_id))
            {
                state.invalid(
                    FindingCode::InvalidManifest,
                    "manifest contains a malformed session, event, or participant ID".into(),
                );
            }
            if manifest.session_id != report.session_id {
                state.invalid(
                    FindingCode::ManifestSessionMismatch,
                    "manifest belongs to a different session".into(),
                );
            }
            if manifest.relay_public_key != report.relay.public_key
                || manifest.relay_key_fingerprint != report.relay.key_fingerprint
            {
                state.invalid(
                    FindingCode::InvalidManifest,
                    "manifest and report relay identities differ".into(),
                );
            }
            if manifest.highest_evidence_order > MAX_REPORT_EVIDENCE_ORDER {
                state.invalid(
                    FindingCode::TooManyReportItems,
                    "manifest evidence order exceeds the report verification bound".into(),
                );
            }
            Some(manifest)
        }
        None => {
            state.gap(
                FindingCode::MissingManifest,
                "no relay-signed final manifest is included".into(),
            );
            None
        }
    };

    let participants = validate_participants(report, manifest, &mut state);
    let mut accepted_events_by_id = HashMap::with_capacity(report.events.len());
    let mut local_events_by_id = HashMap::with_capacity(report.local_only_events.len());
    let mut all_event_ids = HashSet::with_capacity(
        report
            .events
            .len()
            .saturating_add(report.local_only_events.len()),
    );
    let mut accepted_by_participant: HashMap<ParticipantId, Vec<&ParticipantEvent>> =
        HashMap::with_capacity(participants.len());
    let mut all_by_participant: HashMap<ParticipantId, Vec<&ParticipantEvent>> =
        HashMap::with_capacity(participants.len());

    for event in &report.events {
        validate_event(event, report, &participants, &mut state, &mut all_event_ids);
        accepted_events_by_id.insert(event.body.event_id, event);
        accepted_by_participant
            .entry(event.body.participant_id)
            .or_default()
            .push(event);
        all_by_participant
            .entry(event.body.participant_id)
            .or_default()
            .push(event);
    }
    for event in &report.local_only_events {
        validate_event(event, report, &participants, &mut state, &mut all_event_ids);
        local_events_by_id.insert(event.body.event_id, event);
        all_by_participant
            .entry(event.body.participant_id)
            .or_default()
            .push(event);
        state.gap(
            FindingCode::LocalOnlyUnreceiptedEvent,
            format!(
                "participant {} has an event the relay did not acknowledge",
                event.body.participant_id.as_uuid()
            ),
        );
    }

    validate_join_bindings(report, &participants, &mut state);
    validate_chains(
        report,
        manifest,
        &participants,
        &accepted_by_participant,
        &all_by_participant,
        &mut state,
    );

    let mut receipts_by_event = HashMap::with_capacity(report.receipts.len());
    let mut evidence_orders = HashMap::<u64, &'static str>::with_capacity(
        report.receipts.len().saturating_add(report.notices.len()),
    );
    for receipt in &report.receipts {
        if receipt.verify(&report.relay.public_key).is_err()
            || receipt.session_id != report.session_id
            || !valid_id(receipt.session_id)
            || !valid_participant_id(receipt.participant_id)
            || !valid_event_id(receipt.event_id)
        {
            state.invalid(
                FindingCode::InvalidReceipt,
                format!(
                    "receipt for event {} is invalid",
                    receipt.event_id.as_uuid()
                ),
            );
            continue;
        }
        if !participants.contains_key(&receipt.participant_id) {
            state.invalid(
                FindingCode::InvalidReceipt,
                format!(
                    "receipt references unknown participant {}",
                    receipt.participant_id.as_uuid()
                ),
            );
        }
        if receipts_by_event
            .insert(receipt.event_id, receipt)
            .is_some()
        {
            state.invalid(
                FindingCode::DuplicateReceipt,
                format!(
                    "event {} has more than one receipt",
                    receipt.event_id.as_uuid()
                ),
            );
        }
        insert_evidence_order(
            &mut evidence_orders,
            receipt.evidence_order,
            "receipt",
            &mut state,
        );
        if let Some(event) = accepted_events_by_id.get(&receipt.event_id) {
            let hash_matches = event
                .event_hash()
                .is_ok_and(|event_hash| receipt.event_hash == event_hash);
            if receipt.participant_id != event.body.participant_id || !hash_matches {
                state.invalid(
                    FindingCode::ReceiptMismatch,
                    format!("receipt does not bind event {}", receipt.event_id.as_uuid()),
                );
            }
        } else if local_events_by_id.contains_key(&receipt.event_id) {
            state.invalid(
                FindingCode::ReceiptMismatch,
                format!(
                    "event {} is labeled local-only but has a relay receipt",
                    receipt.event_id.as_uuid()
                ),
            );
        } else {
            state.gap(
                FindingCode::MissingEventForReceipt,
                format!(
                    "receipt for event {} has no event body in the report",
                    receipt.event_id.as_uuid()
                ),
            );
        }
    }
    for event in &report.events {
        if !receipts_by_event.contains_key(&event.body.event_id) {
            state.gap(
                FindingCode::MissingReceiptForEvent,
                format!(
                    "event {} has no relay receipt",
                    event.body.event_id.as_uuid()
                ),
            );
        }
    }
    for event in &report.local_only_events {
        if receipts_by_event.contains_key(&event.body.event_id) {
            state.invalid(
                FindingCode::ReceiptMismatch,
                format!(
                    "local-only event {} has a relay receipt",
                    event.body.event_id.as_uuid()
                ),
            );
        }
    }

    validate_receipt_sequence_order(&accepted_by_participant, &receipts_by_event, &mut state);
    validate_notices(report, &participants, &mut state, &mut evidence_orders);
    validate_lifecycle(
        report,
        manifest,
        &accepted_events_by_id,
        &receipts_by_event,
        &mut state,
    );
    validate_evidence_orders(manifest, &evidence_orders, &mut state);

    state.finish(relay_identity)
}

fn validate_receipt_sequence_order(
    accepted: &HashMap<ParticipantId, Vec<&ParticipantEvent>>,
    receipts: &HashMap<equalturn_protocol::EventId, &equalturn_protocol::RelayReceipt>,
    state: &mut VerificationState,
) {
    for (participant_id, events) in accepted {
        let mut ordered_events = events.clone();
        ordered_events.sort_by_key(|event| event.body.sequence);
        let mut previous_order = None;
        for event in ordered_events {
            let Some(receipt) = receipts.get(&event.body.event_id) else {
                continue;
            };
            let hash_matches = event
                .event_hash()
                .is_ok_and(|event_hash| receipt.event_hash == event_hash);
            if receipt.participant_id != *participant_id || !hash_matches {
                continue;
            }
            if previous_order.is_some_and(|previous| receipt.evidence_order <= previous) {
                state.invalid(
                    FindingCode::EventReceiptSequenceOrderMismatch,
                    format!(
                        "participant {} has relay receipts that do not increase with event sequence",
                        participant_id.as_uuid()
                    ),
                );
                break;
            }
            previous_order = Some(receipt.evidence_order);
        }
    }
}

fn validate_participants<'a>(
    report: &'a EvidenceBundle,
    manifest: Option<&equalturn_protocol::SessionManifest>,
    state: &mut VerificationState,
) -> HashMap<ParticipantId, &'a ParticipantSummary> {
    let mut participants = HashMap::with_capacity(report.participants.len());
    let mut nicknames = HashSet::with_capacity(report.participants.len());
    for participant in &report.participants {
        if !valid_participant_id(participant.participant_id)
            || !valid_nickname(&participant.nickname)
            || !valid_client_metadata(&participant.client_version, &participant.os_bucket)
            || VerifyingKey::from_bytes(participant.public_key.as_bytes()).is_err()
            || participant.key_fingerprint != fingerprint(&participant.public_key)
        {
            state.invalid(
                FindingCode::InvalidParticipant,
                format!(
                    "participant {} has invalid identity or metadata",
                    participant.participant_id.as_uuid()
                ),
            );
        }
        if participants
            .insert(participant.participant_id, participant)
            .is_some()
        {
            state.invalid(
                FindingCode::DuplicateParticipant,
                format!(
                    "participant {} appears more than once",
                    participant.participant_id.as_uuid()
                ),
            );
        }
        if !nicknames.insert(participant.nickname.to_ascii_lowercase()) {
            state.invalid(
                FindingCode::DuplicateNickname,
                format!(
                    "nickname {} is duplicated case-insensitively",
                    participant.nickname
                ),
            );
        }
    }

    if let Some(manifest) = manifest {
        let head_ids: HashSet<_> = manifest
            .participant_heads
            .iter()
            .map(|head| head.participant_id)
            .collect();
        for participant_id in participants.keys() {
            if !head_ids.contains(participant_id) {
                state.invalid(
                    FindingCode::ParticipantNotInManifest,
                    format!(
                        "participant {} is absent from the signed manifest",
                        participant_id.as_uuid()
                    ),
                );
            }
        }
        for participant_id in head_ids {
            if !participants.contains_key(&participant_id) {
                state.gap(
                    FindingCode::ParticipantMissingFromReport,
                    format!(
                        "manifest participant {} has no report profile",
                        participant_id.as_uuid()
                    ),
                );
            }
        }
    }
    participants
}

fn validate_event(
    event: &ParticipantEvent,
    report: &EvidenceBundle,
    participants: &HashMap<ParticipantId, &ParticipantSummary>,
    state: &mut VerificationState,
    all_event_ids: &mut HashSet<equalturn_protocol::EventId>,
) {
    let participant = participants.get(&event.body.participant_id);
    if participant.is_none() {
        state.invalid(
            FindingCode::UnknownEventParticipant,
            format!(
                "event {} references an unknown participant",
                event.body.event_id.as_uuid()
            ),
        );
        return;
    }
    if event.body.session_id != report.session_id || !valid_id(event.body.session_id) {
        state.invalid(
            FindingCode::InvalidEvent,
            format!(
                "event {} belongs to a different or invalid session",
                event.body.event_id.as_uuid()
            ),
        );
    }
    if !valid_participant_id(event.body.participant_id) || !valid_event_id(event.body.event_id) {
        state.invalid(
            FindingCode::InvalidEvent,
            format!(
                "event {} has invalid identifiers",
                event.body.event_id.as_uuid()
            ),
        );
    }
    if !all_event_ids.insert(event.body.event_id) {
        state.invalid(
            FindingCode::DuplicateEventId,
            format!(
                "event ID {} is duplicated in the report",
                event.body.event_id.as_uuid()
            ),
        );
    }
    if let Some(participant) = participant
        && event.verify_with_key(&participant.public_key).is_err()
    {
        state.invalid(
            FindingCode::InvalidEvent,
            format!(
                "event {} signature or payload is invalid",
                event.body.event_id.as_uuid()
            ),
        );
    }
}

fn validate_join_bindings(
    report: &EvidenceBundle,
    participants: &HashMap<ParticipantId, &ParticipantSummary>,
    state: &mut VerificationState,
) {
    let mut joins = HashMap::<ParticipantId, &ParticipantEvent>::new();
    for event in report.events.iter().chain(&report.local_only_events) {
        if let EventPayload::ParticipantJoined {
            nickname,
            public_key,
            client_version,
            os_family,
            os_bucket,
        } = &event.body.payload
        {
            if event.body.sequence != 1 {
                state.invalid(
                    FindingCode::JoinEventMismatch,
                    format!(
                        "participant join event {} is not sequence 1",
                        event.body.event_id.as_uuid()
                    ),
                );
            }
            if joins.insert(event.body.participant_id, event).is_some() {
                state.invalid(
                    FindingCode::JoinEventMismatch,
                    format!(
                        "participant {} has multiple join events",
                        event.body.participant_id.as_uuid()
                    ),
                );
            }
            if let Some(participant) = participants.get(&event.body.participant_id)
                && (nickname != &participant.nickname
                    || public_key != &participant.public_key
                    || client_version != &participant.client_version
                    || os_family != &participant.os_family
                    || os_bucket != &participant.os_bucket)
            {
                state.invalid(
                    FindingCode::JoinEventMismatch,
                    format!(
                        "participant {} profile differs from its signed join event",
                        event.body.participant_id.as_uuid()
                    ),
                );
            }
        } else if event.body.sequence == 1 {
            state.invalid(
                FindingCode::JoinEventMismatch,
                format!(
                    "participant {} sequence 1 is not a join event",
                    event.body.participant_id.as_uuid()
                ),
            );
        }
    }
    for participant_id in participants.keys() {
        if !joins.contains_key(participant_id) {
            state.gap(
                FindingCode::MissingJoinEvent,
                format!(
                    "participant {} has no signed join event in the report",
                    participant_id.as_uuid()
                ),
            );
        }
    }
}

fn validate_chains(
    report: &EvidenceBundle,
    manifest: Option<&equalturn_protocol::SessionManifest>,
    participants: &HashMap<ParticipantId, &ParticipantSummary>,
    accepted: &HashMap<ParticipantId, Vec<&ParticipantEvent>>,
    all: &HashMap<ParticipantId, Vec<&ParticipantEvent>>,
    state: &mut VerificationState,
) {
    let head_by_participant: HashMap<_, _> = manifest
        .into_iter()
        .flat_map(|manifest| manifest.participant_heads.iter())
        .map(|head| (head.participant_id, (head.final_sequence, head.final_hash)))
        .collect();
    let local_event_ids: HashSet<_> = report
        .local_only_events
        .iter()
        .map(|event| event.body.event_id)
        .collect();

    for (participant_id, participant) in participants {
        let mut accepted_events = accepted.get(participant_id).cloned().unwrap_or_default();
        accepted_events.sort_by_key(|event| event.body.sequence);
        let expected_head = head_by_participant.get(participant_id).copied();
        if let Some((sequence, hash)) = expected_head {
            match validate_event_chain_to_head(
                &accepted_events.into_iter().cloned().collect::<Vec<_>>(),
                &participant.public_key,
                Some((sequence, hash)),
            ) {
                Ok(validation) => {
                    if !validation.missing_sequences.is_empty() {
                        state.gap(
                            FindingCode::MissingParticipantSequence,
                            format!(
                                "participant {} is missing accepted sequence {}",
                                participant_id.as_uuid(),
                                compact_ranges(&validation.missing_sequences)
                            ),
                        );
                    }
                }
                Err(_) => state.invalid(
                    FindingCode::InvalidEventChain,
                    format!(
                        "participant {} accepted event chain is invalid",
                        participant_id.as_uuid()
                    ),
                ),
            }
        }

        let mut chain_events = all.get(participant_id).cloned().unwrap_or_default();
        chain_events.sort_by_key(|event| event.body.sequence);
        match equalturn_protocol::validate_event_chain(
            &chain_events.into_iter().cloned().collect::<Vec<_>>(),
            &participant.public_key,
        ) {
            Ok(validation) => {
                if !validation.missing_sequences.is_empty() {
                    state.gap(
                        FindingCode::MissingParticipantSequence,
                        format!(
                            "participant {} event chain has missing sequence {}",
                            participant_id.as_uuid(),
                            compact_ranges(&validation.missing_sequences)
                        ),
                    );
                }
            }
            Err(_) => state.invalid(
                FindingCode::InvalidEventChain,
                format!(
                    "participant {} combined accepted/local chain is invalid",
                    participant_id.as_uuid()
                ),
            ),
        }
    }

    for event in report.events.iter().chain(&report.local_only_events) {
        if !participants.contains_key(&event.body.participant_id) {
            // The unknown participant is already reported as an invalid event.
            continue;
        }
        if let Some((final_sequence, _)) = head_by_participant.get(&event.body.participant_id) {
            let local_only = local_event_ids.contains(&event.body.event_id);
            if !local_only && event.body.sequence > *final_sequence {
                state.invalid(
                    FindingCode::InvalidEventChain,
                    format!(
                        "accepted event {} exceeds the manifest participant head",
                        event.body.event_id.as_uuid()
                    ),
                );
            }
            if local_only && event.body.sequence <= *final_sequence {
                state.invalid(
                    FindingCode::InvalidEventChain,
                    format!(
                        "local-only event {} conflicts with the relay manifest head",
                        event.body.event_id.as_uuid()
                    ),
                );
            }
        }
    }
}

fn validate_notices(
    report: &EvidenceBundle,
    participants: &HashMap<ParticipantId, &ParticipantSummary>,
    state: &mut VerificationState,
    evidence_orders: &mut HashMap<u64, &'static str>,
) {
    let mut notice_ids = HashSet::with_capacity(report.notices.len());
    for notice in &report.notices {
        if notice.verify(&report.relay.public_key).is_err()
            || notice.session_id != report.session_id
            || !valid_id(notice.session_id)
            || !valid_notice_id(notice.notice_id)
        {
            state.invalid(
                FindingCode::InvalidNotice,
                format!("relay notice {} is invalid", notice.notice_id.as_uuid()),
            );
        }
        if !notice_ids.insert(notice.notice_id) {
            state.invalid(
                FindingCode::DuplicateNotice,
                format!("relay notice {} is duplicated", notice.notice_id.as_uuid()),
            );
        }
        let presence_matches_kind = match notice.kind {
            RelayNoticeKind::ParticipantOnline
            | RelayNoticeKind::ParticipantStale
            | RelayNoticeKind::ParticipantDisconnected => notice.participant_id.is_some(),
            RelayNoticeKind::SessionEnded => notice.participant_id.is_none(),
        };
        if !presence_matches_kind {
            state.invalid(
                FindingCode::InvalidNotice,
                format!(
                    "relay notice {} has the wrong participant binding",
                    notice.notice_id.as_uuid()
                ),
            );
        }
        if let Some(participant_id) = notice.participant_id
            && !participants.contains_key(&participant_id)
        {
            state.invalid(
                FindingCode::UnknownNoticeParticipant,
                format!(
                    "relay notice {} references an unknown participant",
                    notice.notice_id.as_uuid()
                ),
            );
        }
        insert_evidence_order(evidence_orders, notice.evidence_order, "notice", state);
    }
}

fn validate_lifecycle(
    report: &EvidenceBundle,
    manifest: Option<&equalturn_protocol::SessionManifest>,
    events: &HashMap<equalturn_protocol::EventId, &ParticipantEvent>,
    receipts: &HashMap<equalturn_protocol::EventId, &equalturn_protocol::RelayReceipt>,
    state: &mut VerificationState,
) {
    let Some(manifest) = manifest else {
        return;
    };
    let end_event = events.get(&manifest.end_event_id);
    match end_event {
        Some(event) => {
            if event.body.session_id != report.session_id
                || event.event_hash().ok() != Some(manifest.end_event_hash)
                || !matches!(event.body.payload, EventPayload::SessionEnded { .. })
            {
                state.invalid(
                    FindingCode::ManifestEndEventMismatch,
                    "manifest does not identify the included signed session-end event".into(),
                );
            }
            if !receipts.contains_key(&manifest.end_event_id) {
                state.gap(
                    FindingCode::ManifestEndEventMissing,
                    "manifest end event has no included relay receipt".into(),
                );
            }
        }
        None => state.gap(
            FindingCode::ManifestEndEventMissing,
            "manifest references an end event body absent from the report".into(),
        ),
    }

    let starts: Vec<_> = events
        .values()
        .copied()
        .filter(|event| matches!(event.body.payload, EventPayload::SessionStarted))
        .collect();
    if starts.is_empty() {
        state.gap(
            FindingCode::MissingSessionStart,
            "no signed session-start event is included".into(),
        );
    } else if starts.len() > 1 {
        state.invalid(
            FindingCode::InvalidSessionStart,
            "multiple accepted session-start events are included".into(),
        );
    } else if let Some(receipt) = receipts.get(&starts[0].body.event_id)
        && receipt.relay_timestamp_ms != manifest.start_relay_timestamp_ms
    {
        state.invalid(
            FindingCode::InvalidSessionStart,
            "manifest start time differs from the session-start receipt".into(),
        );
    } else if !receipts.contains_key(&starts[0].body.event_id) {
        state.gap(
            FindingCode::MissingSessionStart,
            "session-start event has no relay receipt".into(),
        );
    }

    let ends: Vec<_> = events
        .values()
        .copied()
        .filter(|event| matches!(event.body.payload, EventPayload::SessionEnded { .. }))
        .collect();
    if ends.len() > 1 {
        state.invalid(
            FindingCode::ManifestEndEventMismatch,
            "multiple accepted session-end events are included".into(),
        );
    }
    if let (Some(start), Some(end)) = (starts.first(), ends.first()) {
        if start.body.participant_id != end.body.participant_id {
            state.invalid(
                FindingCode::ManifestEndEventMismatch,
                "session start and end are not signed by the same participant key".into(),
            );
        }
        if let (Some(start_receipt), Some(end_receipt)) = (
            receipts.get(&start.body.event_id),
            receipts.get(&end.body.event_id),
        ) {
            if start_receipt.evidence_order >= end_receipt.evidence_order {
                state.invalid(
                    FindingCode::InvalidSessionStart,
                    "session end does not follow session start in relay evidence order".into(),
                );
            }
            if end_receipt.relay_timestamp_ms != manifest.end_relay_timestamp_ms {
                state.invalid(
                    FindingCode::ManifestEndEventMismatch,
                    "manifest end time differs from the session-end receipt".into(),
                );
            }
        }
    }
    let start_order = starts
        .first()
        .and_then(|event| receipts.get(&event.body.event_id))
        .map(|receipt| receipt.evidence_order);
    let end_order = receipts
        .get(&manifest.end_event_id)
        .map(|receipt| receipt.evidence_order);
    for event in events.values() {
        let Some(receipt) = receipts.get(&event.body.event_id) else {
            continue;
        };
        match &event.body.payload {
            EventPayload::ParticipantJoined { .. } => {
                if start_order.is_some_and(|start| receipt.evidence_order >= start) {
                    state.invalid(
                        FindingCode::InvalidSessionStart,
                        format!(
                            "participant {} joined after membership froze",
                            event.body.participant_id.as_uuid()
                        ),
                    );
                }
            }
            EventPayload::SyntheticAiAccessAttempt { .. } => {
                if start_order.is_some_and(|start| receipt.evidence_order <= start)
                    || end_order.is_some_and(|end| receipt.evidence_order >= end)
                {
                    state.invalid(
                        FindingCode::ManifestEndEventMismatch,
                        format!(
                            "synthetic event {} is outside the active session",
                            event.body.event_id.as_uuid()
                        ),
                    );
                }
            }
            EventPayload::SessionEnded { .. } => {
                if event.body.event_id != manifest.end_event_id {
                    state.invalid(
                        FindingCode::ManifestEndEventMismatch,
                        "an accepted session-end event differs from the manifest end event".into(),
                    );
                }
            }
            EventPayload::SessionStarted => {}
        }
    }

    let end_notices: Vec<_> = report
        .notices
        .iter()
        .filter(|notice| notice.kind == RelayNoticeKind::SessionEnded)
        .collect();
    match end_notices.len() {
        0 => state.gap(
            FindingCode::MissingSessionEndNotice,
            "relay session-ended notice is absent".into(),
        ),
        1 => {
            if let Some(end_receipt) = receipts.get(&manifest.end_event_id)
                && end_notices[0].evidence_order <= end_receipt.evidence_order
            {
                state.invalid(
                    FindingCode::InvalidNotice,
                    "session-ended notice does not follow the end-event receipt".into(),
                );
            }
        }
        _ => state.invalid(
            FindingCode::DuplicateSessionEndNotice,
            "multiple relay session-ended notices are included".into(),
        ),
    }
}

fn insert_evidence_order(
    orders: &mut HashMap<u64, &'static str>,
    order: u64,
    kind: &'static str,
    state: &mut VerificationState,
) {
    if order == 0 {
        state.invalid(
            FindingCode::DuplicateEvidenceOrder,
            format!("{kind} has zero evidence order"),
        );
        return;
    }
    if orders.insert(order, kind).is_some() {
        state.invalid(
            FindingCode::DuplicateEvidenceOrder,
            format!("evidence order {order} is used more than once"),
        );
    }
}

fn validate_evidence_orders(
    manifest: Option<&equalturn_protocol::SessionManifest>,
    evidence_orders: &HashMap<u64, &'static str>,
    state: &mut VerificationState,
) {
    let Some(manifest) = manifest else {
        return;
    };
    let highest = manifest
        .highest_evidence_order
        .min(MAX_REPORT_EVIDENCE_ORDER);
    let mut missing_orders = Vec::new();
    for order in 1..=highest {
        if !evidence_orders.contains_key(&order) {
            missing_orders.push(order);
        }
    }
    if !missing_orders.is_empty() {
        state.gap(
            FindingCode::MissingEvidenceOrder,
            format!(
                "relay evidence orders {} are missing",
                compact_ranges(&missing_orders)
            ),
        );
    }
    for order in evidence_orders.keys() {
        if *order > manifest.highest_evidence_order {
            state.invalid(
                FindingCode::EvidenceOrderBeyondManifest,
                format!("evidence order {order} exceeds the manifest high-water mark"),
            );
        }
    }
}

fn compact_ranges(sequences: &[u64]) -> String {
    let mut ranges = Vec::new();
    let mut iter = sequences.iter().copied();
    let Some(mut start) = iter.next() else {
        return String::new();
    };
    let mut end = start;
    for sequence in iter {
        if sequence == end.saturating_add(1) {
            end = sequence;
        } else {
            ranges.push(format_range(start, end));
            start = sequence;
            end = sequence;
        }
    }
    ranges.push(format_range(start, end));
    ranges.join(", ")
}

fn format_range(start: u64, end: u64) -> String {
    if start == end {
        start.to_string()
    } else {
        format!("{start}-{end}")
    }
}

#[derive(Default)]
struct VerificationState {
    invalid: bool,
    gaps: bool,
    findings: Vec<Finding>,
}

impl VerificationState {
    fn invalid(&mut self, code: FindingCode, detail: String) {
        self.invalid = true;
        self.findings.push(Finding { code, detail });
    }

    fn gap(&mut self, code: FindingCode, detail: String) {
        self.gaps = true;
        self.findings.push(Finding { code, detail });
    }

    fn finish(self, relay_identity: RelayIdentityStatus) -> VerificationResult {
        let status = if self.invalid {
            VerificationStatus::Invalid
        } else if self.gaps {
            VerificationStatus::ValidWithGaps
        } else {
            VerificationStatus::Valid
        };
        VerificationResult {
            status,
            relay_identity,
            findings: self.findings,
        }
    }
}
