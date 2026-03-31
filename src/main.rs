use clap::Parser;
use std::io::Read;

use tai::action;
use tai::api;
use tai::cli;
use tai::config;
use tai::env;
use tai::error;
use tai::prompt;

fn main() {
    let exit_code = match run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("tai: {}", e);
            e.exit_code()
        }
    };
    std::process::exit(exit_code);
}

fn run() -> Result<i32, error::TaiError> {
    // 1. Parse CLI
    let cli_args = cli::Cli::parse();

    // 2. Build prompt (join args + read stdin if piped)
    let prompt = build_prompt(&cli_args)?;

    // 3. Handle --env-json early (no prompt required)
    let stdin_tty = env::tty::stdin_is_terminal();
    if cli_args.env_json {
        let env_ctx = env::detect_all();
        println!("{}", serde_json::to_string_pretty(&env_ctx).unwrap());
        return Ok(0);
    }

    // 4. Require a prompt for all other operations
    if prompt.is_empty() {
        return Err(error::TaiError::NoPrompt);
    }

    // 5. Resolve config
    let config = config::resolve(&cli_args, stdin_tty)?;

    // 6. Detect environment
    let env_ctx = env::detect_all();

    // 7. Assemble prompt
    let full_prompt = prompt::assemble(&env_ctx, &prompt, &config);

    // 8. Debug output: show prompt
    if cli_args.debug {
        eprintln!("--- PROMPT ---\n{}\n--- END ---", full_prompt);
    }

    // 9. Create API backend
    let backend = api::create_backend(&config)?;

    // 10. Make API call
    let raw_response = backend.call(&full_prompt, &config.model)?;

    // 11. Debug output: show raw response
    if cli_args.debug {
        eprintln!("--- RESPONSE ---\n{}\n--- END ---", raw_response);
    }

    // 12. Parse response
    let response = api::response::parse_response(&raw_response)?;

    // 13. Dispatch action
    action::dispatch(&response, &config, &env_ctx, backend.as_ref())
}

/// Build the prompt string from CLI args and optionally stdin.
fn build_prompt(cli: &cli::Cli) -> Result<String, error::TaiError> {
    let mut prompt = cli.prompt.join(" ");

    // If stdin is not a terminal, read it and append to prompt
    if !env::tty::stdin_is_terminal() {
        let mut stdin_content = String::new();
        // Limit to 100KB to prevent accidental huge reads
        std::io::stdin()
            .take(100 * 1024)
            .read_to_string(&mut stdin_content)
            .map_err(|e| error::TaiError::Config(format!("failed to read stdin: {}", e)))?;
        let stdin_content = stdin_content.trim();
        if !stdin_content.is_empty() {
            if !prompt.is_empty() {
                prompt.push(' ');
            }
            prompt.push_str(stdin_content);
        }
    }

    Ok(prompt)
}
