use equalturn_registry::{Registry, RegistryError};
use serde_json::Value;

const REGISTRY: &[u8] = include_bytes!("../../../registry/registry-v3.json");

fn mutable_registry() -> Value {
    serde_json::from_slice(REGISTRY).expect("checked-in registry parses as JSON")
}

fn first_web_hostname(registry: &mut Value) -> &mut Value {
    registry["entries"]
        .as_array_mut()
        .expect("entries are an array")
        .iter_mut()
        .flat_map(|entry| {
            entry["indicators"]
                .as_array_mut()
                .expect("indicators are an array")
                .iter_mut()
        })
        .find(|indicator| indicator["kind"] == "web_hostname")
        .expect("registry contains a web hostname indicator")
}

#[test]
fn rejects_terminal_control_characters_in_display_strings() {
    for control in ["\0", "\u{1b}[2J", "\u{7f}"] {
        let mut registry = mutable_registry();
        registry["entries"][0]["product"] = Value::String(format!("product{control}name"));
        let bytes = serde_json::to_vec(&registry).expect("mutated registry serializes");

        assert!(
            matches!(Registry::parse(&bytes), Err(RegistryError::Semantic(_))),
            "registry control character {control:?} must be rejected"
        );
    }
}

#[test]
fn rejects_control_characters_in_object_keys() {
    let mut registry = mutable_registry();
    registry
        .as_object_mut()
        .expect("registry root is an object")
        .insert("unexpected\u{1b}key".into(), Value::Null);
    let bytes = serde_json::to_vec(&registry).expect("mutated registry serializes");

    assert!(matches!(
        Registry::parse(&bytes),
        Err(RegistryError::Semantic(_))
    ));
}

#[test]
fn rejects_homoglyphs_in_hostname_indicators_but_accepts_unicode_product_names() {
    let mut registry = mutable_registry();
    registry["entries"][0]["product"] = Value::String("产品名称".into());
    first_web_hostname(&mut registry)["value"] = Value::String("раypal.example".into());
    let bytes = serde_json::to_vec(&registry).expect("mutated registry serializes");

    assert!(matches!(
        Registry::parse(&bytes),
        Err(RegistryError::Semantic(_))
    ));

    let mut registry = mutable_registry();
    registry["entries"][0]["product"] = Value::String("产品名称".into());
    let bytes = serde_json::to_vec(&registry).expect("Unicode product name serializes");
    assert!(Registry::parse(&bytes).is_ok());
}

#[test]
fn rejects_deeply_nested_json_without_panicking() {
    let depth = 1024;
    let mut bytes = vec![b'['; depth];
    bytes.extend_from_slice(b"null");
    bytes.extend(std::iter::repeat_n(b']', depth));

    assert!(matches!(
        Registry::parse(&bytes),
        Err(RegistryError::MalformedJson)
    ));
}

#[test]
fn rejects_raw_nul_bytes_in_json() {
    assert!(matches!(
        Registry::parse(b"{\0}"),
        Err(RegistryError::MalformedJson)
    ));
}

#[test]
fn rejects_schema_maximum_string_length_and_entry_count_overruns() {
    let mut overlong = mutable_registry();
    overlong["entries"][0]["product"] = Value::String("x".repeat(301));
    let bytes = serde_json::to_vec(&overlong).expect("overlong registry serializes");
    assert!(matches!(
        Registry::parse(&bytes),
        Err(RegistryError::Schema(_))
    ));

    let mut too_many = mutable_registry();
    let entries = too_many["entries"]
        .as_array_mut()
        .expect("entries are an array");
    let template = entries
        .iter()
        .find(|entry| entry["id"] == "brave-leo")
        .expect("minimal entry exists")
        .clone();
    let entries: Vec<_> = (0..1001)
        .map(|index| {
            let mut entry = template.clone();
            entry["id"] = Value::String(format!("entry-{index:04}"));
            entry
        })
        .collect();
    too_many["entries"] = Value::Array(entries);
    let bytes = serde_json::to_vec(&too_many).expect("large entry array serializes");
    assert!(matches!(
        Registry::parse(&bytes),
        Err(RegistryError::Schema(_))
    ));
}
