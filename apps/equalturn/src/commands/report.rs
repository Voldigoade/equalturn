use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Write},
    path::Path,
};

use equalturn_client_core::SessionProfile;
use equalturn_protocol::PublicKey;
use equalturn_report::{
    EvidenceBundle, MAX_REPORT_JSON_BYTES, VerificationStatus, verify_report_json,
    verify_report_json_with_relay_key,
};

use crate::{
    app::{AppError, Context, parse_participant_id, parse_session_id},
    cli::ReportCommand,
};

pub async fn run(context: &Context, command: ReportCommand) -> Result<(), AppError> {
    match command {
        ReportCommand::Export {
            session,
            json,
            markdown,
            participant,
        } => {
            export(
                context,
                &session,
                &json,
                markdown.as_deref(),
                participant.as_deref(),
            )
            .await
        }
        ReportCommand::Verify { report, relay_key } => verify(&report, relay_key.as_deref()),
    }
}

async fn export(
    context: &Context,
    session: &str,
    json_path: &Path,
    markdown_path: Option<&Path>,
    participant_selector: Option<&str>,
) -> Result<(), AppError> {
    let session_id = parse_session_id(session)?;
    let mut bundle = context.relay.session_report(session_id).await?;
    let profiles = context.store.profiles_for_session(session_id).await?;
    let local_profile = match participant_selector {
        Some(selector) => {
            let participant_id = parse_participant_id(selector)?;
            Some(
                profiles
                    .into_iter()
                    .find(|profile| profile.participant_id == participant_id)
                    .ok_or_else(|| {
                        AppError::Message("local participant profile was not found".into())
                    })?,
            )
        }
        None => match profiles.as_slice() {
            [profile] => Some(profile.clone()),
            [] => None,
            _ => {
                return Err(AppError::Message(
                    "multiple local profiles exist; select one with --participant".into(),
                ));
            }
        },
    };
    if let Some(profile) = local_profile.as_ref() {
        let verification = bundle.verify_with_relay_key(&profile.relay_public_key);
        if verification.status == VerificationStatus::Invalid {
            return Err(AppError::Message(
                "relay report failed verification against the locally pinned relay key".into(),
            ));
        }
        add_pending_local_events(context, &mut bundle, profile).await?;
    }
    let encoded = bundle.to_json_pretty()?;
    write_new(json_path, encoded.as_bytes())?;
    if let Some(path) = markdown_path {
        write_new(path, bundle.render_markdown().as_bytes())?;
        println!("JSON evidence: {}", json_path.display());
        println!("Markdown summary: {}", path.display());
    } else {
        println!("{}", bundle.render_markdown());
        println!("JSON evidence: {}", json_path.display());
    }
    let result = if let Some(profile) = local_profile {
        bundle.verify_with_relay_key(&profile.relay_public_key)
    } else {
        bundle.verify()
    };
    println!(
        "Verification: {:?} ({:?} relay identity)",
        result.status, result.relay_identity
    );
    Ok(())
}

async fn add_pending_local_events(
    context: &Context,
    bundle: &mut EvidenceBundle,
    profile: &SessionProfile,
) -> Result<(), AppError> {
    let pending = context
        .store
        .pending_events(profile.session_id, profile.participant_id)
        .await?;
    for event in pending {
        let accepted_event = bundle
            .events
            .iter()
            .find(|accepted| accepted.body.event_id == event.body.event_id);
        if let Some(accepted_event) = accepted_event {
            let accepted_hash = accepted_event.event_hash()?;
            let receipt = bundle
                .receipts
                .iter()
                .find(|receipt| receipt.event_id == event.body.event_id)
                .ok_or_else(|| {
                    AppError::Message(
                        "relay report contains an accepted event without its receipt".into(),
                    )
                })?;
            if event.event_hash()? != accepted_hash || receipt.event_hash != accepted_hash {
                return Err(AppError::Message(
                    "local queued event conflicts with relay evidence".into(),
                ));
            }
            context.store.acknowledge(receipt).await?;
        } else {
            bundle.local_only_events.push(event);
        }
    }
    bundle
        .local_only_events
        .sort_by_key(|event| (event.body.participant_id, event.body.sequence));
    Ok(())
}

pub fn verify(path: &Path, relay_key_hex: Option<&str>) -> Result<(), AppError> {
    let file = File::open(path)?;
    if file.metadata()?.len() > u64::try_from(MAX_REPORT_JSON_BYTES).unwrap_or(u64::MAX) {
        return Err(AppError::Message("report exceeds the 64 MiB limit".into()));
    }
    let bytes = read_bounded(file, MAX_REPORT_JSON_BYTES).map_err(|error| {
        if error.kind() == io::ErrorKind::InvalidData {
            AppError::Message("report exceeds the 64 MiB limit".into())
        } else {
            AppError::Io(error)
        }
    })?;
    let json = std::str::from_utf8(&bytes)
        .map_err(|_| AppError::Message("report JSON is not UTF-8".into()))?;
    let result = if let Some(encoded_key) = relay_key_hex {
        let key_bytes = equalturn_protocol::decode_hex::<32>(encoded_key).map_err(|_| {
            AppError::Message("relay key must be 64 lowercase hexadecimal characters".into())
        })?;
        let relay_key = PublicKey::new(key_bytes);
        verify_report_json_with_relay_key(json, &relay_key)
    } else {
        verify_report_json(json)
    };
    println!("Verification: {:?}", result.status);
    println!("Relay identity: {:?}", result.relay_identity);
    for finding in &result.findings {
        println!("{}: {}", finding.code_str(), finding.detail);
    }
    println!(
        "A valid report authenticates included evidence; it does not prove no cheating occurred."
    );
    if result.status == VerificationStatus::Invalid {
        return Err(AppError::Message("evidence bundle is INVALID".into()));
    }
    Ok(())
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), AppError> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn read_bounded<R: Read>(reader: R, maximum_bytes: usize) -> io::Result<Vec<u8>> {
    let read_limit = maximum_bytes
        .checked_add(1)
        .and_then(|limit| u64::try_from(limit).ok())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid byte limit"))?;
    let mut bytes = Vec::new();
    reader.take(read_limit).read_to_end(&mut bytes)?;
    if bytes.len() > maximum_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "input exceeds configured byte limit",
        ));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::read_bounded;

    #[test]
    fn bounded_reader_rejects_input_that_grows_past_the_limit() {
        let error = read_bounded(Cursor::new(b"12345"), 4)
            .expect_err("reader stops after the configured bound");

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    }

    #[test]
    fn bounded_reader_accepts_input_at_the_limit() {
        let bytes = read_bounded(Cursor::new(b"1234"), 4).expect("input is within the bound");

        assert_eq!(bytes, b"1234");
    }
}
