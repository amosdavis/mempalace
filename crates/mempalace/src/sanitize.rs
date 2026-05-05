use crate::error::MpError;
use regex::Regex;
use std::sync::OnceLock;

static NAME_RE: OnceLock<Regex> = OnceLock::new();

fn name_regex() -> &'static Regex {
    NAME_RE.get_or_init(|| {
        Regex::new(r"^(?:[^\W_]|[^\W_][\w .'\-]{0,126}[^\W_])$").unwrap()
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
