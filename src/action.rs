// Phase 6: Action dispatch (propose/inform/act)

use crate::api::ApiBackend;
use crate::api::response::LlmResponse;
use crate::cli::ActionMode;
use crate::config::ResolvedConfig;
use crate::env::EnvContext;
use crate::error::TaiError;
use crate::exec;

use owo_colors::OwoColorize;

/// Check whether stderr supports color output.
fn use_color() -> bool {
    supports_color::on(supports_color::Stream::Stderr).is_some()
}

/// Dispatch based on action mode. Returns exit code.
pub fn dispatch(
    response: &LlmResponse,
    config: &ResolvedConfig,
    env: &EnvContext,
    backend: &dyn ApiBackend,
) -> Result<i32, TaiError> {
    // If no command, just print the explanation
    if response.command.is_none() {
        println!("{}", response.explanation);
        return Ok(0);
    }

    let cmd = response.command.as_ref().unwrap();

    match config.action {
        ActionMode::Inform => inform(cmd, &response.explanation),
        ActionMode::Act => act(cmd, &response.explanation),
        ActionMode::Propose => propose(cmd, &response.explanation, env, &config.model, backend),
    }
}

/// Inform mode: display the command (and explanation) without executing.
///
/// If stdout is a TTY, prints the command in bold green to stdout and the
/// explanation to stderr. If stdout is piped, prints only the raw command
/// to stdout so it can be captured with `$()`.
fn inform(cmd: &str, explanation: &str) -> Result<i32, TaiError> {
    if crate::env::tty::stdout_is_terminal() {
        if use_color() {
            println!("{}", cmd.bold().green());
            eprintln!("{}", explanation.dimmed());
        } else {
            println!("{}", cmd);
            eprintln!("{}", explanation);
        }
    } else {
        // Piped: raw command only, suitable for $() capture
        print!("{}", cmd);
    }
    Ok(0)
}

/// Act mode: execute the command immediately without confirmation.
///
/// On first use, prints a one-time warning and creates a sentinel file.
fn act(cmd: &str, _explanation: &str) -> Result<i32, TaiError> {
    // Check for first-run sentinel
    let sentinel_path = sentinel_path();
    if let Some(ref path) = sentinel_path
        && !path.exists()
    {
        if use_color() {
            eprintln!(
                "{}",
                "tai: warning: --act executes commands without confirmation. Use with caution."
                    .yellow()
            );
        } else {
            eprintln!(
                "tai: warning: --act executes commands without confirmation. Use with caution."
            );
        }
        // Create sentinel file (and parent dirs if needed)
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, "");
    }

    if use_color() {
        eprintln!("{} {}", "tai: running:".yellow(), cmd);
    } else {
        eprintln!("tai: running: {}", cmd);
    }

    exec::run_command(cmd)
}

/// Return the path to the act-acknowledged sentinel file.
fn sentinel_path() -> Option<std::path::PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        return Some(
            std::path::PathBuf::from(xdg)
                .join("tai")
                .join("act-acknowledged"),
        );
    }
    if let Ok(home) = std::env::var("HOME") {
        return Some(
            std::path::PathBuf::from(home)
                .join(".config")
                .join("tai")
                .join("act-acknowledged"),
        );
    }
    None
}

/// Propose mode: interactive Y/n/? loop.
///
/// Displays the command and explanation, then prompts the user for a decision.
fn propose(
    cmd: &str,
    explanation: &str,
    env: &EnvContext,
    model: &str,
    backend: &dyn ApiBackend,
) -> Result<i32, TaiError> {
    propose_with_input(
        cmd,
        explanation,
        env,
        model,
        backend,
        &mut std::io::stdin().lock(),
    )
}

/// Inner propose implementation that accepts a `BufRead` for testability.
fn propose_with_input(
    cmd: &str,
    explanation: &str,
    env: &EnvContext,
    model: &str,
    backend: &dyn ApiBackend,
    input: &mut dyn std::io::BufRead,
) -> Result<i32, TaiError> {
    // Display command and explanation on stderr
    if use_color() {
        eprintln!("{}", cmd.bold().green());
        eprintln!("{}", explanation.dimmed());
    } else {
        eprintln!("{}", cmd);
        eprintln!("{}", explanation);
    }

    // Y/n/? loop
    loop {
        eprint!("[Y/n/?] ");
        std::io::Write::flush(&mut std::io::stderr()).ok();

        let mut line = String::new();
        let bytes_read = input
            .read_line(&mut line)
            .map_err(|e| TaiError::ApiRequest(format!("failed to read input: {}", e)))?;

        // EOF (e.g., piped empty input) — treat as decline
        if bytes_read == 0 {
            return Err(TaiError::UserDeclined);
        }

        let choice = line.trim().chars().next().unwrap_or('Y');

        match choice {
            'Y' | 'y' => {
                return exec::run_command(cmd);
            }
            'n' | 'N' => {
                return Err(TaiError::UserDeclined);
            }
            '?' => {
                // Make explanation API call
                let explain_prompt = crate::prompt::assemble_explain(env, cmd);
                backend.call_stream(&explain_prompt, model, &mut std::io::stderr())?;
                eprintln!(); // newline after streamed output
                // Re-prompt
                continue;
            }
            _ => {
                // Unknown input, re-prompt
                continue;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::response::LlmResponse;
    use crate::cli::{ActionMode, ComplexityMode, Provider};

    fn test_config(action: ActionMode) -> ResolvedConfig {
        ResolvedConfig {
            action,
            complexity: ComplexityMode::Human,
            model: "test-model".into(),
            provider: Provider::Anthropic,
            api_key: None,
        }
    }

    fn test_env() -> EnvContext {
        EnvContext {
            os: "linux".into(),
            os_version: Some("22.04".into()),
            os_family: Some("debian".into()),
            kernel: Some("6.1.0".into()),
            shell: "bash".into(),
            interactive: true,
            multiplexer: None,
            remote: None,
            container: None,
            package_managers: vec!["apt".into()],
            cwd: Some("/home/user".into()),
            git_repo: false,
            git_branch: None,
            git_dirty: None,
        }
    }

    /// A stub backend for testing dispatch with no-command responses.
    struct StubBackend;

    impl ApiBackend for StubBackend {
        fn call(&self, _prompt: &str, _model: &str) -> Result<String, TaiError> {
            Ok(String::new())
        }

        fn call_stream(
            &self,
            _prompt: &str,
            _model: &str,
            _out: &mut dyn std::io::Write,
        ) -> Result<String, TaiError> {
            Ok(String::new())
        }
    }

    #[test]
    fn dispatch_no_command_returns_zero() {
        let response = LlmResponse {
            command: None,
            explanation: "No command needed".into(),
        };
        let config = test_config(ActionMode::Propose);
        let env = test_env();
        let backend = StubBackend;

        let code = dispatch(&response, &config, &env, &backend).unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn dispatch_inform_returns_zero() {
        let response = LlmResponse {
            command: Some("echo hello".into()),
            explanation: "Prints hello".into(),
        };
        let config = test_config(ActionMode::Inform);
        let env = test_env();
        let backend = StubBackend;

        let code = dispatch(&response, &config, &env, &backend).unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn dispatch_act_runs_command() {
        let response = LlmResponse {
            command: Some("true".into()),
            explanation: "Does nothing".into(),
        };
        let config = test_config(ActionMode::Act);
        let env = test_env();
        let backend = StubBackend;

        let code = dispatch(&response, &config, &env, &backend).unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn propose_yes_executes() {
        let env = test_env();
        let backend = StubBackend;
        let mut input = std::io::Cursor::new(b"y\n" as &[u8]);

        let code = propose_with_input(
            "true",
            "Does nothing",
            &env,
            "test-model",
            &backend,
            &mut input,
        )
        .unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn propose_enter_defaults_to_yes() {
        let env = test_env();
        let backend = StubBackend;
        let mut input = std::io::Cursor::new(b"\n" as &[u8]);

        let code = propose_with_input(
            "true",
            "Does nothing",
            &env,
            "test-model",
            &backend,
            &mut input,
        )
        .unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn propose_no_declines() {
        let env = test_env();
        let backend = StubBackend;
        let mut input = std::io::Cursor::new(b"n\n" as &[u8]);

        let result = propose_with_input(
            "true",
            "Does nothing",
            &env,
            "test-model",
            &backend,
            &mut input,
        );
        assert!(matches!(result, Err(TaiError::UserDeclined)));
    }

    #[test]
    fn propose_question_mark_then_yes() {
        let env = test_env();
        let backend = StubBackend;
        // First input: ?, then: y
        let mut input = std::io::Cursor::new(b"?\ny\n" as &[u8]);

        let code = propose_with_input(
            "true",
            "Does nothing",
            &env,
            "test-model",
            &backend,
            &mut input,
        )
        .unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn propose_eof_declines() {
        let env = test_env();
        let backend = StubBackend;
        // Empty input (EOF immediately)
        let mut input = std::io::Cursor::new(b"" as &[u8]);

        let result = propose_with_input(
            "true",
            "Does nothing",
            &env,
            "test-model",
            &backend,
            &mut input,
        );
        assert!(matches!(result, Err(TaiError::UserDeclined)));
    }

    #[test]
    fn propose_garbage_then_no() {
        let env = test_env();
        let backend = StubBackend;
        // Unknown input 'x', then 'n'
        let mut input = std::io::Cursor::new(b"x\nn\n" as &[u8]);

        let result = propose_with_input(
            "true",
            "Does nothing",
            &env,
            "test-model",
            &backend,
            &mut input,
        );
        assert!(matches!(result, Err(TaiError::UserDeclined)));
    }
}
