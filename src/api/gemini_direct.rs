use crate::api::ApiBackend;
use crate::error::TaiError;

const BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta/models";

pub struct GeminiDirectBackend {
    api_key: String,
}

impl GeminiDirectBackend {
    pub fn new(api_key: String) -> Self {
        Self { api_key }
    }

    fn url(&self, model: &str, stream: bool) -> String {
        if stream {
            format!(
                "{}/{}:streamGenerateContent?alt=sse&key={}",
                BASE_URL, model, self.api_key
            )
        } else {
            format!(
                "{}/{}:generateContent?key={}",
                BASE_URL, model, self.api_key
            )
        }
    }

    fn build_body(&self, prompt: &str) -> String {
        serde_json::json!({
            "contents": [{"parts": [{"text": prompt}]}]
        })
        .to_string()
    }
}

impl ApiBackend for GeminiDirectBackend {
    fn call(&self, prompt: &str, model: &str) -> Result<String, TaiError> {
        let url = self.url(model, false);
        let body = self.build_body(prompt);

        let mut response = ureq::post(&url)
            .header("Content-Type", "application/json")
            .send(body.as_bytes())
            .map_err(|e| TaiError::ApiRequest(format!("Gemini request failed: {}", e)))?;

        let response_text = response
            .body_mut()
            .read_to_string()
            .map_err(|e| TaiError::ApiRequest(format!("failed to read response: {}", e)))?;

        let parsed: serde_json::Value = serde_json::from_str(&response_text)
            .map_err(|e| TaiError::ApiRequest(format!("failed to parse response: {}", e)))?;

        parsed
            .get("candidates")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("content"))
            .and_then(|p| p.get("parts"))
            .and_then(|p| p.get(0))
            .and_then(|p| p.get("text"))
            .and_then(|t| t.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| {
                TaiError::ApiRequest(format!(
                    "unexpected Gemini response: {}",
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
        let url = self.url(model, true);
        let body = self.build_body(prompt);

        let response = ureq::post(&url)
            .header("Content-Type", "application/json")
            .send(body.as_bytes())
            .map_err(|e| TaiError::ApiRequest(format!("Gemini request failed: {}", e)))?;

        let (_, response_body) = response.into_parts();
        let reader = response_body.into_reader();

        crate::api::sse::parse_gemini_stream(reader, out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_non_stream() {
        let backend = GeminiDirectBackend::new("test-key".into());
        let url = backend.url("gemini-2.5-flash", false);
        assert!(url.contains("generateContent"));
        assert!(url.contains("key=test-key"));
        assert!(!url.contains("stream"));
    }

    #[test]
    fn url_stream() {
        let backend = GeminiDirectBackend::new("test-key".into());
        let url = backend.url("gemini-2.5-flash", true);
        assert!(url.contains("streamGenerateContent"));
        assert!(url.contains("alt=sse"));
        assert!(url.contains("key=test-key"));
    }

    #[test]
    fn build_body_format() {
        let backend = GeminiDirectBackend::new("test-key".into());
        let body = backend.build_body("hello");
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed["contents"][0]["parts"][0]["text"], "hello");
    }
}
