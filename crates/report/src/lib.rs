mod markdown;
mod model;
mod verify;

pub use model::{
    EvidenceBundle, MAX_REPORT_EVIDENCE_ORDER, MAX_REPORT_JSON_BYTES, ParticipantSummary,
    REPORT_SCHEMA_VERSION, RelayIdentity, ReportError,
};

pub use markdown::render_markdown;
pub use verify::{
    Finding, FindingCode, RelayIdentityStatus, VerificationResult, VerificationStatus,
    verify_report_json, verify_report_json_with_relay_key,
};

impl EvidenceBundle {
    /// Verifies the evidence carried by this report without trusting the relay key identity.
    pub fn verify(&self) -> VerificationResult {
        verify::verify_bundle(self, None)
    }

    /// Verifies the evidence and requires the included relay key to match a locally pinned key.
    pub fn verify_with_relay_key(
        &self,
        expected_relay_key: &equalturn_protocol::PublicKey,
    ) -> VerificationResult {
        verify::verify_bundle(self, Some(expected_relay_key))
    }

    /// Serializes this evidence bundle as bounded, pretty-printed JSON.
    pub fn to_json_pretty(&self) -> Result<String, ReportError> {
        let json = serde_json::to_string_pretty(self)?;
        if json.len() > MAX_REPORT_JSON_BYTES {
            return Err(ReportError::ReportTooLarge);
        }
        Ok(json)
    }

    /// Parses the v1 JSON report representation, rejecting noncanonical UUID spelling.
    pub fn from_json(json: &str) -> Result<Self, ReportError> {
        if json.len() > MAX_REPORT_JSON_BYTES {
            return Err(ReportError::ReportTooLarge);
        }
        let report: Self = serde_json::from_str(json)?;
        let value: serde_json::Value = serde_json::from_str(json)?;
        if serde_json::to_value(&report)? != value {
            return Err(ReportError::NoncanonicalJson);
        }
        Ok(report)
    }

    /// Renders a readable summary. User-supplied strings are escaped for Markdown tables.
    pub fn render_markdown(&self) -> String {
        render_markdown(self)
    }
}
