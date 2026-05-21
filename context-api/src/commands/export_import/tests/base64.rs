use super::super::{
    ExportFormat,
    base64::{
        base64_decode,
        base64_encode,
    },
};

#[test]
fn base64_round_trip_empty() {
    let encoded = base64_encode(b"");
    assert_eq!(encoded, "");
    let decoded = base64_decode(&encoded).unwrap();
    assert!(decoded.is_empty());
}

#[test]
fn base64_round_trip_small() {
    for input in &[b"a".as_slice(), b"ab", b"abc", b"abcd", b"hello world"] {
        let encoded = base64_encode(input);
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(&decoded, input, "round-trip failed for {:?}", input);
    }
}

#[test]
fn base64_round_trip_binary() {
    let input: Vec<u8> = (0..=255).collect();
    let encoded = base64_encode(&input);
    let decoded = base64_decode(&encoded).unwrap();
    assert_eq!(decoded, input);
}

#[test]
fn export_format_serde_json_variant() {
    let json = serde_json::to_string(&ExportFormat::Json).unwrap();
    assert_eq!(json, r#""json""#);
    let deser: ExportFormat = serde_json::from_str(&json).unwrap();
    assert_eq!(deser, ExportFormat::Json);
}

#[test]
fn export_format_serde_bincode_variant() {
    let json = serde_json::to_string(&ExportFormat::Bincode).unwrap();
    assert_eq!(json, r#""bincode""#);
    let deser: ExportFormat = serde_json::from_str(&json).unwrap();
    assert_eq!(deser, ExportFormat::Bincode);
}
