use equalturn_registry::{REGISTRY_MAX_BYTES, Registry, RegistryError};

const REGISTRY: &[u8] = include_bytes!("../../../registry/registry-v3.json");

#[test]
fn parses_the_checked_in_registry_and_exposes_typed_queries() {
    let registry = Registry::parse(REGISTRY).expect("checked-in registry is valid");

    assert_eq!(registry.schema_version(), 3);
    assert_eq!(registry.registry_version(), "0.4.0");
    assert_eq!(registry.entries().len(), 131);
    assert_eq!(registry.indicator_count(), 84);
    let ollama = registry.entry("ollama").unwrap();
    assert_eq!(ollama.provider().name(), "Ollama");
    assert!(
        registry
            .search("openai")
            .iter()
            .any(|entry| entry.product() == "ChatGPT")
    );
}

#[test]
fn rejects_duplicate_json_object_keys() {
    let input = br#"{"schema_version":3,"schema_version":3}"#;

    assert!(matches!(
        Registry::parse(input),
        Err(RegistryError::DuplicateJsonKey)
    ));
}

#[test]
fn rejects_unknown_fields_and_unsupported_schema_versions() {
    let unknown = br#"{"schema_version":3,"registry_version":"1.0.0","last_updated":"2026-10-19","purpose":"research_only","entries":[],"surprise":true}"#;
    let unsupported = br#"{"schema_version":4,"registry_version":"1.0.0","last_updated":"2026-10-19","purpose":"research_only","entries":[]}"#;

    assert!(matches!(
        Registry::parse(unknown),
        Err(RegistryError::Schema(_))
    ));
    assert!(matches!(
        Registry::parse(unsupported),
        Err(RegistryError::Schema(_))
    ));
}

#[test]
fn rejects_oversized_and_invalid_utf8_inputs() {
    let oversized = vec![b' '; REGISTRY_MAX_BYTES + 1];

    assert!(matches!(
        Registry::parse(&oversized),
        Err(RegistryError::TooLarge { .. })
    ));
    assert!(matches!(
        Registry::parse(&[0xff]),
        Err(RegistryError::InvalidUtf8)
    ));
}

#[test]
fn rejects_duplicate_ids_and_future_dates() {
    let mut json: serde_json::Value = serde_json::from_slice(REGISTRY).unwrap();
    let entries = json["entries"].as_array_mut().unwrap();
    entries[1]["id"] = entries[0]["id"].clone();
    let duplicate_id = serde_json::to_vec(&json).unwrap();
    assert!(matches!(
        Registry::parse(&duplicate_id),
        Err(RegistryError::Semantic(_))
    ));

    json["entries"][1]["id"] = serde_json::Value::String("future-date-case".into());
    json["last_updated"] = serde_json::Value::String("2999-01-01".into());
    let future_date = serde_json::to_vec(&json).unwrap();
    assert!(matches!(
        Registry::parse(&future_date),
        Err(RegistryError::Semantic(_))
    ));
}

#[test]
fn rejects_a_root_that_does_not_match_the_out_of_band_pin() {
    let root = b"not a TUF root";
    let pin = [0_u8; 32];

    assert!(matches!(
        equalturn_registry::verify_bootstrap_root(root, &pin),
        Err(RegistryError::BootstrapPinMismatch)
    ));
}
