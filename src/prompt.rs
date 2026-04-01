// Phase 4: Prompt template assembly

use crate::cli::ComplexityMode;
use crate::config::ResolvedConfig;
use crate::env::EnvContext;

const SYSTEM_PREAMBLE: &str = "\
You are tai, a terminal assistant. The user is a terminal-native person who
prefers to stay in control. Give them what they asked for, then stop. No
follow-up suggestions, no \"you might also want to...\", no offers of further
help. Answer and exit.";

const HUMAN_COMPLEXITY: &str = "\
## Command Style
- Generate commands a human can read, verify, and understand.
- Prefer multiple sequential commands over complex one-liners.
- No nested command substitution, no long pipe chains (3+ stages).
- If the task genuinely requires complexity, explain the steps instead.";

const AGENT_COMPLEXITY: &str = "\
## Command Style
- Generate a single command optimized for one-shot execution.
- Complex pipes, command substitution, and chaining are acceptable.
- Prioritize correctness and completeness over readability.";

const RESPONSE_FORMAT: &str = r#"## Response Format
Respond with ONLY this JSON object, no other text:
```json
{
  "command": "<shell command, or null if no command is applicable>",
  "explanation": "<brief explanation -- what the command does, or the answer to the question>"
}
```"#;

/// Assemble the full prompt string sent to the LLM.
pub fn assemble(env: &EnvContext, query: &str, config: &ResolvedConfig) -> String {
    let env_json = serde_json::to_string_pretty(env).expect("EnvContext serialization cannot fail");

    let complexity_clause = match config.complexity {
        ComplexityMode::Human => HUMAN_COMPLEXITY,
        ComplexityMode::Agent => AGENT_COMPLEXITY,
    };

    let mut conditional_clauses = String::new();
    if let Some(ref container) = env.container {
        conditional_clauses.push_str(&format!(
            "The user is inside a {} container. Host-level commands (systemctl, mount) may not work.\n\n",
            container
        ));
    }
    if let Some(ref remote) = env.remote {
        conditional_clauses.push_str(&format!(
            "The user is connected via {}. Avoid commands requiring local display servers.\n\n",
            remote
        ));
    }

    format!(
        "{}\n\n## Environment\n```json\n{}\n```\n\n## User Request\n{}\n\n{}\n\n{}{}\n",
        SYSTEM_PREAMBLE, env_json, query, complexity_clause, conditional_clauses, RESPONSE_FORMAT,
    )
}

/// Assemble the explanation prompt for the `?` action in propose mode.
pub fn assemble_explain(env: &EnvContext, command: &str) -> String {
    let env_json = serde_json::to_string_pretty(env).expect("EnvContext serialization cannot fail");

    format!(
        "Explain this command in detail. What does each part do? Any risks?\n\n\
         Environment: {}\nCommand: {}",
        env_json, command
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{ActionMode, Provider};

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

    fn human_config() -> ResolvedConfig {
        ResolvedConfig {
            action: ActionMode::Propose,
            complexity: ComplexityMode::Human,
            model: "test-model".into(),
            provider: Provider::Anthropic,
            api_key: None,
        }
    }

    fn agent_config() -> ResolvedConfig {
        ResolvedConfig {
            action: ActionMode::Propose,
            complexity: ComplexityMode::Agent,
            model: "test-model".into(),
            provider: Provider::Anthropic,
            api_key: None,
        }
    }

    #[test]
    fn assembled_prompt_contains_env_json() {
        let env = test_env();
        let prompt = assemble(&env, "list files", &human_config());
        let env_json = serde_json::to_string_pretty(&env).unwrap();
        assert!(prompt.contains(&env_json));
    }

    #[test]
    fn assembled_prompt_contains_user_query() {
        let prompt = assemble(&test_env(), "list files", &human_config());
        assert!(prompt.contains("list files"));
    }

    #[test]
    fn assembled_prompt_contains_human_complexity_clause() {
        let prompt = assemble(&test_env(), "list files", &human_config());
        assert!(prompt.contains("Generate commands a human can read"));
        assert!(!prompt.contains("one-shot execution"));
    }

    #[test]
    fn assembled_prompt_contains_agent_complexity_clause() {
        let prompt = assemble(&test_env(), "list files", &agent_config());
        assert!(prompt.contains("one-shot execution"));
        assert!(!prompt.contains("Generate commands a human can read"));
    }

    #[test]
    fn container_clause_present_when_set() {
        let mut env = test_env();
        env.container = Some("docker".into());
        let prompt = assemble(&env, "query", &human_config());
        assert!(prompt.contains("inside a docker container"));
    }

    #[test]
    fn container_clause_absent_when_none() {
        let env = test_env();
        assert!(env.container.is_none());
        let prompt = assemble(&env, "query", &human_config());
        assert!(!prompt.contains("inside a"));
    }

    #[test]
    fn remote_clause_present_when_set() {
        let mut env = test_env();
        env.remote = Some("ssh".into());
        let prompt = assemble(&env, "query", &human_config());
        assert!(prompt.contains("connected via ssh"));
    }

    #[test]
    fn remote_clause_absent_when_none() {
        let env = test_env();
        assert!(env.remote.is_none());
        let prompt = assemble(&env, "query", &human_config());
        assert!(!prompt.contains("connected via"));
    }

    #[test]
    fn explain_prompt_contains_command_and_env() {
        let env = test_env();
        let prompt = assemble_explain(&env, "ls -la /tmp");
        assert!(prompt.contains("ls -la /tmp"));
        let env_json = serde_json::to_string_pretty(&env).unwrap();
        assert!(prompt.contains(&env_json));
        assert!(prompt.contains("Explain this command in detail"));
    }

    #[test]
    fn assembled_prompt_contains_response_format() {
        let prompt = assemble(&test_env(), "query", &human_config());
        assert!(prompt.contains("Respond with ONLY this JSON object"));
    }

    #[test]
    fn assembled_prompt_contains_system_preamble() {
        let prompt = assemble(&test_env(), "query", &human_config());
        assert!(prompt.contains("You are tai, a terminal assistant"));
    }
}
