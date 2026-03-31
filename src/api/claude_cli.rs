use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

use crate::api::ApiBackend;
use crate::error::TaiError;

pub struct ClaudeCliBackend;

impl ApiBackend for ClaudeCliBackend {
    fn call(&self, prompt: &str, model: &str) -> Result<String, TaiError> {
        let child = Command::new("claude")
            .args(["-p", prompt, "--model", model, "--output-format", "json"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| TaiError::ApiRequest(format!("failed to spawn claude: {}", e)))?;

        let output = child
            .wait_with_output()
            .map_err(|e| TaiError::ApiRequest(format!("failed to read claude output: {}", e)))?;

        if !output.status.success() {
            return Err(TaiError::ApiRequest(format!(
                "claude exited with status {}",
                output.status
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();

        // The claude CLI with --output-format json returns a JSON object with a `result` field.
        let parsed: serde_json::Value = serde_json::from_str(&stdout)
            .map_err(|e| TaiError::ApiRequest(format!("failed to parse claude JSON output: {}", e)))?;

        match parsed.get("result").and_then(|r| r.as_str()) {
            Some(text) => Ok(text.to_string()),
            None => {
                // Fall back to the raw stdout if no "result" field
                Ok(stdout)
            }
        }
    }

    fn call_stream(
        &self,
        prompt: &str,
        model: &str,
        out: &mut dyn std::io::Write,
    ) -> Result<String, TaiError> {
        let mut child = Command::new("claude")
            .args(["-p", prompt, "--model", model])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| TaiError::ApiRequest(format!("failed to spawn claude: {}", e)))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| TaiError::ApiRequest("failed to capture claude stdout".into()))?;

        let reader = BufReader::new(stdout);
        let mut accumulated = String::new();

        for line in reader.lines() {
            let line = line
                .map_err(|e| TaiError::ApiRequest(format!("failed to read claude output: {}", e)))?;
            out.write_all(line.as_bytes()).ok();
            out.write_all(b"\n").ok();
            accumulated.push_str(&line);
            accumulated.push('\n');
        }

        let status = child
            .wait()
            .map_err(|e| TaiError::ApiRequest(format!("failed to wait for claude: {}", e)))?;

        if !status.success() {
            return Err(TaiError::ApiRequest(format!(
                "claude exited with status {}",
                status
            )));
        }

        Ok(accumulated)
    }
}
