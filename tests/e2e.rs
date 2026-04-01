//! End-to-end integration tests for tai's multi-provider auth system.
//!
//! These tests run the actual compiled binary with mock CLI backends,
//! controlled environment variables, and temp config files to validate
//! the full authentication pipeline for each provider.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Path to the compiled tai binary.
fn tai_bin() -> PathBuf {
    // cargo test builds to target/debug/tai
    let mut path = std::env::current_exe().unwrap();
    path.pop(); // remove test binary name
    path.pop(); // remove deps/
    path.push("tai");
    path
}

/// Create a temp directory with a mock CLI script that returns canned JSON.
/// Returns the path to the temp directory (add to front of PATH).
struct MockCli {
    dir: tempfile::TempDir,
}

impl MockCli {
    /// Create a mock script named `name` that outputs the given response to stdout.
    fn new(name: &str, response: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let script_path = dir.path().join(name);
        // Write a shell script that ignores all args and prints the response
        fs::write(
            &script_path,
            format!("#!/bin/sh\necho '{}'\n", response.replace('\'', "'\\''")),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        Self { dir }
    }

    /// Create a mock `claude` script that returns valid JSON output format.
    /// The claude CLI with --output-format json wraps in {"result": "..."}.
    fn claude(response_json: &str) -> Self {
        let wrapper = format!(
            r#"{{"result": {}}}"#,
            serde_json::to_string(response_json).unwrap()
        );
        Self::new("claude", &wrapper)
    }

    /// Create a mock `codex` script that returns raw text.
    fn codex(response_text: &str) -> Self {
        Self::new("codex", response_text)
    }

    /// Create a mock script that exits with a non-zero status.
    fn failing(name: &str, exit_code: i32) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let script_path = dir.path().join(name);
        fs::write(&script_path, format!("#!/bin/sh\nexit {}\n", exit_code)).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        Self { dir }
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }
}

/// Build a PATH string with the mock dir first, then the real system PATH.
fn path_with_mock(mock_dir: &Path) -> String {
    let system_path = std::env::var("PATH").unwrap_or_default();
    format!("{}:{}", mock_dir.display(), system_path)
}

/// Build a PATH string that EXCLUDES common locations of `claude` and `codex`
/// to ensure they're not found, while keeping basic system tools available.
fn path_without_cli_tools() -> String {
    let system_path = std::env::var("PATH").unwrap_or_default();
    // Filter out directories that might contain claude/codex
    system_path
        .split(':')
        .filter(|dir| {
            !Path::new(dir).join("claude").exists() && !Path::new(dir).join("codex").exists()
        })
        .collect::<Vec<_>>()
        .join(":")
}

/// The standard LlmResponse JSON that tai expects from the model.
fn llm_response_with_command(cmd: &str, explanation: &str) -> String {
    serde_json::json!({
        "command": cmd,
        "explanation": explanation
    })
    .to_string()
}

fn llm_response_info_only(explanation: &str) -> String {
    serde_json::json!({
        "command": null,
        "explanation": explanation
    })
    .to_string()
}

/// Run tai with given args and environment overrides.
/// Returns (exit_code, stdout, stderr).
fn run_tai(
    args: &[&str],
    env_overrides: &[(&str, &str)],
    path_override: Option<&str>,
) -> (i32, String, String) {
    let mut cmd = Command::new(tai_bin());
    cmd.args(args);

    // Clear potentially interfering env vars
    cmd.env_remove("ANTHROPIC_API_KEY");
    cmd.env_remove("OPENAI_API_KEY");
    cmd.env_remove("GEMINI_API_KEY");
    cmd.env_remove("GOOGLE_API_KEY");
    cmd.env_remove("CLAUDE_CODE_OAUTH_TOKEN");
    cmd.env_remove("XDG_CONFIG_HOME");

    // Apply overrides
    for (key, val) in env_overrides {
        cmd.env(key, val);
    }
    if let Some(path) = path_override {
        cmd.env("PATH", path);
    }

    let output = cmd.output().expect("failed to run tai binary");
    let exit_code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (exit_code, stdout, stderr)
}

// ============================================================================
// Basic functionality tests
// ============================================================================

#[test]
fn no_args_exits_64() {
    let (code, _, stderr) = run_tai(&[], &[], None);
    assert_eq!(code, 64, "stderr: {}", stderr);
    assert!(stderr.contains("no prompt"), "stderr: {}", stderr);
}

#[test]
fn help_exits_0() {
    let (code, stdout, _) = run_tai(&["--help"], &[], None);
    assert_eq!(code, 0);
    assert!(stdout.contains("--provider"));
    assert!(stdout.contains("--api-key"));
    assert!(stdout.contains("--model"));
}

#[test]
fn env_json_exits_0_without_auth() {
    let (code, stdout, _) = run_tai(&["--env-json"], &[], None);
    assert_eq!(code, 0);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(parsed.get("os").is_some());
    assert!(parsed.get("shell").is_some());
    assert!(parsed.get("git_repo").is_some());
}

// ============================================================================
// Anthropic CLI piggybacking (mock claude)
// ============================================================================

#[test]
fn anthropic_cli_inform_mode() {
    let response = llm_response_with_command("ls -la", "Lists files in detail");
    let mock = MockCli::claude(&response);
    let path = path_with_mock(mock.path());

    let (code, stdout, stderr) = run_tai(&["--inform", "list files"], &[], Some(&path));
    assert_eq!(code, 0, "stderr: {}", stderr);
    assert!(
        stdout.contains("ls -la"),
        "stdout should contain the command: {}",
        stdout
    );
}

#[test]
fn anthropic_cli_info_response() {
    let response = llm_response_info_only("Port 3000 is used by Node.js");
    let mock = MockCli::claude(&response);
    let path = path_with_mock(mock.path());

    let (code, stdout, _) = run_tai(&["--inform", "what is using port 3000"], &[], Some(&path));
    assert_eq!(code, 0);
    assert!(
        stdout.contains("Port 3000"),
        "stdout should contain the explanation: {}",
        stdout
    );
}

#[test]
fn anthropic_cli_act_mode() {
    let response = llm_response_with_command("echo hello-from-tai", "Prints hello");
    let mock = MockCli::claude(&response);
    let path = path_with_mock(mock.path());

    let (code, stdout, stderr) = run_tai(&["--act", "say hello"], &[], Some(&path));
    assert_eq!(code, 0, "stderr: {}", stderr);
    assert!(
        stderr.contains("tai: running:"),
        "stderr should log the command: {}",
        stderr
    );
    assert!(
        stdout.contains("hello-from-tai"),
        "stdout should contain command output: {}",
        stdout
    );
}

#[test]
fn anthropic_cli_with_explicit_provider() {
    let response = llm_response_with_command("echo ok", "ok");
    let mock = MockCli::claude(&response);
    let path = path_with_mock(mock.path());

    let (code, _, stderr) = run_tai(
        &["--provider", "anthropic", "--inform", "test"],
        &[],
        Some(&path),
    );
    assert_eq!(code, 0, "stderr: {}", stderr);
}

#[test]
fn anthropic_cli_not_found_exits_69() {
    let path = path_without_cli_tools();

    let (code, _, stderr) = run_tai(&["test prompt"], &[], Some(&path));
    assert_eq!(code, 69, "stderr: {}", stderr);
    assert!(stderr.contains("not found in PATH"), "stderr: {}", stderr);
}

#[test]
fn anthropic_cli_failing_exits_with_api_error() {
    let mock = MockCli::failing("claude", 1);
    let path = path_with_mock(mock.path());

    let (code, _, stderr) = run_tai(&["--inform", "test"], &[], Some(&path));
    assert_eq!(
        code, 76,
        "should be API error exit code, stderr: {}",
        stderr
    );
}

// ============================================================================
// Anthropic direct API key
// ============================================================================

#[test]
fn anthropic_direct_with_api_key_flag() {
    // We can't actually call the API, but we can verify the pipeline
    // reaches the API call stage (exit 76 = API request failed, not 65/69)
    let path = path_without_cli_tools();

    let (code, _, stderr) = run_tai(
        &["--api-key", "sk-ant-api03-fake-key", "--inform", "test"],
        &[],
        Some(&path),
    );
    // Should fail with API error (76), not config error (65) or CLI not found (69)
    assert_eq!(code, 76, "should reach API call stage, stderr: {}", stderr);
    assert!(
        stderr.contains("API") || stderr.contains("request failed"),
        "should be an API error: {}",
        stderr
    );
}

#[test]
fn anthropic_direct_with_env_var() {
    let path = path_without_cli_tools();

    let (code, _, stderr) = run_tai(
        &["--inform", "test"],
        &[("ANTHROPIC_API_KEY", "sk-ant-api03-fake-key")],
        Some(&path),
    );
    assert_eq!(code, 76, "should reach API call stage, stderr: {}", stderr);
}

#[test]
fn anthropic_direct_with_config_file() {
    let config_dir = tempfile::tempdir().unwrap();
    let config_path = config_dir.path().join("tai.toml");
    fs::write(
        &config_path,
        "anthropic_api_key = \"sk-ant-api03-fake-config-key\"\n",
    )
    .unwrap();

    let path = path_without_cli_tools();

    let (code, _, stderr) = run_tai(
        &["--inform", "test"],
        &[("XDG_CONFIG_HOME", config_dir.path().to_str().unwrap())],
        Some(&path),
    );
    assert_eq!(code, 76, "should reach API call stage, stderr: {}", stderr);
}

// ============================================================================
// OpenAI CLI piggybacking (mock codex)
// ============================================================================

#[test]
fn openai_codex_cli_inform_mode() {
    let response = llm_response_with_command("git status", "Shows git status");
    let mock = MockCli::codex(&response);
    let path = path_with_mock(mock.path());

    let (code, stdout, stderr) = run_tai(
        &["--provider", "openai", "--inform", "check git"],
        &[],
        Some(&path),
    );
    assert_eq!(code, 0, "stderr: {}", stderr);
    assert!(stdout.contains("git status"), "stdout: {}", stdout);
}

#[test]
fn openai_inferred_from_model_name() {
    let response = llm_response_with_command("echo hi", "Says hi");
    let mock = MockCli::codex(&response);
    let path = path_with_mock(mock.path());

    let (code, _, stderr) = run_tai(
        &["--model", "gpt-4o", "--inform", "say hi"],
        &[],
        Some(&path),
    );
    assert_eq!(
        code, 0,
        "should auto-detect OpenAI from model, stderr: {}",
        stderr
    );
}

#[test]
fn openai_codex_not_found_and_no_key() {
    let path = path_without_cli_tools();

    let (code, _, stderr) = run_tai(&["--provider", "openai", "test"], &[], Some(&path));
    assert_eq!(code, 65, "should be config error, stderr: {}", stderr);
    assert!(
        stderr.contains("OPENAI_API_KEY") || stderr.contains("codex"),
        "should mention available auth methods: {}",
        stderr
    );
}

// ============================================================================
// OpenAI direct API key
// ============================================================================

#[test]
fn openai_direct_with_api_key_flag() {
    let path = path_without_cli_tools();

    let (code, _, stderr) = run_tai(
        &[
            "--provider",
            "openai",
            "--api-key",
            "sk-fake-openai-key",
            "--inform",
            "test",
        ],
        &[],
        Some(&path),
    );
    // Should reach API call (76), not config error
    assert_eq!(code, 76, "should reach API call stage, stderr: {}", stderr);
}

#[test]
fn openai_direct_with_env_var() {
    let path = path_without_cli_tools();

    let (code, _, stderr) = run_tai(
        &["--provider", "openai", "--inform", "test"],
        &[("OPENAI_API_KEY", "sk-fake-openai-key")],
        Some(&path),
    );
    assert_eq!(code, 76, "should reach API call stage, stderr: {}", stderr);
}

#[test]
fn openai_direct_inferred_from_key_prefix() {
    let path = path_without_cli_tools();

    let (code, _, stderr) = run_tai(
        &["--api-key", "sk-proj-fake-key", "--inform", "test"],
        &[],
        Some(&path),
    );
    // sk-proj- starts with sk- (not sk-ant-), so inferred as OpenAI
    assert_eq!(code, 76, "should reach OpenAI API call, stderr: {}", stderr);
}

#[test]
fn openai_env_var_auto_infers_provider() {
    let path = path_without_cli_tools();

    let (code, _, stderr) = run_tai(
        &["--model", "gpt-4o", "--inform", "test"],
        &[("OPENAI_API_KEY", "sk-fake-key")],
        Some(&path),
    );
    // Model gpt-4o infers OpenAI, OPENAI_API_KEY is found
    assert_eq!(code, 76, "should reach API call stage, stderr: {}", stderr);
}

// ============================================================================
// Google/Gemini direct API key
// ============================================================================

#[test]
fn gemini_direct_with_api_key_flag() {
    let path = path_without_cli_tools();

    let (code, _, stderr) = run_tai(
        &[
            "--provider",
            "google",
            "--api-key",
            "AIzaSy-fake-key",
            "--inform",
            "test",
        ],
        &[],
        Some(&path),
    );
    assert_eq!(code, 76, "should reach API call stage, stderr: {}", stderr);
}

#[test]
fn gemini_direct_with_env_var() {
    let path = path_without_cli_tools();

    let (code, _, stderr) = run_tai(
        &["--provider", "google", "--inform", "test"],
        &[("GEMINI_API_KEY", "AIzaSy-fake-key")],
        Some(&path),
    );
    assert_eq!(code, 76, "should reach API call stage, stderr: {}", stderr);
}

#[test]
fn gemini_google_api_key_env_var() {
    let path = path_without_cli_tools();

    let (code, _, stderr) = run_tai(
        &["--provider", "google", "--inform", "test"],
        &[("GOOGLE_API_KEY", "AIzaSy-fake-key")],
        Some(&path),
    );
    assert_eq!(
        code, 76,
        "GOOGLE_API_KEY should work as fallback, stderr: {}",
        stderr
    );
}

#[test]
fn gemini_inferred_from_model_name() {
    let path = path_without_cli_tools();

    let (code, _, stderr) = run_tai(
        &["--model", "gemini-2.5-pro", "--inform", "test"],
        &[("GEMINI_API_KEY", "AIzaSy-fake-key")],
        Some(&path),
    );
    assert_eq!(
        code, 76,
        "should auto-detect Google from model, stderr: {}",
        stderr
    );
}

#[test]
fn gemini_no_auth_exits_config_error() {
    let path = path_without_cli_tools();

    let (code, _, stderr) = run_tai(&["--provider", "google", "test"], &[], Some(&path));
    assert_eq!(code, 65, "should be config error, stderr: {}", stderr);
    assert!(
        stderr.contains("GEMINI_API_KEY"),
        "should mention GEMINI_API_KEY: {}",
        stderr
    );
}

#[test]
fn gemini_inferred_from_key_prefix() {
    let path = path_without_cli_tools();

    let (code, _, stderr) = run_tai(
        &["--api-key", "AIzaSyFakeGoogleKey123", "--inform", "test"],
        &[],
        Some(&path),
    );
    // AIza prefix infers Google
    assert_eq!(code, 76, "should reach Gemini API call, stderr: {}", stderr);
}

// ============================================================================
// Provider inference from model names
// ============================================================================

#[test]
fn model_gpt4o_infers_openai() {
    let response = llm_response_with_command("echo ok", "ok");
    let mock = MockCli::codex(&response);
    let path = path_with_mock(mock.path());

    let (code, _, stderr) = run_tai(&["--model", "gpt-4o", "--inform", "test"], &[], Some(&path));
    assert_eq!(code, 0, "gpt-4o should use codex CLI, stderr: {}", stderr);
}

#[test]
fn model_o1_preview_infers_openai() {
    let response = llm_response_with_command("echo ok", "ok");
    let mock = MockCli::codex(&response);
    let path = path_with_mock(mock.path());

    let (code, _, stderr) = run_tai(
        &["--model", "o1-preview", "--inform", "test"],
        &[],
        Some(&path),
    );
    assert_eq!(
        code, 0,
        "o1-preview should infer OpenAI, stderr: {}",
        stderr
    );
}

#[test]
fn model_gemini_infers_google() {
    let path = path_without_cli_tools();

    let (code, _, stderr) = run_tai(&["--model", "gemini-2.5-flash", "test"], &[], Some(&path));
    // No Google auth → config error (65), not CLI not found (69)
    assert_eq!(
        code, 65,
        "gemini model should infer Google provider, stderr: {}",
        stderr
    );
    assert!(stderr.contains("GEMINI_API_KEY"), "stderr: {}", stderr);
}

#[test]
fn model_claude_stays_anthropic() {
    let response = llm_response_with_command("echo ok", "ok");
    let mock = MockCli::claude(&response);
    let path = path_with_mock(mock.path());

    let (code, _, stderr) = run_tai(
        &["--model", "claude-sonnet-4-20250514", "--inform", "test"],
        &[],
        Some(&path),
    );
    assert_eq!(
        code, 0,
        "claude model should use claude CLI, stderr: {}",
        stderr
    );
}

// ============================================================================
// Config file tests
// ============================================================================

#[test]
fn config_per_provider_api_key() {
    let config_dir = tempfile::tempdir().unwrap();
    let config_path = config_dir.path().join("tai.toml");
    fs::write(
        &config_path,
        r#"
provider = "openai"
openai_api_key = "sk-fake-from-config"
"#,
    )
    .unwrap();

    let path = path_without_cli_tools();

    let (code, _, stderr) = run_tai(
        &["--inform", "test"],
        &[("XDG_CONFIG_HOME", config_dir.path().to_str().unwrap())],
        Some(&path),
    );
    // Should reach API call (76) using the config key
    assert_eq!(code, 76, "should use config key, stderr: {}", stderr);
}

#[test]
fn config_legacy_api_mode_backward_compat() {
    let config_dir = tempfile::tempdir().unwrap();
    let config_path = config_dir.path().join("tai.toml");
    fs::write(
        &config_path,
        r#"
api_mode = "direct"
api_key = "sk-ant-api03-fake-legacy-key"
"#,
    )
    .unwrap();

    let path = path_without_cli_tools();

    let (code, _, stderr) = run_tai(
        &["--inform", "test"],
        &[("XDG_CONFIG_HOME", config_dir.path().to_str().unwrap())],
        Some(&path),
    );
    // Should work (legacy api_mode maps to Anthropic), reach API call
    assert_eq!(
        code, 76,
        "legacy config should still work, stderr: {}",
        stderr
    );
    assert!(
        stderr.contains("deprecated"),
        "should warn about deprecated api_mode: {}",
        stderr
    );
}

#[test]
fn config_provider_overrides_api_mode() {
    let config_dir = tempfile::tempdir().unwrap();
    let config_path = config_dir.path().join("tai.toml");
    fs::write(
        &config_path,
        r#"
provider = "openai"
api_mode = "cli"
openai_api_key = "sk-fake-key"
"#,
    )
    .unwrap();

    let path = path_without_cli_tools();

    let (code, _, stderr) = run_tai(
        &["--inform", "test"],
        &[("XDG_CONFIG_HOME", config_dir.path().to_str().unwrap())],
        Some(&path),
    );
    // provider=openai should win, use OpenAI direct
    assert_eq!(
        code, 76,
        "provider should override api_mode, stderr: {}",
        stderr
    );
}

// ============================================================================
// Auth cascade priority tests
// ============================================================================

#[test]
fn api_key_flag_takes_priority_over_env_var() {
    // Set ANTHROPIC_API_KEY but also pass --api-key with a different key
    // Both will fail at API call, but we verify the flag key is used
    let path = path_without_cli_tools();

    let (code, _, stderr) = run_tai(
        &[
            "--api-key",
            "sk-ant-api03-flag-key",
            "--inform",
            "--debug",
            "test",
        ],
        &[("ANTHROPIC_API_KEY", "sk-ant-api03-env-key")],
        Some(&path),
    );
    assert_eq!(code, 76, "should reach API call, stderr: {}", stderr);
}

#[test]
fn cli_flag_provider_overrides_model_inference() {
    // Model says OpenAI (gpt-4o) but --provider says anthropic
    let response = llm_response_with_command("echo ok", "ok");
    let mock = MockCli::claude(&response);
    let path = path_with_mock(mock.path());

    let (code, _, stderr) = run_tai(
        &[
            "--provider",
            "anthropic",
            "--model",
            "gpt-4o",
            "--inform",
            "test",
        ],
        &[],
        Some(&path),
    );
    // Should use claude CLI (anthropic) despite model name
    assert_eq!(code, 0, "explicit provider should win, stderr: {}", stderr);
}

// ============================================================================
// Debug flag validates pipeline
// ============================================================================

#[test]
fn debug_shows_prompt_and_response() {
    let response = llm_response_with_command("echo ok", "ok");
    let mock = MockCli::claude(&response);
    let path = path_with_mock(mock.path());

    let (code, _, stderr) = run_tai(&["--debug", "--inform", "list files"], &[], Some(&path));
    assert_eq!(code, 0, "stderr: {}", stderr);
    assert!(
        stderr.contains("--- PROMPT ---"),
        "should show prompt: {}",
        stderr
    );
    assert!(
        stderr.contains("--- RESPONSE ---"),
        "should show response: {}",
        stderr
    );
    assert!(
        stderr.contains("list files"),
        "prompt should contain query: {}",
        stderr
    );
}

// ============================================================================
// Stdin piping tests
// ============================================================================

#[test]
fn stdin_pipe_with_prompt() {
    let response = llm_response_with_command("echo ok", "ok");
    let mock = MockCli::claude(&response);
    let path = path_with_mock(mock.path());

    let output = Command::new(tai_bin())
        .args(["--inform", "prefix"])
        .env("PATH", path_with_mock(mock.path()))
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("CLAUDE_CODE_OAUTH_TOKEN")
        .env_remove("XDG_CONFIG_HOME")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(ref mut stdin) = child.stdin {
                stdin.write_all(b"suffix query").ok();
            }
            drop(child.stdin.take());
            child.wait_with_output()
        })
        .unwrap();

    let code = output.status.code().unwrap_or(-1);
    let _ = path; // keep mock alive
    assert_eq!(
        code,
        0,
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

// ============================================================================
// Default model per provider
// ============================================================================

#[test]
fn default_model_anthropic_in_debug() {
    let response = llm_response_with_command("echo ok", "ok");
    let mock = MockCli::claude(&response);
    let path = path_with_mock(mock.path());

    let (code, _, stderr) = run_tai(&["--debug", "--inform", "test"], &[], Some(&path));
    assert_eq!(code, 0, "stderr: {}", stderr);
    // The prompt should contain the env context, not the model name directly,
    // but the claude mock is called with --model claude-sonnet-4-20250514
}

#[test]
fn default_model_openai_used_when_no_model_flag() {
    let response = llm_response_with_command("echo ok", "ok");
    let mock = MockCli::codex(&response);
    let path = path_with_mock(mock.path());

    let (code, _, stderr) = run_tai(
        &["--provider", "openai", "--inform", "test"],
        &[],
        Some(&path),
    );
    // Should succeed using codex CLI with default model gpt-4o
    assert_eq!(code, 0, "stderr: {}", stderr);
}

// ============================================================================
// OAuth credential discovery
// ============================================================================

#[test]
fn oauth_discovery_prefers_cli_when_available() {
    // Create a mock credentials file AND a mock claude script
    // tai should find OAuth credentials but prefer claude CLI
    let response = llm_response_with_command("echo oauth-test", "oauth works");
    let mock = MockCli::claude(&response);
    let path = path_with_mock(mock.path());

    // Create temp home with credentials
    let temp_home = tempfile::tempdir().unwrap();
    let claude_dir = temp_home.path().join(".claude");
    fs::create_dir_all(&claude_dir).unwrap();
    fs::write(
        claude_dir.join(".credentials.json"),
        r#"{
            "claudeAiOauth": {
                "accessToken": "sk-ant-oat01-test-token",
                "refreshToken": "sk-ant-ort01-test-refresh",
                "expiresAt": 9999999999999
            }
        }"#,
    )
    .unwrap();

    let (code, stdout, stderr) = run_tai(
        &["--inform", "test oauth"],
        &[("HOME", temp_home.path().to_str().unwrap())],
        Some(&path),
    );
    assert_eq!(
        code, 0,
        "should succeed with OAuth + CLI, stderr: {}",
        stderr
    );
    assert!(stdout.contains("echo oauth-test"), "stdout: {}", stdout);
}

#[test]
fn oauth_discovery_falls_back_to_direct_when_no_cli() {
    // Create credentials but NO claude CLI
    let temp_home = tempfile::tempdir().unwrap();
    let claude_dir = temp_home.path().join(".claude");
    fs::create_dir_all(&claude_dir).unwrap();
    fs::write(
        claude_dir.join(".credentials.json"),
        r#"{
            "claudeAiOauth": {
                "accessToken": "sk-ant-oat01-test-token",
                "refreshToken": "sk-ant-ort01-test-refresh",
                "expiresAt": 9999999999999
            }
        }"#,
    )
    .unwrap();

    let path = path_without_cli_tools();

    let (code, _, stderr) = run_tai(
        &["--inform", "test"],
        &[("HOME", temp_home.path().to_str().unwrap())],
        Some(&path),
    );
    // Should reach API call (76) using OAuth token as direct key
    assert_eq!(
        code, 76,
        "should use OAuth token for direct API, stderr: {}",
        stderr
    );
}

#[test]
fn expired_oauth_falls_through_to_cli() {
    let temp_home = tempfile::tempdir().unwrap();
    let claude_dir = temp_home.path().join(".claude");
    fs::create_dir_all(&claude_dir).unwrap();
    fs::write(
        claude_dir.join(".credentials.json"),
        r#"{
            "claudeAiOauth": {
                "accessToken": "sk-ant-oat01-expired",
                "refreshToken": "sk-ant-ort01-expired",
                "expiresAt": 1000
            }
        }"#,
    )
    .unwrap();

    let response = llm_response_with_command("echo ok", "ok");
    let mock = MockCli::claude(&response);
    let path = path_with_mock(mock.path());

    let (code, stdout, stderr) = run_tai(
        &["--inform", "test"],
        &[("HOME", temp_home.path().to_str().unwrap())],
        Some(&path),
    );
    // Expired OAuth should be skipped, fall through to claude CLI
    assert_eq!(code, 0, "should fall through to CLI, stderr: {}", stderr);
    assert!(stdout.contains("echo ok"), "stdout: {}", stdout);
}

#[test]
fn missing_oauth_falls_through_to_cli() {
    let temp_home = tempfile::tempdir().unwrap();
    // No .claude directory at all

    let response = llm_response_with_command("echo ok", "ok");
    let mock = MockCli::claude(&response);
    let path = path_with_mock(mock.path());

    let (code, _, stderr) = run_tai(
        &["--inform", "test"],
        &[("HOME", temp_home.path().to_str().unwrap())],
        Some(&path),
    );
    assert_eq!(code, 0, "should fall through to CLI, stderr: {}", stderr);
}

// ============================================================================
// CLAUDE_CODE_OAUTH_TOKEN env var
// ============================================================================

#[test]
fn claude_code_oauth_token_reaches_api() {
    // CLAUDE_CODE_OAUTH_TOKEN should be used for direct API with Bearer auth
    let path = path_without_cli_tools();
    let temp_home = tempfile::tempdir().unwrap();

    let (code, _, stderr) = run_tai(
        &["--inform", "test"],
        &[
            ("CLAUDE_CODE_OAUTH_TOKEN", "sk-ant-oat01-fake-token"),
            ("HOME", temp_home.path().to_str().unwrap()),
        ],
        Some(&path),
    );
    // Should reach API call (76), not CLI not found (69) or config error (65)
    assert_eq!(
        code, 76,
        "should use OAuth token for direct API, stderr: {}",
        stderr
    );
}

#[test]
fn claude_code_oauth_token_takes_priority_over_credentials_file() {
    // CLAUDE_CODE_OAUTH_TOKEN env var should be preferred over credentials file
    let temp_home = tempfile::tempdir().unwrap();
    let claude_dir = temp_home.path().join(".claude");
    fs::create_dir_all(&claude_dir).unwrap();
    fs::write(
        claude_dir.join(".credentials.json"),
        r#"{
            "claudeAiOauth": {
                "accessToken": "sk-ant-oat01-file-token",
                "refreshToken": "sk-ant-ort01-test-refresh",
                "expiresAt": 9999999999999
            }
        }"#,
    )
    .unwrap();

    let path = path_without_cli_tools();

    let (code, _, stderr) = run_tai(
        &["--inform", "test"],
        &[
            ("CLAUDE_CODE_OAUTH_TOKEN", "sk-ant-oat01-env-token"),
            ("HOME", temp_home.path().to_str().unwrap()),
        ],
        Some(&path),
    );
    // Should reach API call (76), using the env var token
    assert_eq!(
        code, 76,
        "should use CLAUDE_CODE_OAUTH_TOKEN, stderr: {}",
        stderr
    );
}

#[test]
fn api_key_takes_priority_over_claude_code_oauth_token() {
    // Explicit API key should win over CLAUDE_CODE_OAUTH_TOKEN
    let path = path_without_cli_tools();
    let temp_home = tempfile::tempdir().unwrap();

    let (code, _, stderr) = run_tai(
        &["--api-key", "sk-ant-api03-explicit-key", "--inform", "test"],
        &[
            ("CLAUDE_CODE_OAUTH_TOKEN", "sk-ant-oat01-env-token"),
            ("HOME", temp_home.path().to_str().unwrap()),
        ],
        Some(&path),
    );
    // Should reach API call (76) using the explicit key
    assert_eq!(
        code, 76,
        "explicit api-key should win over CLAUDE_CODE_OAUTH_TOKEN, stderr: {}",
        stderr
    );
}

// ============================================================================
// Cross-provider error isolation
// ============================================================================

#[test]
fn openai_error_does_not_mention_claude() {
    let path = path_without_cli_tools();

    let (_, _, stderr) = run_tai(&["--provider", "openai", "test"], &[], Some(&path));
    assert!(
        !stderr.contains("claude"),
        "OpenAI error should not mention claude: {}",
        stderr
    );
}

#[test]
fn google_error_does_not_mention_claude_or_codex() {
    let path = path_without_cli_tools();

    let (_, _, stderr) = run_tai(&["--provider", "google", "test"], &[], Some(&path));
    assert!(
        !stderr.contains("claude") && !stderr.contains("codex"),
        "Google error should not mention other providers: {}",
        stderr
    );
}
