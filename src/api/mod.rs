use crate::cli::Provider;
use crate::config::ResolvedConfig;
use crate::error::TaiError;

pub mod claude_cli;
pub mod codex_cli;
pub mod direct;
pub mod gemini_direct;
pub mod oauth;
pub mod openai_direct;
pub mod response;
pub mod sse;

pub trait ApiBackend {
    /// Send prompt, return full response body text.
    fn call(&self, prompt: &str, model: &str) -> Result<String, TaiError>;

    /// Send prompt, stream text to writer. Returns accumulated text.
    fn call_stream(
        &self,
        prompt: &str,
        model: &str,
        out: &mut dyn std::io::Write,
    ) -> Result<String, TaiError>;
}

pub fn create_backend(config: &ResolvedConfig) -> Result<Box<dyn ApiBackend>, TaiError> {
    match config.provider {
        Provider::Anthropic => {
            if let Some(ref key) = config.api_key {
                Ok(Box::new(direct::DirectApiBackend::new(key.clone())))
            } else if let Ok(token) = std::env::var("CLAUDE_CODE_OAUTH_TOKEN") {
                Ok(Box::new(direct::DirectApiBackend::with_bearer(token)))
            } else if let Some(oauth_creds) = oauth::discover_claude_oauth() {
                // OAuth found — prefer CLI if available (OAuth tokens may not
                // work directly with api.anthropic.com)
                if which::which("claude").is_ok() {
                    Ok(Box::new(claude_cli::ClaudeCliBackend))
                } else {
                    // Try direct API with OAuth token as fallback
                    Ok(Box::new(direct::DirectApiBackend::with_bearer(
                        oauth_creds.access_token,
                    )))
                }
            } else if which::which("claude").is_ok() {
                Ok(Box::new(claude_cli::ClaudeCliBackend))
            } else {
                Err(TaiError::CliNotFound("claude".into()))
            }
        }
        Provider::OpenAi => {
            if let Some(ref key) = config.api_key {
                Ok(Box::new(openai_direct::OpenAiDirectBackend::new(
                    key.clone(),
                )))
            } else if which::which("codex").is_ok() {
                Ok(Box::new(codex_cli::CodexCliBackend))
            } else {
                Err(TaiError::Config(
                    "no OpenAI auth: set OPENAI_API_KEY or install codex CLI".into(),
                ))
            }
        }
        Provider::Google => {
            if let Some(ref key) = config.api_key {
                Ok(Box::new(gemini_direct::GeminiDirectBackend::new(
                    key.clone(),
                )))
            } else {
                Err(TaiError::Config(
                    "no Google auth: set GEMINI_API_KEY".into(),
                ))
            }
        }
    }
}
