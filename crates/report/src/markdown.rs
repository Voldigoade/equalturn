use std::collections::HashMap;

use equalturn_protocol::{EventPayload, ParticipantEvent, RelayNoticeKind};

use crate::{EvidenceBundle, RelayIdentityStatus, VerificationStatus};

pub fn render_markdown(report: &EvidenceBundle) -> String {
    let verification = report.verify();
    let mut output = String::new();
    output.push_str("# Equalturn session report\n\n");
    output.push_str(&format!(
        "- Verification: **{}**\n",
        status_label(verification.status)
    ));
    output.push_str(&format!(
        "- Relay identity: {}\n",
        relay_label(verification.relay_identity)
    ));
    output.push_str(&format!(
        "- Report schema: {}\n",
        report.report_schema_version
    ));
    output.push_str(&format!("- Protocol: {}\n", report.protocol_version));
    output.push_str(&format!(
        "- Session ID: `{}`\n",
        report.session_id.as_uuid()
    ));
    if let Some(manifest) = &report.manifest {
        let elapsed = manifest
            .end_relay_timestamp_ms
            .saturating_sub(manifest.start_relay_timestamp_ms);
        output.push_str(&format!(
            "- Relay-reported start: {} ms since Unix epoch\n",
            manifest.start_relay_timestamp_ms
        ));
        output.push_str(&format!(
            "- Relay-reported end: {} ms since Unix epoch\n",
            manifest.end_relay_timestamp_ms
        ));
        output.push_str(&format!("- Relay-reported elapsed time: {elapsed} ms\n"));
        output.push_str(&format!(
            "- Evidence order high-water: {}\n",
            manifest.highest_evidence_order
        ));
    } else {
        output.push_str("- End-of-session manifest: missing\n");
    }
    output.push_str("\nSigned claims authenticate a participant key's statement; they do not prove workstation truth, event completeness before signing, or absence of prohibited AI use. Relay timestamps and the relay's manifest are relay assertions.\n\n");

    output.push_str("## Participants\n\n");
    output.push_str(
        "| Nickname | Participant ID | Key fingerprint | Client version | OS (self-reported) |\n",
    );
    output.push_str("| --- | --- | --- | --- | --- |\n");
    for participant in &report.participants {
        output.push_str(&format!(
            "| {} | `{}` | `{}` | {} | {}{} |\n",
            escape_cell(&participant.nickname),
            participant.participant_id.as_uuid(),
            participant.key_fingerprint.to_hex(),
            escape_cell(&participant.client_version),
            os_label(participant.os_family),
            if participant.os_bucket.is_empty() {
                String::new()
            } else {
                format!(" {}", escape_cell(&participant.os_bucket))
            }
        ));
    }
    if report.participants.is_empty() {
        output.push_str("\nNo participant profiles are present.\n");
    }

    output.push_str("\n## Participant events\n\n");
    output.push_str("| Participant | Sequence | Relay time | Event | Detail |\n");
    output.push_str("| --- | ---: | ---: | --- | --- |\n");
    let receipt_times: HashMap<_, _> = report
        .receipts
        .iter()
        .map(|receipt| (receipt.event_id, receipt.relay_timestamp_ms))
        .collect();
    let participant_names: HashMap<_, _> = report
        .participants
        .iter()
        .map(|participant| (participant.participant_id, participant.nickname.as_str()))
        .collect();
    let mut events = report.events.iter().collect::<Vec<_>>();
    events.sort_by_key(|event| (event.body.sequence, event.body.participant_id));
    for event in events {
        let nickname = participant_names
            .get(&event.body.participant_id)
            .copied()
            .unwrap_or("unknown participant");
        output.push_str(&event_row(
            event,
            nickname,
            receipt_times.get(&event.body.event_id).copied(),
            false,
        ));
    }
    if report.events.is_empty() {
        output.push_str("| — | — | — | None | — |\n");
    }

    if !report.local_only_events.is_empty() {
        output.push_str("\n## Local-only, unreceipted events\n\n");
        output.push_str("These participant-signed events were not acknowledged by the relay. They are not relay-accepted evidence.\n\n");
        output.push_str("| Participant | Sequence | Relay time | Event | Detail |\n");
        output.push_str("| --- | ---: | ---: | --- | --- |\n");
        let mut local = report.local_only_events.iter().collect::<Vec<_>>();
        local.sort_by_key(|event| (event.body.sequence, event.body.participant_id));
        for event in local {
            let nickname = participant_names
                .get(&event.body.participant_id)
                .copied()
                .unwrap_or("unknown participant");
            output.push_str(&event_row(event, nickname, None, true));
        }
    }

    output.push_str("\n## Relay liveness notices\n\n");
    output.push_str("| Evidence order | Relay time | Participant | State |\n");
    output.push_str("| ---: | ---: | --- | --- |\n");
    let mut notices = report.notices.iter().collect::<Vec<_>>();
    notices.sort_by_key(|notice| notice.evidence_order);
    for notice in notices {
        let participant = notice
            .participant_id
            .and_then(|id| participant_names.get(&id).copied())
            .unwrap_or("—");
        output.push_str(&format!(
            "| {} | {} ms | {} | {} |\n",
            notice.evidence_order,
            notice.relay_timestamp_ms,
            escape_cell(participant),
            notice_label(notice.kind)
        ));
    }
    if report.notices.is_empty() {
        output.push_str("| — | — | — | No relay notices |\n");
    }

    output.push_str("\n## Verification findings\n\n");
    if verification.findings.is_empty() {
        output.push_str("No structural or cryptographic gaps were found in the included evidence. This is not proof of honest conduct or complete workstation observation.\n");
    } else {
        for finding in verification.findings {
            output.push_str(&format!(
                "- `{}`: {}\n",
                finding.code.as_str(),
                escape_text(&finding.detail)
            ));
        }
    }
    output
}

fn event_row(
    event: &ParticipantEvent,
    nickname: &str,
    relay_time: Option<u64>,
    local_only: bool,
) -> String {
    let (kind, detail) = match &event.body.payload {
        EventPayload::ParticipantJoined { .. } => (
            "Participant joined",
            "signed participant metadata".to_owned(),
        ),
        EventPayload::SessionStarted => (
            "Session started",
            "participant-signed lifecycle event; creator role is not independently proven"
                .to_owned(),
        ),
        EventPayload::SyntheticAiAccessAttempt { target } => (
            "Synthetic AI access attempt",
            target.as_deref().unwrap_or("target unspecified").to_owned(),
        ),
        EventPayload::SessionEnded { .. } => (
            "Session ended",
            "participant-signed lifecycle event; creator role is not independently proven"
                .to_owned(),
        ),
    };
    format!(
        "| {} | {} | {} | {}{} | {} |\n",
        escape_cell(nickname),
        event.body.sequence,
        relay_time
            .map(|time| format!("{time} ms"))
            .unwrap_or_else(|| "—".to_owned()),
        if local_only { "Unreceipted: " } else { "" },
        kind,
        escape_cell(&detail)
    )
}

fn status_label(status: VerificationStatus) -> &'static str {
    match status {
        VerificationStatus::Valid => "VALID",
        VerificationStatus::ValidWithGaps => "VALID WITH GAPS",
        VerificationStatus::Invalid => "INVALID",
    }
}

fn relay_label(status: RelayIdentityStatus) -> &'static str {
    match status {
        RelayIdentityStatus::Anchored => "anchored to the supplied key",
        RelayIdentityStatus::Unanchored => "unanchored (included key is not externally pinned)",
        RelayIdentityStatus::Mismatched => {
            "mismatched (included key does not match the supplied pin)"
        }
    }
}

fn os_label(os: equalturn_protocol::OsFamily) -> &'static str {
    match os {
        equalturn_protocol::OsFamily::Unknown => "Unknown",
        equalturn_protocol::OsFamily::Windows => "Windows",
        equalturn_protocol::OsFamily::Linux => "Linux",
        equalturn_protocol::OsFamily::Other => "Other",
    }
}

fn notice_label(kind: RelayNoticeKind) -> &'static str {
    match kind {
        RelayNoticeKind::ParticipantOnline => "Online (liveness only)",
        RelayNoticeKind::ParticipantStale => "Stale (liveness unknown)",
        RelayNoticeKind::ParticipantDisconnected => "Disconnected (cause unknown)",
        RelayNoticeKind::SessionEnded => "Session ended",
    }
}

fn escape_cell(value: &str) -> String {
    escape_text(value)
        .replace('|', "&#124;")
        .replace(['\n', '\r'], " ")
}

fn escape_text(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        if matches!(
            character,
            '\\' | '`'
                | '*'
                | '_'
                | '{'
                | '}'
                | '['
                | ']'
                | '('
                | ')'
                | '<'
                | '>'
                | '#'
                | '+'
                | '-'
                | '.'
                | '!'
                | '|'
        ) {
            escaped.push('\\');
        }
        if character == '\n' || character == '\r' {
            escaped.push(' ');
        } else {
            escaped.push(character);
        }
    }
    escaped
}
