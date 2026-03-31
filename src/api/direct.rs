use crate::api::sse;
use crate::api::ApiBackend;
use crate::error::TaiError;

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";

pub struct DirectApiBackend {
    api_key: String,
}

impl DirectApiBackend {
    pub fn new(api_key: String) -> Self {
        Self { api_key }
    }

    fn build_body(&self, prompt: &str, model: &str, stream: bool) -> String {
        let body = serde_json::json!({
            "model": model,
            "max_tokens": 4096,
            "stream": stream,
            "messages": [
                {
                    "role": "user",
                    "content": prompt
                }
            ]
        });
        body.to_string()
    }
}

impl ApiBackend for DirectApiBackend {
    fn call(&self, prompt: &str, model: &str) -> Result<String, TaiError> {
        let body = self.build_body(prompt, model, false);

        let mut response = ureq::post(API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .send(body.as_bytes())
            .map_err(|e| TaiError::ApiRequest(format!("API request failed: {}", e)))?;

        let response_text = response
            .body_mut()
            .read_to_string()
            .map_err(|e| TaiError::ApiRequest(format!("failed to read response: {}", e)))?;

        // Extract content[0].text from the Messages API response
        let parsed: serde_json::Value = serde_json::from_str(&response_text)
            .map_err(|e| {
                TaiError::ApiRequest(format!("failed to parse API response: {}", e))
            })?;

        parsed
            .get("content")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|block| block.get("text"))
            .and_then(|t| t.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| {
                TaiError::ApiRequest(format!(
                    "unexpected API response structure: {}",
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
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .send(body.as_bytes())
            .map_err(|e| TaiError::ApiRequest(format!("API request failed: {}", e)))?;

        let (_, response_body) = response.into_parts();
        let reader = response_body.into_reader();

        sse::parse_anthropic_stream(reader, out)
    }
}
