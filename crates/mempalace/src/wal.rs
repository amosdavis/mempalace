use crate::config::MempalaceConfig;
use serde_json::Value;
use std::io::Write;

const WAL_REDACT_KEYS: &[&str] = &[
    "content", "content_preview", "document", "entry", "entry_preview",
    "query", "text",
];

pub fn wal_log(operation: &str, params: &Value, result: Option<&Value>) {
    let wal_dir = MempalaceConfig::wal_dir();
    if std::fs::create_dir_all(&wal_dir).is_err() {
        return;
    }
    let log_file = wal_dir.join("write_log.jsonl");
    let redacted_params = redact_value(params);
    let redacted_result = result.map(|r| redact_value(r));
    let entry = serde_json::json!({
        "operation": operation,
        "params": redacted_params,
        "result": redacted_result,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)
    {
        let _ = writeln!(file, "{}", entry);
    }
}

fn redact_value(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut new_map = serde_json::Map::new();
            for (k, v) in map {
                if WAL_REDACT_KEYS.contains(&k.as_str()) {
                    new_map.insert(k.clone(), Value::String("[REDACTED]".to_string()));
                } else {
                    new_map.insert(k.clone(), redact_value(v));
                }
            }
            Value::Object(new_map)
        }
        Value::Array(arr) => Value::Array(arr.iter().map(redact_value).collect()),
        other => other.clone(),
    }
}
