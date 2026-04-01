use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

use crate::api::ApiBackend;
use crate::error::TaiError;

pub struct CodexCliBackend;

impl ApiBackend for CodexCliBackend {
    fn call(&self, prompt: &str, model: &str) -> Result<String, TaiError> {
        let child = Command::new("codex")
            .args(["--model", model, "--quiet", prompt])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| TaiError::ApiRequest(format!("failed to spawn codex: {}", e)))?;

        let output = child
            .wait_with_output()
            .map_err(|e| TaiError::ApiRequest(format!("failed to read codex output: {}", e)))?;

        if !output.status.success() {
            return Err(TaiError::ApiRequest(format!(
                "codex exited with status {}",
                output.status
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    fn call_stream(
        &self,
        prompt: &str,
        model: &str,
        out: &mut dyn std::io::Write,
    ) -> Result<String, TaiError> {
        let mut child = Command::new("codex")
            .args(["--model", model, "--quiet", prompt])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| TaiError::ApiRequest(format!("failed to spawn codex: {}", e)))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| TaiError::ApiRequest("failed to capture codex stdout".into()))?;

        let reader = BufReader::new(stdout);
        let mut accumulated = String::new();

        for line in reader.lines() {
            let line = line
                .map_err(|e| TaiError::ApiRequest(format!("failed to read codex output: {}", e)))?;
            out.write_all(line.as_bytes()).ok();
            out.write_all(b"\n").ok();
            accumulated.push_str(&line);
            accumulated.push('\n');
        }

        let status = child
            .wait()
            .map_err(|e| TaiError::ApiRequest(format!("failed to wait for codex: {}", e)))?;

        if !status.success() {
            return Err(TaiError::ApiRequest(format!(
                "codex exited with status {}",
                status
            )));
        }

        Ok(accumulated)
    }
}
