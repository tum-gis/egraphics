/// A generic attribute value attached to an [`Object`](super::Object).
///
/// Covers the common primitive attribute types (string/int/float/bool); extend with new
/// variants if a richer type (date, URI, nested set, ...) is needed later.
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyValue {
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
}

impl From<String> for PropertyValue {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<&str> for PropertyValue {
    fn from(value: &str) -> Self {
        Self::String(value.to_string())
    }
}

impl From<i64> for PropertyValue {
    fn from(value: i64) -> Self {
        Self::Int(value)
    }
}

impl From<f64> for PropertyValue {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}

impl From<bool> for PropertyValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_impls_produce_expected_variants() {
        assert_eq!(
            PropertyValue::from("Building"),
            PropertyValue::String("Building".into())
        );
        assert_eq!(
            PropertyValue::from(String::from("Road")),
            PropertyValue::String("Road".into())
        );
        assert_eq!(PropertyValue::from(5i64), PropertyValue::Int(5));
        assert_eq!(PropertyValue::from(12.5f64), PropertyValue::Float(12.5));
        assert_eq!(PropertyValue::from(true), PropertyValue::Bool(true));
    }
}
