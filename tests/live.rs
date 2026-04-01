//! Live integration tests that hit real APIs with real keys.
//!
//! These tests are `#[ignore]`d so they don't run in normal `cargo test`.
//! Run them with:
//!
//! ```sh
//! source secrets.env && cargo test --test live -- --ignored
//! ```
//!
//! Each test reads its required API key from the environment and panics
//! with a clear message if it's missing or empty.

use tai::api::ApiBackend;
use tai::api::claude_cli::ClaudeCliBackend;
use tai::api::direct::DirectApiBackend;
use tai::api::gemini_direct::GeminiDirectBackend;
use tai::api::openai_direct::OpenAiDirectBackend;
use tai::api::response::parse_response;

/// The prompt we send to all providers. It asks for a specific JSON format
/// that matches tai's LlmResponse schema.
const TEST_PROMPT: &str = r#"Respond with ONLY this exact JSON object, no other text:
{"command": null, "explanation": "pong"}"#;

/// A prompt that should produce a command.
const TEST_PROMPT_CMD: &str = r#"Respond with ONLY this exact JSON object, no other text:
{"command": "echo hello", "explanation": "prints hello"}"#;

fn require_key(var: &str) -> String {
    match std::env::var(var) {
        Ok(val) if !val.is_empty() => val,
        _ => panic!(
            "{} is not set or empty. Fill it in secrets.env and run: source secrets.env && cargo test --test live -- --ignored",
            var
        ),
    }
}

fn has_key(var: &str) -> bool {
    std::env::var(var).map(|v| !v.is_empty()).unwrap_or(false)
}

fn has_cli(name: &str) -> bool {
    which::which(name).is_ok()
}

// ============================================================================
// Anthropic Direct API
// ============================================================================

#[test]
#[ignore]
fn anthropic_direct_call() {
    let key = require_key("ANTHROPIC_API_KEY");
    let backend = DirectApiBackend::new(key);
    let result = backend.call(TEST_PROMPT, "claude-sonnet-4-20250514");

    let text = result.expect("Anthropic direct call should succeed");
    assert!(!text.is_empty(), "response should not be empty");
    assert!(
        text.contains("pong") || text.contains("explanation"),
        "response should contain expected content: {}",
        text
    );
}

#[test]
#[ignore]
fn anthropic_direct_stream() {
    let key = require_key("ANTHROPIC_API_KEY");
    let backend = DirectApiBackend::new(key);
    let mut output = Vec::new();
    let result = backend.call_stream(TEST_PROMPT, "claude-sonnet-4-20250514", &mut output);

    let accumulated = result.expect("Anthropic streaming should succeed");
    assert!(
        !accumulated.is_empty(),
        "accumulated text should not be empty"
    );
    let written = String::from_utf8(output).expect("output should be valid UTF-8");
    assert_eq!(
        accumulated, written,
        "accumulated should match written output"
    );
}

#[test]
#[ignore]
fn anthropic_direct_full_pipeline() {
    let key = require_key("ANTHROPIC_API_KEY");
    let backend = DirectApiBackend::new(key);
    let raw = backend
        .call(TEST_PROMPT, "claude-sonnet-4-20250514")
        .expect("API call should succeed");

    let response = parse_response(&raw).expect("response should parse as LlmResponse");
    assert!(
        response.command.is_none(),
        "command should be null: {:?}",
        response
    );
    assert!(
        !response.explanation.is_empty(),
        "explanation should not be empty: {:?}",
        response
    );
}

#[test]
#[ignore]
fn anthropic_direct_command_response() {
    let key = require_key("ANTHROPIC_API_KEY");
    let backend = DirectApiBackend::new(key);
    let raw = backend
        .call(TEST_PROMPT_CMD, "claude-sonnet-4-20250514")
        .expect("API call should succeed");

    let response = parse_response(&raw).expect("response should parse as LlmResponse");
    assert!(
        response.command.is_some(),
        "command should be present: {:?}",
        response
    );
}

// ============================================================================
// Anthropic CLI
// ============================================================================

#[test]
#[ignore]
fn anthropic_cli_call() {
    if !has_cli("claude") {
        eprintln!("SKIP: claude CLI not found in PATH");
        return;
    }

    let backend = ClaudeCliBackend;
    let result = backend.call(TEST_PROMPT, "claude-sonnet-4-20250514");

    let text = result.expect("claude CLI call should succeed");
    assert!(!text.is_empty(), "response should not be empty");
}

#[test]
#[ignore]
fn anthropic_cli_stream() {
    if !has_cli("claude") {
        eprintln!("SKIP: claude CLI not found in PATH");
        return;
    }

    let backend = ClaudeCliBackend;
    let mut output = Vec::new();
    let result = backend.call_stream(TEST_PROMPT, "claude-sonnet-4-20250514", &mut output);

    let accumulated = result.expect("claude CLI streaming should succeed");
    assert!(
        !accumulated.is_empty(),
        "accumulated text should not be empty"
    );
}

#[test]
#[ignore]
fn anthropic_cli_full_pipeline() {
    if !has_cli("claude") {
        eprintln!("SKIP: claude CLI not found in PATH");
        return;
    }

    let backend = ClaudeCliBackend;
    let raw = backend
        .call(TEST_PROMPT, "claude-sonnet-4-20250514")
        .expect("claude CLI call should succeed");

    let response = parse_response(&raw).expect("CLI response should parse as LlmResponse");
    assert!(
        !response.explanation.is_empty(),
        "explanation should not be empty: {:?}",
        response
    );
}

// ============================================================================
// OpenAI Direct API
// ============================================================================

#[test]
#[ignore]
fn openai_direct_call() {
    let key = require_key("OPENAI_API_KEY");
    let backend = OpenAiDirectBackend::new(key);
    let result = backend.call(TEST_PROMPT, "gpt-4o");

    let text = result.expect("OpenAI direct call should succeed");
    assert!(!text.is_empty(), "response should not be empty");
    assert!(
        text.contains("pong") || text.contains("explanation"),
        "response should contain expected content: {}",
        text
    );
}

#[test]
#[ignore]
fn openai_direct_stream() {
    let key = require_key("OPENAI_API_KEY");
    let backend = OpenAiDirectBackend::new(key);
    let mut output = Vec::new();
    let result = backend.call_stream(TEST_PROMPT, "gpt-4o", &mut output);

    let accumulated = result.expect("OpenAI streaming should succeed");
    assert!(
        !accumulated.is_empty(),
        "accumulated text should not be empty"
    );
    let written = String::from_utf8(output).expect("output should be valid UTF-8");
    assert_eq!(
        accumulated, written,
        "accumulated should match written output"
    );
}

#[test]
#[ignore]
fn openai_direct_full_pipeline() {
    let key = require_key("OPENAI_API_KEY");
    let backend = OpenAiDirectBackend::new(key);
    let raw = backend
        .call(TEST_PROMPT, "gpt-4o")
        .expect("API call should succeed");

    let response = parse_response(&raw).expect("response should parse as LlmResponse");
    assert!(
        response.command.is_none(),
        "command should be null: {:?}",
        response
    );
    assert!(
        !response.explanation.is_empty(),
        "explanation should not be empty: {:?}",
        response
    );
}

#[test]
#[ignore]
fn openai_direct_command_response() {
    let key = require_key("OPENAI_API_KEY");
    let backend = OpenAiDirectBackend::new(key);
    let raw = backend
        .call(TEST_PROMPT_CMD, "gpt-4o")
        .expect("API call should succeed");

    let response = parse_response(&raw).expect("response should parse as LlmResponse");
    assert!(
        response.command.is_some(),
        "command should be present: {:?}",
        response
    );
}

// ============================================================================
// Gemini Direct API
// ============================================================================

#[test]
#[ignore]
fn gemini_direct_call() {
    let key = require_key("GEMINI_API_KEY");
    let backend = GeminiDirectBackend::new(key);
    let result = backend.call(TEST_PROMPT, "gemini-2.5-flash");

    let text = result.expect("Gemini direct call should succeed");
    assert!(!text.is_empty(), "response should not be empty");
    assert!(
        text.contains("pong") || text.contains("explanation"),
        "response should contain expected content: {}",
        text
    );
}

#[test]
#[ignore]
fn gemini_direct_stream() {
    let key = require_key("GEMINI_API_KEY");
    let backend = GeminiDirectBackend::new(key);
    let mut output = Vec::new();
    let result = backend.call_stream(TEST_PROMPT, "gemini-2.5-flash", &mut output);

    let accumulated = result.expect("Gemini streaming should succeed");
    assert!(
        !accumulated.is_empty(),
        "accumulated text should not be empty"
    );
    let written = String::from_utf8(output).expect("output should be valid UTF-8");
    assert_eq!(
        accumulated, written,
        "accumulated should match written output"
    );
}

#[test]
#[ignore]
fn gemini_direct_full_pipeline() {
    let key = require_key("GEMINI_API_KEY");
    let backend = GeminiDirectBackend::new(key);
    let raw = backend
        .call(TEST_PROMPT, "gemini-2.5-flash")
        .expect("API call should succeed");

    let response = parse_response(&raw).expect("response should parse as LlmResponse");
    assert!(
        response.command.is_none(),
        "command should be null: {:?}",
        response
    );
    assert!(
        !response.explanation.is_empty(),
        "explanation should not be empty: {:?}",
        response
    );
}

#[test]
#[ignore]
fn gemini_direct_command_response() {
    let key = require_key("GEMINI_API_KEY");
    let backend = GeminiDirectBackend::new(key);
    let raw = backend
        .call(TEST_PROMPT_CMD, "gemini-2.5-flash")
        .expect("API call should succeed");

    let response = parse_response(&raw).expect("response should parse as LlmResponse");
    assert!(
        response.command.is_some(),
        "command should be present: {:?}",
        response
    );
}

// ============================================================================
// Cross-provider
// ============================================================================

#[test]
#[ignore]
fn all_providers_return_parseable_response() {
    let prompt =
        r#"Respond with ONLY this JSON, no other text: {"command": null, "explanation": "ok"}"#;

    if has_key("ANTHROPIC_API_KEY") {
        let backend = DirectApiBackend::new(require_key("ANTHROPIC_API_KEY"));
        let raw = backend
            .call(prompt, "claude-sonnet-4-20250514")
            .expect("Anthropic call failed");
        parse_response(&raw).expect("Anthropic response should parse");
    }

    if has_key("OPENAI_API_KEY") {
        let backend = OpenAiDirectBackend::new(require_key("OPENAI_API_KEY"));
        let raw = backend.call(prompt, "gpt-4o").expect("OpenAI call failed");
        parse_response(&raw).expect("OpenAI response should parse");
    }

    if has_key("GEMINI_API_KEY") {
        let backend = GeminiDirectBackend::new(require_key("GEMINI_API_KEY"));
        let raw = backend
            .call(prompt, "gemini-2.5-flash")
            .expect("Gemini call failed");
        parse_response(&raw).expect("Gemini response should parse");
    }
}

#[test]
#[ignore]
fn default_models_are_accepted() {
    // Verify each provider's default model string is actually valid
    if has_key("ANTHROPIC_API_KEY") {
        let backend = DirectApiBackend::new(require_key("ANTHROPIC_API_KEY"));
        let model = tai::config::default_model(tai::cli::Provider::Anthropic);
        backend.call("Say hi", model).expect(&format!(
            "Anthropic default model '{}' should be accepted",
            model
        ));
    }

    if has_key("OPENAI_API_KEY") {
        let backend = OpenAiDirectBackend::new(require_key("OPENAI_API_KEY"));
        let model = tai::config::default_model(tai::cli::Provider::OpenAi);
        backend.call("Say hi", model).expect(&format!(
            "OpenAI default model '{}' should be accepted",
            model
        ));
    }

    if has_key("GEMINI_API_KEY") {
        let backend = GeminiDirectBackend::new(require_key("GEMINI_API_KEY"));
        let model = tai::config::default_model(tai::cli::Provider::Google);
        backend.call("Say hi", model).expect(&format!(
            "Gemini default model '{}' should be accepted",
            model
        ));
    }
}

// ============================================================================
// OAuth credential discovery
// ============================================================================

#[test]
#[ignore]
fn oauth_credentials_discovered() {
    // This test checks if Claude Code OAuth credentials exist on this machine.
    // It doesn't require an API key — it reads from ~/.claude/.credentials.json.
    let result = tai::api::oauth::discover_claude_oauth();
    if let Some(creds) = result {
        assert!(
            !creds.access_token.is_empty(),
            "OAuth access token should not be empty"
        );
        eprintln!(
            "OAuth credentials found: token starts with {}...",
            &creds.access_token[..20.min(creds.access_token.len())]
        );
    } else {
        eprintln!("SKIP: no Claude Code OAuth credentials found at ~/.claude/.credentials.json");
    }
}
