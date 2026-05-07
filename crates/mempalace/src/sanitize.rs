use crate::error::MpError;
use regex::Regex;
use std::sync::OnceLock;

static NAME_RE: OnceLock<Regex> = OnceLock::new();

fn name_regex() -> &'static Regex {
    NAME_RE.get_or_init(|| {
        Regex::new(r"^(?:[^\W_]|[^\W_][\w .'\-]{0,126}[^\W_])$").expect("valid regex literal")
    })
}

pub fn sanitize_name(value: &str, field_name: &str) -> Result<String, MpError> {
    if value.is_empty() {
        return Err(MpError::Validation(format!("{field_name} cannot be empty")));
    }
    if value.len() > 128 {
        return Err(MpError::Validation(format!("{field_name} too long (max 128 chars)")));
    }
    if value.contains('\0') {
        return Err(MpError::Validation(format!("{field_name} contains null byte")));
    }
    if value.contains("..") || value.contains('/') || value.contains('\\') {
        return Err(MpError::Validation(format!("{field_name} contains path traversal")));
    }
    if !name_regex().is_match(value) {
        return Err(MpError::Validation(format!("{field_name} contains invalid characters: {value}")));
    }
    Ok(value.to_string())
}

pub fn sanitize_content(value: &str) -> Result<String, MpError> {
    if value.contains('\0') {
        return Err(MpError::Validation("content contains null byte".to_string()));
    }
    if value.len() > 100_000 {
        return Err(MpError::Validation("content too long (max 100,000 chars)".to_string()));
    }
    Ok(value.to_string())
}

pub fn sanitize_kg_value(value: &str, field_name: &str) -> Result<String, MpError> {
    if value.is_empty() {
        return Err(MpError::Validation(format!("{field_name} cannot be empty")));
    }
    if value.len() > 128 {
        return Err(MpError::Validation(format!("{field_name} too long (max 128 chars)")));
    }
    if value.contains('\0') {
        return Err(MpError::Validation(format!("{field_name} contains null byte")));
    }
    Ok(value.to_string())
}

pub fn sanitize_optional_name(value: Option<&str>, field_name: &str) -> Result<Option<String>, MpError> {
    match value {
        None | Some("") => Ok(None),
        Some(v) => sanitize_name(v, field_name).map(Some),
    }
}

pub fn normalize_wing_name(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c == ' ' || c == '-' { '_' } else { c })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_valid() {
        assert!(sanitize_name("wing_user", "test").is_ok());
        assert!(sanitize_name("my-room", "test").is_ok());
        assert!(sanitize_name("ab", "test").is_ok());
    }

    #[test]
    fn name_empty() {
        assert!(sanitize_name("", "field").is_err());
    }

    #[test]
    fn name_too_long() {
        let long = "a".repeat(129);
        assert!(sanitize_name(&long, "field").is_err());
    }

    #[test]
    fn name_null_byte() {
        assert!(sanitize_name("foo\0bar", "field").is_err());
    }

    #[test]
    fn name_path_traversal() {
        assert!(sanitize_name("../etc", "field").is_err());
        assert!(sanitize_name("foo/bar", "field").is_err());
        assert!(sanitize_name("foo\\bar", "field").is_err());
    }

    #[test]
    fn content_valid() {
        assert!(sanitize_content("hello world").is_ok());
    }

    #[test]
    fn content_null_byte() {
        assert!(sanitize_content("hello\0world").is_err());
    }

    #[test]
    fn content_too_long() {
        let long = "x".repeat(100_001);
        assert!(sanitize_content(&long).is_err());
    }

    #[test]
    fn content_max_length_ok() {
        let exact = "x".repeat(100_000);
        assert!(sanitize_content(&exact).is_ok());
    }

    #[test]
    fn kg_value_valid() {
        assert!(sanitize_kg_value("alice", "name").is_ok());
    }

    #[test]
    fn kg_value_empty() {
        assert!(sanitize_kg_value("", "name").is_err());
    }

    #[test]
    fn kg_value_too_long() {
        let long = "a".repeat(129);
        assert!(sanitize_kg_value(&long, "name").is_err());
    }

    #[test]
    fn optional_name_none() {
        assert_eq!(sanitize_optional_name(None, "f").unwrap(), None);
    }

    #[test]
    fn optional_name_empty_string() {
        assert_eq!(sanitize_optional_name(Some(""), "f").unwrap(), None);
    }

    #[test]
    fn optional_name_valid() {
        assert_eq!(
            sanitize_optional_name(Some("hello"), "f").unwrap(),
            Some("hello".to_string())
        );
    }

    #[test]
    fn normalize_wing() {
        assert_eq!(normalize_wing_name("My Wing"), "my_wing");
        assert_eq!(normalize_wing_name("foo-bar"), "foo_bar");
        assert_eq!(normalize_wing_name("UPPER"), "upper");
    }
}
