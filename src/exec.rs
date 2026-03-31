// Phase 6: Command execution with exit code passthrough

use crate::error::TaiError;
use std::process::Command;

/// Execute a shell command via `sh -c`, inheriting stdin/stdout/stderr.
/// Returns the child's exit code.
pub fn run_command(command: &str) -> Result<i32, TaiError> {
    let status = Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()?; // TaiError::Exec via From<io::Error>

    Ok(status.code().unwrap_or(128)) // 128 for signal termination
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn echo_returns_zero() {
        let code = run_command("echo hello").unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn false_returns_one() {
        let code = run_command("false").unwrap();
        assert_eq!(code, 1);
    }

    #[test]
    fn exit_42_returns_42() {
        let code = run_command("exit 42").unwrap();
        assert_eq!(code, 42);
    }
}
