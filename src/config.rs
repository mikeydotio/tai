use crate::cli::{ActionMode, ApiMode, Cli, ComplexityMode, Provider};
use crate::error::TaiError;

pub fn default_model(provider: Provider) -> &'static str {
    match provider {
        Provider::Anthropic => "claude-sonnet-4-20250514",
        Provider::OpenAi => "gpt-4o",
        Provider::Google => "gemini-2.5-flash",
    }
}

/// Infer provider from model name or API key prefix.
pub fn infer_provider(model: &str, api_key: Option<&str>) -> Provider {
    // Model-based inference
    if model.starts_with("gpt-")
        || model.starts_with("o1-")
        || model.starts_with("o3-")
        || model.starts_with("o4-")
        || model.starts_with("chatgpt-")
    {
        return Provider::OpenAi;
    }
    if model.starts_with("gemini-") {
        return Provider::Google;
    }

    // Key-based inference
    if let Some(key) = api_key {
        if key.starts_with("sk-ant-") {
            return Provider::Anthropic;
        }
        if key.starts_with("sk-") {
            return Provider::OpenAi;
        }
        if key.starts_with("AIza") {
            return Provider::Google;
        }
    }

    Provider::Anthropic
}

#[derive(serde::Deserialize, Default, Debug)]
pub struct FileConfig {
    pub action: Option<ActionMode>,
    pub complexity: Option<ComplexityMode>,
    pub model: Option<String>,
    pub provider: Option<Provider>,
    // Per-provider API keys
    pub anthropic_api_key: Option<String>,
    pub openai_api_key: Option<String>,
    pub gemini_api_key: Option<String>,
    // Legacy fields (backward compat)
    pub api_mode: Option<ApiMode>,
    pub api_key: Option<String>,
    pub non_tty_action: Option<ActionMode>,
}

#[derive(Debug)]
pub struct ResolvedConfig {
    pub action: ActionMode,
    pub complexity: ComplexityMode,
    pub model: String,
    pub provider: Provider,
    pub api_key: Option<String>,
}

/// Resolve config from 4 layers: defaults -> TOML file -> TTY override -> CLI flags.
pub fn resolve(cli: &Cli, stdin_is_tty: bool) -> Result<ResolvedConfig, TaiError> {
    // Layer 1: Defaults
    let mut action = ActionMode::Propose;
    let mut complexity = ComplexityMode::Human;
    let mut model: Option<String> = None;
    let mut provider: Option<Provider> = None;
    let mut api_key: Option<String> = None;
    let mut non_tty_action = ActionMode::Inform;

    // Per-provider key storage from config file
    let mut anthropic_api_key: Option<String> = None;
    let mut openai_api_key: Option<String> = None;
    let mut gemini_api_key: Option<String> = None;

    // Layer 2: TOML file
    let file_config = load_file_config();
    if let Some(fc) = &file_config {
        if let Some(a) = fc.action {
            action = a;
        }
        if let Some(c) = fc.complexity {
            complexity = c;
        }
        if let Some(ref m) = fc.model {
            model = Some(m.clone());
        }
        if let Some(p) = fc.provider {
            provider = Some(p);
        }
        // Per-provider keys
        if let Some(ref k) = fc.anthropic_api_key {
            anthropic_api_key = Some(k.clone());
        }
        if let Some(ref k) = fc.openai_api_key {
            openai_api_key = Some(k.clone());
        }
        if let Some(ref k) = fc.gemini_api_key {
            gemini_api_key = Some(k.clone());
        }
        // Legacy api_key
        if let Some(ref k) = fc.api_key {
            api_key = Some(k.clone());
        }
        // Legacy api_mode → provider (with deprecation)
        if fc.api_mode.is_some() && fc.provider.is_none() {
            eprintln!("tai: warning: api_mode is deprecated, use provider = \"anthropic\" instead");
            if provider.is_none() {
                provider = Some(Provider::Anthropic);
            }
        }
        if let Some(nta) = fc.non_tty_action {
            non_tty_action = nta;
        }
    }

    // Layer 3: TTY override
    if action == ActionMode::Propose && !stdin_is_tty {
        action = non_tty_action;
    }

    // Layer 4: CLI flags
    if cli.act {
        action = ActionMode::Act;
    }
    if cli.inform {
        action = ActionMode::Inform;
    }
    if cli.agent {
        complexity = ComplexityMode::Agent;
    }

    if let Some(a) = cli.action {
        action = a;
    }
    if let Some(c) = cli.complexity {
        complexity = c;
    }
    if let Some(ref m) = cli.model {
        model = Some(m.clone());
    }
    if let Some(ref k) = cli.api_key {
        api_key = Some(k.clone());
    }
    if let Some(p) = cli.provider {
        provider = Some(p);
    }

    // Post-resolution: infer provider if not explicitly set
    let resolved_provider = provider.unwrap_or_else(|| {
        let model_ref = model.as_deref().unwrap_or("");
        infer_provider(model_ref, api_key.as_deref())
    });

    // Resolve model: use explicit model, or default for provider
    let resolved_model = model.unwrap_or_else(|| default_model(resolved_provider).to_string());

    // Resolve API key: explicit --api-key > per-provider config > legacy config > env var
    let resolved_key = api_key.or(match resolved_provider {
        Provider::Anthropic => anthropic_api_key,
        Provider::OpenAi => openai_api_key,
        Provider::Google => gemini_api_key,
    }).or_else(|| {
        // Per-provider env var
        match resolved_provider {
            Provider::Anthropic => std::env::var("ANTHROPIC_API_KEY").ok(),
            Provider::OpenAi => std::env::var("OPENAI_API_KEY").ok(),
            Provider::Google => std::env::var("GEMINI_API_KEY")
                .ok()
                .or_else(|| std::env::var("GOOGLE_API_KEY").ok()),
        }
    });

    Ok(ResolvedConfig {
        action,
        complexity,
        model: resolved_model,
        provider: resolved_provider,
        api_key: resolved_key,
    })
}

/// Locate and load the TOML config file, returning None if not found.
fn load_file_config() -> Option<FileConfig> {
    let config_path = config_path()?;

    let contents = match std::fs::read_to_string(&config_path) {
        Ok(c) => c,
        Err(_) => return None,
    };

    // Check file permissions for api_key security
    check_permissions(&config_path);

    match toml::from_str::<FileConfig>(&contents) {
        Ok(fc) => Some(fc),
        Err(e) => {
            eprintln!(
                "tai: warning: failed to parse {}: {}",
                config_path.display(),
                e
            );
            None
        }
    }
}

/// Return the path to tai.toml, using XDG_CONFIG_HOME if set.
fn config_path() -> Option<std::path::PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        let path = std::path::PathBuf::from(xdg).join("tai.toml");
        if path.exists() {
            return Some(path);
        }
    }

    if let Ok(home) = std::env::var("HOME") {
        let path = std::path::PathBuf::from(home)
            .join(".config")
            .join("tai.toml");
        if path.exists() {
            return Some(path);
        }
    }

    None
}

/// Warn if the config file is readable by group or others (when it contains api_key).
fn check_permissions(path: &std::path::Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = std::fs::metadata(path) {
            let mode = metadata.permissions().mode();
            if mode & 0o077 != 0 {
                eprintln!(
                    "tai: warning: {} has overly permissive permissions ({:o}). \
                     Consider running: chmod 600 {}",
                    path.display(),
                    mode & 0o777,
                    path.display()
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn default_cli() -> Cli {
        Cli::parse_from(["tai", "hello"])
    }

    #[test]
    fn default_resolution() {
        let cli = default_cli();
        let config = resolve(&cli, true).unwrap();
        assert_eq!(config.action, ActionMode::Propose);
        assert_eq!(config.complexity, ComplexityMode::Human);
        assert_eq!(config.model, "claude-sonnet-4-20250514");
        assert_eq!(config.provider, Provider::Anthropic);
        assert!(config.api_key.is_none());
    }

    #[test]
    fn cli_act_flag_overrides() {
        let cli = Cli::parse_from(["tai", "--act", "do", "it"]);
        let config = resolve(&cli, true).unwrap();
        assert_eq!(config.action, ActionMode::Act);
    }

    #[test]
    fn cli_agent_flag_overrides() {
        let cli = Cli::parse_from(["tai", "--agent", "query"]);
        let config = resolve(&cli, true).unwrap();
        assert_eq!(config.complexity, ComplexityMode::Agent);
    }

    #[test]
    fn toml_parsing() {
        let fc: FileConfig =
            toml::from_str("action = \"act\"\ncomplexity = \"agent\"").unwrap();
        assert_eq!(fc.action, Some(ActionMode::Act));
        assert_eq!(fc.complexity, Some(ComplexityMode::Agent));
    }

    #[test]
    fn tty_override_propose_to_inform() {
        let cli = default_cli();
        let config = resolve(&cli, false).unwrap();
        assert_eq!(config.action, ActionMode::Inform);
    }

    #[test]
    fn tty_override_does_not_apply_when_tty() {
        let cli = default_cli();
        let config = resolve(&cli, true).unwrap();
        assert_eq!(config.action, ActionMode::Propose);
    }

    #[test]
    fn api_key_passes_through() {
        let cli = Cli::parse_from(["tai", "--api-key", "sk-ant-test", "hello"]);
        let config = resolve(&cli, true).unwrap();
        assert_eq!(config.api_key, Some("sk-ant-test".into()));
        assert_eq!(config.provider, Provider::Anthropic);
    }

    #[test]
    fn full_action_flag_overrides_shortcut() {
        let cli = Cli::parse_from(["tai", "--act", "--action", "inform", "hello"]);
        let config = resolve(&cli, true).unwrap();
        assert_eq!(config.action, ActionMode::Inform);
    }

    #[test]
    fn model_flag_overrides_default() {
        let cli = Cli::parse_from(["tai", "--model", "claude-opus-4-20250514", "hello"]);
        let config = resolve(&cli, true).unwrap();
        assert_eq!(config.model, "claude-opus-4-20250514");
    }

    #[test]
    fn toml_full_config() {
        let toml_str = r#"
action = "act"
complexity = "agent"
model = "custom-model"
api_mode = "direct"
api_key = "sk-secret"
non_tty_action = "act"
"#;
        let fc: FileConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(fc.action, Some(ActionMode::Act));
        assert_eq!(fc.complexity, Some(ComplexityMode::Agent));
        assert_eq!(fc.model, Some("custom-model".into()));
        assert_eq!(fc.api_mode, Some(ApiMode::Direct));
        assert_eq!(fc.api_key, Some("sk-secret".into()));
        assert_eq!(fc.non_tty_action, Some(ActionMode::Act));
    }

    // Phase 1: Provider inference tests
    #[test]
    fn infer_provider_from_gpt_model() {
        assert_eq!(infer_provider("gpt-4o", None), Provider::OpenAi);
    }

    #[test]
    fn infer_provider_from_o1_model() {
        assert_eq!(infer_provider("o1-preview", None), Provider::OpenAi);
    }

    #[test]
    fn infer_provider_from_gemini_model() {
        assert_eq!(infer_provider("gemini-2.5-pro", None), Provider::Google);
    }

    #[test]
    fn infer_provider_from_claude_model() {
        assert_eq!(
            infer_provider("claude-sonnet-4-20250514", None),
            Provider::Anthropic
        );
    }

    #[test]
    fn infer_provider_from_anthropic_key() {
        assert_eq!(
            infer_provider("custom", Some("sk-ant-api03-xxx")),
            Provider::Anthropic
        );
    }

    #[test]
    fn infer_provider_from_openai_key() {
        assert_eq!(
            infer_provider("custom", Some("sk-proj-xxx")),
            Provider::OpenAi
        );
    }

    #[test]
    fn infer_provider_from_google_key() {
        assert_eq!(
            infer_provider("custom", Some("AIzaSyXXX")),
            Provider::Google
        );
    }

    #[test]
    fn infer_provider_defaults_to_anthropic() {
        assert_eq!(infer_provider("custom-model", None), Provider::Anthropic);
    }

    #[test]
    fn default_model_per_provider() {
        assert_eq!(default_model(Provider::Anthropic), "claude-sonnet-4-20250514");
        assert_eq!(default_model(Provider::OpenAi), "gpt-4o");
        assert_eq!(default_model(Provider::Google), "gemini-2.5-flash");
    }

    #[test]
    fn provider_flag_overrides_inference() {
        let cli =
            Cli::parse_from(["tai", "--provider", "openai", "--model", "claude-opus-4", "hello"]);
        let config = resolve(&cli, true).unwrap();
        assert_eq!(config.provider, Provider::OpenAi);
        assert_eq!(config.model, "claude-opus-4");
    }

    #[test]
    fn model_infers_provider_when_no_explicit() {
        let cli = Cli::parse_from(["tai", "--model", "gpt-4o", "hello"]);
        let config = resolve(&cli, true).unwrap();
        assert_eq!(config.provider, Provider::OpenAi);
    }

    #[test]
    fn backward_compat_legacy_api_mode() {
        let fc: FileConfig =
            toml::from_str("api_mode = \"direct\"\napi_key = \"sk-ant-xxx\"").unwrap();
        assert_eq!(fc.api_mode, Some(ApiMode::Direct));
        assert_eq!(fc.api_key, Some("sk-ant-xxx".into()));
    }

    #[test]
    fn new_provider_config_parsing() {
        let fc: FileConfig =
            toml::from_str("provider = \"openai\"\nopenai_api_key = \"sk-xxx\"").unwrap();
        assert_eq!(fc.provider, Some(Provider::OpenAi));
        assert_eq!(fc.openai_api_key, Some("sk-xxx".into()));
    }

    #[test]
    fn gemini_model_uses_gemini_default() {
        let cli = Cli::parse_from(["tai", "--provider", "google", "hello"]);
        let config = resolve(&cli, true).unwrap();
        assert_eq!(config.provider, Provider::Google);
        assert_eq!(config.model, "gemini-2.5-flash");
    }
}
