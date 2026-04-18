# tai

Fast, context-aware terminal AI assistant. Type a request in plain English,
get a shell command back — proposed for confirmation, printed for capture, or
run immediately.

`tai` detects your environment (OS, shell, git repo, container, multiplexer,
SSH session, package managers) and ships that context to the model alongside
your prompt, so the command it returns fits the system you're actually on.
Backed by Claude, OpenAI, or Gemini.

## Install

Requires a recent Rust toolchain.

```bash
cargo build --release
cp target/release/tai ~/.local/bin/   # or anywhere on PATH
```

Or install with cargo directly:

```bash
cargo install --path .
```

You also need either the `claude` CLI on `PATH`, or an API key in your
environment (see **Auth** below).

## Quick start

```bash
# Default: propose a command, prompt Y/n before running
tai find files larger than 100MB in this repo

# Print only — useful inside command substitution
git checkout $(tai --inform branch matching feat/auth)

# Run immediately, no confirmation
tai --act compress all PNGs in this directory
```

## Action modes

`tai` has three modes for what to do with the generated command:

| Mode      | Flag                | Behavior                                                          |
| --------- | ------------------- | ----------------------------------------------------------------- |
| `propose` | (default)           | Show command + explanation, prompt `Y/n/?`, execute on `Y`.       |
| `inform`  | `--inform`          | Print command on stdout, explanation on stderr. Don't execute.    |
| `act`     | `--act`             | Execute immediately. First-use shows a one-time warning.          |

The full form is `-a {propose,inform,act}` / `--action ...` and overrides the
shortcut flags. When stdin is piped (not a TTY) and the resolved mode is
`propose`, `tai` automatically falls back to `inform` so command substitution
works cleanly. Override that fallback with `non_tty_action` in config.

## Complexity modes

| Mode    | Flag       | Use for                                                      |
| ------- | ---------- | ------------------------------------------------------------ |
| `human` | (default)  | Single shell commands a person would reasonably type.        |
| `agent` | `--agent`  | Complex pipelines, scripts, multi-step commands.             |

Full form: `-c {human,agent}` / `--complexity ...`.

## Providers & auth

The provider is inferred from `--model` / `--api-key` prefixes, or set
explicitly with `--provider {anthropic,openai,google}`.

| Provider    | Default model              | Env var                              |
| ----------- | -------------------------- | ------------------------------------ |
| `anthropic` | `claude-sonnet-4-20250514` | `ANTHROPIC_API_KEY`                  |
| `openai`    | `gpt-4o`                   | `OPENAI_API_KEY`                     |
| `google`    | `gemini-2.5-flash`         | `GEMINI_API_KEY` / `GOOGLE_API_KEY`  |

API key precedence (highest first): `--api-key` → per-provider config key →
env var. For Anthropic, you can also use the `claude` CLI (which handles its
own auth) or set `CLAUDE_CODE_OAUTH_TOKEN`.

## CLI reference

```
tai [OPTIONS] [PROMPT...]
```

| Flag                            | Description                                                  |
| ------------------------------- | ------------------------------------------------------------ |
| `[PROMPT...]`                   | Words joined into the prompt. Stdin (≤100 KB) is appended when not a TTY. |
| `--act`                         | Shortcut for `--action act`.                                 |
| `--inform`                      | Shortcut for `--action inform`.                              |
| `--agent`                       | Shortcut for `--complexity agent`.                           |
| `-a, --action {propose,inform,act}` | Set action mode (overrides shortcut).                    |
| `-c, --complexity {human,agent}`    | Set complexity mode (overrides shortcut).                |
| `-m, --model <NAME>`            | Override the model name.                                     |
| `--provider {anthropic,openai,google}` | Force a provider.                                     |
| `--api-key <KEY>`               | Use this API key (overrides config and env).                 |
| `--history`                     | Print recent invocations from the history DB and exit.       |
| `--env-json`                    | Print detected environment as JSON and exit.                 |
| `--debug`                       | Print full prompt and raw response to stderr.                |

## Configuration

`tai` reads `$XDG_CONFIG_HOME/tai.toml`, falling back to `~/.config/tai.toml`.
All fields are optional.

```toml
action     = "propose"            # propose | inform | act
complexity = "human"              # human | agent
model      = "claude-sonnet-4-20250514"
provider   = "anthropic"          # anthropic | openai | google

# Per-provider keys (preferred)
anthropic_api_key = "sk-ant-..."
openai_api_key    = "sk-..."
gemini_api_key    = "AIza..."

# When stdin is piped and action would be "propose", use this instead
non_tty_action = "inform"
```

Resolution layers, lowest to highest precedence:

1. Built-in defaults
2. TOML file
3. TTY override (`non_tty_action` when stdin is piped)
4. CLI flags

`tai` will warn on stderr if the config file holds an API key and is readable
by group or others.

## History

Every invocation is appended to a SQLite database at
`$XDG_DATA_HOME/tai/history.db` (or `~/.local/share/tai/history.db`):
timestamp, prompt, generated command, mode, model, provider, the user's
choice, and the exit code. View recent entries:

```bash
tai --history
```

History writes are best-effort — failures are logged to stderr but never
abort the run.

## Environment context

Inspect what `tai` sees about your system:

```bash
tai --env-json
```

Detection reads `/etc/os-release`, `/proc/...`, walks the parent process
chain to identify the shell, runs `git` for repo state, scans `PATH` for
package managers, and checks env vars for SSH / multiplexer / container
hints. Every check has a fallback; detection never fails.

## Exit codes

| Code | Meaning                                         |
| ---- | ----------------------------------------------- |
| `0`  | Success                                         |
| `1`  | Command execution error (or signal)             |
| `64` | No prompt provided                              |
| `65` | Config error                                    |
| `69` | Required CLI (e.g., `claude`) not found in PATH |
| `74` | User declined at the propose prompt             |
| `76` | API request failed                              |
| `77` | Failed to parse model response                  |

When `tai` executes a command, that command's exit code is passed through.

## Architecture

```
src/
├── main.rs        # Orchestrates the request pipeline
├── cli.rs         # clap-derived flag parser
├── config.rs      # 4-layer config resolution
├── prompt.rs      # System preamble + env JSON + user query
├── env/           # Infallible environment detection
├── api/           # ApiBackend trait + per-provider backends
├── action.rs      # Dispatch by mode (propose/inform/act)
├── exec.rs        # Run via `sh -c`, pass through exit code
├── history.rs     # SQLite append + display
└── error.rs       # TaiError enum mapped to exit codes
```

## Development

```bash
cargo test                # unit + e2e tests
cargo test --test e2e     # integration tests with mock backends
cargo test --test live    # real API tests (needs auth)
tai --debug <prompt>      # inspect the full prompt and response
```

## License

MIT. (Add a `LICENSE` file and `license = "MIT"` to `Cargo.toml` to make
this official.)
