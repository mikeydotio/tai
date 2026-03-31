use crate::api::ApiBackend;
use crate::error::TaiError;

const API_URL: &str = "https://api.openai.com/v1/chat/completions";

pub struct OpenAiDirectBackend {
    api_key: String,
}

impl OpenAiDirectBackend {
    pub fn new(api_key: String) -> Self {
        Self { api_key }
    }

    fn build_body(&self, prompt: &str, model: &str, stream: bool) -> String {
        serde_json::json!({
            "model": model,
            "messages": [{"role": "user", "content": prompt}],
            "stream": stream
        })
        .to_string()
    }
}

impl ApiBackend for OpenAiDirectBackend {
    fn call(&self, prompt: &str, model: &str) -> Result<String, TaiError> {
        let body = self.build_body(prompt, model, false);

        let mut response = ureq::post(API_URL)
            .header("Authorization", &format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .send(body.as_bytes())
            .map_err(|e| TaiError::ApiRequest(format!("OpenAI request failed: {}", e)))?;

        let response_text = response
            .body_mut()
            .read_to_string()
            .map_err(|e| TaiError::ApiRequest(format!("failed to read response: {}", e)))?;

        let parsed: serde_json::Value = serde_json::from_str(&response_text)
            .map_err(|e| TaiError::ApiRequest(format!("failed to parse response: {}", e)))?;

        parsed
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|t| t.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| {
                TaiError::ApiRequest(format!(
                    "unexpected OpenAI response: {}",
                    &response_text[..response_text.len().min(200)]
                ))
            })
    }

    fn call_stream(
        &self,
        prompt: &str,
        model: &str,
        out: &mut dyn std::io::Write,
    ) -> Result<String, TaiError> {
        let body = self.build_body(prompt, model, true);

        let response = ureq::post(API_URL)
            .header("Authorization", &format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .send(body.as_bytes())
            .map_err(|e| TaiError::ApiRequest(format!("OpenAI request failed: {}", e)))?;

        let (_, response_body) = response.into_parts();
        let reader = response_body.into_reader();

        crate::api::sse::parse_openai_stream(reader, out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_body_format() {
        let backend = OpenAiDirectBackend::new("test-key".into());
        let body = backend.build_body("hello", "gpt-4o", false);
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed["model"], "gpt-4o");
        assert_eq!(parsed["messages"][0]["role"], "user");
        assert_eq!(parsed["messages"][0]["content"], "hello");
        assert_eq!(parsed["stream"], false);
    }

    #[test]
    fn build_body_stream() {
        let backend = OpenAiDirectBackend::new("test-key".into());
        let body = backend.build_body("hello", "gpt-4o", true);
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed["stream"], true);
    }
}
