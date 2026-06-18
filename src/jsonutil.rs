//! Small helpers for reaching into untyped `serde_json::Value` trees.
//!
//! The dashboard and CLI both read benchmark reports and run params as loosely
//! typed JSON so that unknown or adapter-specific fields survive round-trips.
//! These helpers keep that access terse and consistent.

use serde_json::Value;

/// Walk a path of object keys, returning the nested value when every hop exists.
pub fn nested<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut cursor = value;
    for key in path {
        cursor = cursor.get(*key)?;
    }
    Some(cursor)
}

/// Nested lookup coerced to `u64`.
pub fn nested_u64(value: &Value, path: &[&str]) -> Option<u64> {
    nested(value, path)?.as_u64()
}

/// Nested lookup coerced to `f64`.
pub fn nested_f64(value: &Value, path: &[&str]) -> Option<f64> {
    nested(value, path)?.as_f64()
}

/// Nested lookup coerced to `&str`.
pub fn nested_str<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    nested(value, path)?.as_str()
}

/// Nested lookup coerced to `bool`.
pub fn nested_bool(value: &Value, path: &[&str]) -> Option<bool> {
    nested(value, path)?.as_bool()
}

/// Owned `String` from a nested lookup.
pub fn nested_string(value: &Value, path: &[&str]) -> Option<String> {
    nested_str(value, path).map(ToOwned::to_owned)
}
