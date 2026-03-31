use crate::error::TaiError;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct LlmResponse {
    pub command: Option<String>,
    pub explanation: String,
}

/// Strip markdown code fences from around JSON content.
///
/// Handles patterns like:
/// ```json
/// {"key": "value"}
/// ```
fn strip_code_fences(raw: &str) -> &str {
    let trimmed = raw.trim();

    // Check for opening fence: ``` optionally followed by a language tag
    if let Some(rest) = trimmed.strip_prefix("```") {
        // Skip the optional language tag (e.g., "json") on the first line
        let after_lang = match rest.find('\n') {
            Some(pos) => &rest[pos + 1..],
            None => return trimmed, // malformed: no newline after opening fence
        };

        // Strip closing fence
        if let Some(content) = after_lang.strip_suffix("```") {
            return content.trim();
        }
        // Try trimmed version in case there's trailing whitespace after closing fence
        let after_lang_trimmed = after_lang.trim_end();
        if let Some(content) = after_lang_trimmed.strip_suffix("```") {
            return content.trim();
        }
    }

    trimmed
}

/// Truncate a string to at most `max_len` characters, appending "..." if truncated.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

pub fn parse_response(raw: &str) -> Result<LlmResponse, TaiError> {
    let cleaned = strip_code_fences(raw);
    serde_json::from_str::<LlmResponse>(cleaned)
        .map_err(|e| TaiError::ResponseParse(format!("{}: {}", e, truncate(raw, 200))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_json_with_command_and_explanation() {
        let raw = r#"{"command": "ls -la", "explanation": "List files in detail"}"#;
        let resp = parse_response(raw).unwrap();
        assert_eq!(resp.command, Some("ls -la".into()));
        assert_eq!(resp.explanation, "List files in detail");
    }

    #[test]
    fn valid_json_with_null_command() {
        let raw = r#"{"command": null, "explanation": "No command needed"}"#;
        let resp = parse_response(raw).unwrap();
        assert!(resp.command.is_none());
        assert_eq!(resp.explanation, "No command needed");
    }

    #[test]
    fn json_wrapped_in_code_fences() {
        let raw = "```json\n{\"command\": \"echo hi\", \"explanation\": \"Say hello\"}\n```";
        let resp = parse_response(raw).unwrap();
        assert_eq!(resp.command, Some("echo hi".into()));
        assert_eq!(resp.explanation, "Say hello");
    }

    #[test]
    fn missing_explanation_field_is_error() {
        let raw = r#"{"command": "ls"}"#;
        let err = parse_response(raw).unwrap_err();
        assert!(matches!(err, TaiError::ResponseParse(_)));
        let msg = err.to_string();
        assert!(msg.contains("explanation"), "error should mention missing field: {}", msg);
    }

    #[test]
    fn garbage_input_is_error_with_truncated_content() {
        let raw = "this is not json at all and it goes on for a very long time to test truncation behavior when the input is really really long and exceeds the truncation limit that we have set in the parse_response function which should be 200 characters or so";
        let err = parse_response(raw).unwrap_err();
        assert!(matches!(err, TaiError::ResponseParse(_)));
        let msg = err.to_string();
        assert!(msg.contains("..."), "error should contain truncated content: {}", msg);
    }
}
