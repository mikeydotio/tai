use clap::Parser;

#[derive(clap::ValueEnum, serde::Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ActionMode {
    Propose,
    Inform,
    Act,
}

#[derive(clap::ValueEnum, serde::Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ComplexityMode {
    Human,
    Agent,
}

#[derive(clap::ValueEnum, serde::Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ApiMode {
    Cli,
    Direct,
}

#[derive(clap::ValueEnum, serde::Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Anthropic,
    #[serde(alias = "openai")]
    #[value(alias = "openai")]
    OpenAi,
    Google,
}

#[derive(Parser, Debug)]
#[command(name = "tai", about = "Terminal AI assistant")]
pub struct Cli {
    /// Prompt words (joined by spaces)
    #[arg(trailing_var_arg = true)]
    pub prompt: Vec<String>,

    // Shortcut flags
    #[arg(long)]
    pub act: bool,
    #[arg(long)]
    pub inform: bool,
    #[arg(long)]
    pub agent: bool,

    // Full flags (override shortcuts)
    #[arg(long, short = 'a')]
    pub action: Option<ActionMode>,
    #[arg(long, short = 'c')]
    pub complexity: Option<ComplexityMode>,
    #[arg(long, short = 'm')]
    pub model: Option<String>,
    #[arg(long)]
    pub api_key: Option<String>,
    #[arg(long)]
    pub provider: Option<Provider>,

    // History
    /// Show recent command history
    #[arg(long)]
    pub history: bool,

    // Debug
    #[arg(long)]
    pub env_json: bool,
    #[arg(long)]
    pub debug: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_prompt_words() {
        let cli = Cli::parse_from(["tai", "hello", "world"]);
        assert_eq!(cli.prompt, vec!["hello", "world"]);
    }

    #[test]
    fn parse_act_flag() {
        let cli = Cli::parse_from(["tai", "--act", "do", "something"]);
        assert!(cli.act);
        assert_eq!(cli.prompt, vec!["do", "something"]);
    }

    #[test]
    fn parse_inform_and_agent() {
        let cli = Cli::parse_from(["tai", "--inform", "--agent", "query"]);
        assert!(cli.inform);
        assert!(cli.agent);
        assert_eq!(cli.prompt, vec!["query"]);
    }

    #[test]
    fn parse_full_flags() {
        let cli = Cli::parse_from([
            "tai",
            "--action",
            "act",
            "--complexity",
            "agent",
            "--model",
            "opus",
            "prompt",
        ]);
        assert_eq!(cli.action, Some(ActionMode::Act));
        assert_eq!(cli.complexity, Some(ComplexityMode::Agent));
        assert_eq!(cli.model, Some("opus".into()));
    }

    #[test]
    fn parse_api_key_flag() {
        let cli = Cli::parse_from(["tai", "--api-key", "sk-123", "hello"]);
        assert_eq!(cli.api_key, Some("sk-123".into()));
    }

    #[test]
    fn parse_debug_flags() {
        let cli = Cli::parse_from(["tai", "--env-json", "--debug", "test"]);
        assert!(cli.env_json);
        assert!(cli.debug);
    }
}
