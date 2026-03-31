use std::io::{BufRead, BufReader, Read, Write};

use crate::error::TaiError;

/// Parse Anthropic SSE stream format.
pub fn parse_anthropic_stream(
    reader: impl Read,
    out: &mut dyn Write,
) -> Result<String, TaiError> {
    let buf_reader = BufReader::new(reader);
    let mut accumulated = String::new();
    let mut event_type = String::new();

    for line in buf_reader.lines() {
        let line =
            line.map_err(|e| TaiError::ApiRequest(format!("stream read error: {}", e)))?;

        if let Some(evt) = line.strip_prefix("event: ") {
            event_type = evt.to_string();
        } else if let Some(data) = line.strip_prefix("data: ") {
            if event_type == "content_block_delta"
                && let Ok(parsed) = serde_json::from_str::<serde_json::Value>(data)
                && let Some(text) = parsed
                    .get("delta")
                    .and_then(|d| d.get("text"))
                    .and_then(|t| t.as_str())
            {
                out.write_all(text.as_bytes()).ok();
                accumulated.push_str(text);
            }
            if event_type == "message_stop" {
                break;
            }
        }
    }

    Ok(accumulated)
}

/// Parse OpenAI SSE stream format.
pub fn parse_openai_stream(
    reader: impl Read,
    out: &mut dyn Write,
) -> Result<String, TaiError> {
    let buf_reader = BufReader::new(reader);
    let mut accumulated = String::new();

    for line in buf_reader.lines() {
        let line =
            line.map_err(|e| TaiError::ApiRequest(format!("stream read error: {}", e)))?;

        if let Some(data) = line.strip_prefix("data: ") {
            if data == "[DONE]" {
                break;
            }
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(data)
                && let Some(content) = parsed
                    .get("choices")
                    .and_then(|c| c.get(0))
                    .and_then(|c| c.get("delta"))
                    .and_then(|d| d.get("content"))
                    .and_then(|t| t.as_str())
            {
                out.write_all(content.as_bytes()).ok();
                accumulated.push_str(content);
            }
        }
    }

    Ok(accumulated)
}

/// Parse Gemini SSE stream format.
pub fn parse_gemini_stream(
    reader: impl Read,
    out: &mut dyn Write,
) -> Result<String, TaiError> {
    let buf_reader = BufReader::new(reader);
    let mut accumulated = String::new();

    for line in buf_reader.lines() {
        let line =
            line.map_err(|e| TaiError::ApiRequest(format!("stream read error: {}", e)))?;

        if let Some(data) = line.strip_prefix("data: ")
            && let Ok(parsed) = serde_json::from_str::<serde_json::Value>(data)
            && let Some(text) = parsed
                .get("candidates")
                .and_then(|c| c.get(0))
                .and_then(|c| c.get("content"))
                .and_then(|p| p.get("parts"))
                .and_then(|p| p.get(0))
                .and_then(|p| p.get("text"))
                .and_then(|t| t.as_str())
        {
            out.write_all(text.as_bytes()).ok();
            accumulated.push_str(text);
        }
    }

    Ok(accumulated)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Anthropic tests
    #[test]
    fn anthropic_stream_accumulates_text() {
        let input = "\
event: content_block_delta\n\
data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}\n\
\n\
event: content_block_delta\n\
data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\" world\"}}\n\
\n\
event: message_stop\n\
data: {}\n";
        let mut output = Vec::new();
        let result = parse_anthropic_stream(input.as_bytes(), &mut output).unwrap();
        assert_eq!(result, "Hello world");
        assert_eq!(String::from_utf8(output).unwrap(), "Hello world");
    }

    #[test]
    fn anthropic_message_stop_stops_reading() {
        let input = "\
event: content_block_delta\n\
data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"first\"}}\n\
\n\
event: message_stop\n\
data: {}\n\
\n\
event: content_block_delta\n\
data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"second\"}}\n";
        let mut output = Vec::new();
        let result = parse_anthropic_stream(input.as_bytes(), &mut output).unwrap();
        assert_eq!(result, "first");
    }

    #[test]
    fn anthropic_empty_stream() {
        let mut output = Vec::new();
        let result = parse_anthropic_stream("".as_bytes(), &mut output).unwrap();
        assert_eq!(result, "");
        assert!(output.is_empty());
    }

    #[test]
    fn anthropic_malformed_data_skipped() {
        let input = "\
event: content_block_delta\n\
data: not valid json\n\
\n\
event: content_block_delta\n\
data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"ok\"}}\n\
\n\
event: message_stop\n\
data: {}\n";
        let mut output = Vec::new();
        let result = parse_anthropic_stream(input.as_bytes(), &mut output).unwrap();
        assert_eq!(result, "ok");
    }

    // OpenAI tests
    #[test]
    fn openai_stream_accumulates_content() {
        let input = "\
data: {\"id\":\"1\",\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\n\
\n\
data: {\"id\":\"2\",\"choices\":[{\"delta\":{\"content\":\" world\"}}]}\n\
\n\
data: [DONE]\n";
        let mut output = Vec::new();
        let result = parse_openai_stream(input.as_bytes(), &mut output).unwrap();
        assert_eq!(result, "Hello world");
        assert_eq!(String::from_utf8(output).unwrap(), "Hello world");
    }

    #[test]
    fn openai_done_stops_reading() {
        let input = "\
data: {\"id\":\"1\",\"choices\":[{\"delta\":{\"content\":\"first\"}}]}\n\
\n\
data: [DONE]\n\
\n\
data: {\"id\":\"2\",\"choices\":[{\"delta\":{\"content\":\"second\"}}]}\n";
        let mut output = Vec::new();
        let result = parse_openai_stream(input.as_bytes(), &mut output).unwrap();
        assert_eq!(result, "first");
    }

    #[test]
    fn openai_empty_delta_skipped() {
        let input = "\
data: {\"id\":\"1\",\"choices\":[{\"delta\":{}}]}\n\
\n\
data: {\"id\":\"2\",\"choices\":[{\"delta\":{\"content\":\"ok\"}}]}\n\
\n\
data: [DONE]\n";
        let mut output = Vec::new();
        let result = parse_openai_stream(input.as_bytes(), &mut output).unwrap();
        assert_eq!(result, "ok");
    }

    // Gemini tests
    #[test]
    fn gemini_stream_accumulates_text() {
        let input = "\
data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"Hello\"}]}}]}\n\
\n\
data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\" world\"}]}}]}\n";
        let mut output = Vec::new();
        let result = parse_gemini_stream(input.as_bytes(), &mut output).unwrap();
        assert_eq!(result, "Hello world");
        assert_eq!(String::from_utf8(output).unwrap(), "Hello world");
    }

    #[test]
    fn gemini_empty_stream() {
        let mut output = Vec::new();
        let result = parse_gemini_stream("".as_bytes(), &mut output).unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn gemini_malformed_data_skipped() {
        let input = "\
data: not valid json\n\
\n\
data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"ok\"}]}}]}\n";
        let mut output = Vec::new();
        let result = parse_gemini_stream(input.as_bytes(), &mut output).unwrap();
        assert_eq!(result, "ok");
    }
}
