# tai — System Design

## Overview

tai is a single-turn, synchronous Rust CLI that helps terminal users by generating shell commands and explanations informed by deep environment detection. Every response includes a command (if one is plausible) and a brief explanation. The user's configured mode (propose/inform/act) determines what tai does with the result.

There is no explicit info/action classification. The model always returns a structured JSON response with a nullable `command` field and a required `explanation` field. If a command is plausible, it's included. If not (purely conceptual question), the command is null and tai prints the explanation. The prompt enforces output discipline: answer, then stop. No follow-ups, no unsolicited suggestions.

---

## Architecture

```
User invocation
    │
    ▼
┌──────────────┐    ┌──────────────────┐
│ CLI parsing  │───▶│  Config resolve   │
│ (clap derive)│    │  defaults → toml  │
└──────────────┘    │  → tty → flags    │
                    └──────────────────┘
                            │
                            ▼
                    ┌──────────────────┐
                    │  Env detection    │
                    │  (deterministic)  │
                    └──────────────────┘
                            │
                            ▼
                    ┌──────────────────┐
                    │  Prompt assembly  │
                    │  (template + env  │
                    │   JSON + query)   │
                    └──────────────────┘
                            │
                            ▼
                    ┌──────────────────┐
                    │  API call         │
                    │  (claude CLI or   │
                    │   direct HTTP)    │
                    └──────────────────┘
                            │
                            ▼
                    ┌──────────────────┐
                    │  Response parse   │
                    │  (JSON schema)    │
                    └──────────────────┘
                            │
                            ▼
                    ┌──────────────────┐
                    │  Output/Execute   │
                    │  based on mode    │
                    └──────────────────┘
                            │
                            ▼
                        Exit code
```

The design is deliberately linear. No event loop, no concurrency, no background work.

---

## Module Structure

```
tai/
  Cargo.toml
  src/
    main.rs           -- Entry point, orchestrates the pipeline
    cli.rs            -- clap derive structs, argument parsing
    config.rs         -- FileConfig, ResolvedConfig, TOML loading, layering
    env/
      mod.rs          -- EnvContext struct, top-level detect_all()
      os.rs           -- OS/distro detection
      shell.rs        -- Parent process walk for current shell
      tty.rs          -- stdin/stdout terminal checks
      multiplexer.rs  -- tmux/screen/zellij detection
      remote.rs       -- SSH/mosh detection
      container.rs    -- Docker/Podman/LXC/K8s detection
      packages.rs     -- PATH scan for package managers
      git.rs          -- cwd, repo, branch, dirty state
    prompt.rs         -- Prompt template, variable substitution
    api/
      mod.rs          -- ApiBackend trait, factory function
      claude_cli.rs   -- claude CLI backend
      direct.rs       -- Direct Anthropic HTTP API backend
      response.rs     -- LlmResponse struct, JSON parsing
      sse.rs          -- SSE stream parser for direct API
    action.rs         -- Dispatch: propose [Y/n/?], inform, act, or print explanation
    exec.rs           -- Command execution, exit code passthrough
    error.rs          -- TaiError enum, exit codes
```

---

## Data Flow

### Step 1: CLI Parsing

```rust
#[derive(Parser)]
#[command(name = "tai", about = "Terminal AI assistant")]
struct Cli {
    /// Prompt words (joined by spaces)
    #[arg(trailing_var_arg = true)]
    prompt: Vec<String>,

    // Shortcut flags
    #[arg(long)]
    act: bool,
    #[arg(long)]
    inform: bool,
    #[arg(long)]
    agent: bool,

    // Full flags (override shortcuts)
    #[arg(long, short = 'a')]
    action: Option<ActionMode>,
    #[arg(long, short = 'c')]
    complexity: Option<ComplexityMode>,
    #[arg(long, short = 'm')]
    model: Option<String>,
    #[arg(long)]
    api_key: Option<String>,

    // Debug
    #[arg(long)]
    env_json: bool,     // Print env context JSON and exit
    #[arg(long)]
    debug: bool,        // Print prompt, raw response, etc.
}
```

If stdin is not a terminal, read stdin and append to prompt. Empty prompt after this = error (exit 64).

### Step 2: Config Resolution

Four layers, each overriding the previous:

```
Layer 1: DEFAULTS (compile-time)
    action         = Propose
    complexity     = Human
    model          = "claude-sonnet-4-20250514"
    api_mode       = Cli
    non_tty_action = Inform

Layer 2: TOML FILE
    Read $XDG_CONFIG_HOME/tai.toml (fallback: ~/.config/tai.toml)
    Missing file = skip. Parse error = warn to stderr and skip.

Layer 3: TTY OVERRIDE
    If action == Propose AND stdin is not a terminal:
        action = config.non_tty_action (default: Inform)

Layer 4: CLI FLAGS (highest priority)
    --act / --inform / --action override action
    --agent / --human / --complexity override complexity
    --model overrides model
    --api-key implies api_mode=Direct
```

```rust
#[derive(Deserialize, Default)]
struct FileConfig {
    action: Option<ActionMode>,
    complexity: Option<ComplexityMode>,
    model: Option<String>,
    api_mode: Option<ApiMode>,
    api_key: Option<String>,
    non_tty_action: Option<ActionMode>,  // what propose degrades to when non-TTY
}

struct ResolvedConfig {
    action: ActionMode,
    complexity: ComplexityMode,
    model: String,
    api_mode: ApiMode,
    api_key: Option<String>,
}
```

Example `~/.config/tai.toml`:
```toml
action = "act"
complexity = "agent"
model = "claude-sonnet-4-20250514"
non_tty_action = "act"  # power user: piped tai executes
```

**Security:** When config file contains `api_key` and is readable by group/other, warn to stderr.

### Step 3: Environment Detection

All detection is deterministic Rust code. No LLM calls. Each detector is infallible (returns None/Unknown/empty on failure). Detectors accept parameters for testability.

```rust
#[derive(Serialize)]
struct EnvContext {
    os: String,                     // "ubuntu", "macos", "arch"
    os_version: Option<String>,     // "24.04", "15.3"
    os_family: Option<String>,      // "debian", null for macOS
    kernel: Option<String>,         // uname -r
    shell: String,                  // "zsh", "bash", "fish", "unknown"
    interactive: bool,              // stdin is a TTY
    multiplexer: Option<String>,    // "tmux", "screen", "zellij"
    remote: Option<String>,         // "ssh", "mosh" (IPs stripped)
    container: Option<String>,      // "docker", "podman", "lxc", "kubernetes"
    package_managers: Vec<String>,  // ["apt", "snap"]
    cwd: Option<String>,            // None if cwd deleted
    git_repo: bool,
    git_branch: Option<String>,
    git_dirty: Option<bool>,        // None = not in repo or timed out
}
```

Detection methods per field:

| Field | Linux | macOS |
|-------|-------|-------|
| os/version/family | Parse `/etc/os-release` | `sw_vers` |
| shell | `/proc/{ppid}/exe` via readlink | `ps -p {ppid} -o comm=` |
| interactive | `std::io::IsTerminal` on stdin | same |
| multiplexer | `$TMUX`, `$STY`, `$ZELLIJ` | same |
| remote | Process tree for mosh-server, then `$SSH_CONNECTION` | same |
| container | Layered: `/.dockerenv`, `/run/.containerenv`, `$container`, `/proc/self/mountinfo` | N/A (not in containers) |
| package_managers | Scan `$PATH` dirs for known binaries | same |
| cwd | `std::env::current_dir()` | same |
| git_* | `git rev-parse` + `git status --porcelain` (200ms timeout) | same |

**Security:** SSH detection sends `"remote": "ssh"`, never raw IP addresses from `$SSH_CONNECTION`.

### Step 4: Prompt Assembly

Single monolithic template with variable substitution. The template is a `const &str` in `prompt.rs`.

```
You are tai, a terminal assistant. The user is a terminal-native person who
prefers to stay in control. Give them what they asked for, then stop. No
follow-up suggestions, no "you might also want to...", no offers of further
help. Answer and exit.

## Environment
```json
{env_json}
```

## User Request
{user_query}

{complexity_clause}

## Response Format
Respond with ONLY this JSON object, no other text:
```json
{
  "command": "<shell command, or null if no command is applicable>",
  "explanation": "<brief explanation — what the command does, or the answer to the question>"
}
```
```

**Complexity clauses:**

Human mode:
```
## Command Style
- Generate commands a human can read, verify, and understand.
- Prefer multiple sequential commands over complex one-liners.
- No nested command substitution, no long pipe chains (3+ stages).
- If the task genuinely requires complexity, explain the steps instead.
```

Agent mode:
```
## Command Style
- Generate a single command optimized for one-shot execution.
- Complex pipes, command substitution, and chaining are acceptable.
- Prioritize correctness and completeness over readability.
```

**Conditional clauses** (included only when relevant):

Container: `"The user is inside a {container} container. Host-level commands (systemctl, mount) may not work."`

Remote: `"The user is connected via {remote}. Avoid commands requiring local display servers."`

### Step 5: API Call

**ApiBackend trait:**

```rust
trait ApiBackend {
    /// Send prompt, return full response body.
    fn call(&self, prompt: &str, model: &str) -> Result<String, TaiError>;

    /// Send prompt, stream text to writer. Returns accumulated text.
    fn call_stream(&self, prompt: &str, model: &str, out: &mut dyn Write)
        -> Result<String, TaiError>;
}
```

Two implementations:

**ClaudeCliBackend:**
- `call()`: runs `claude -p <prompt> --model <model> --output-format json`, extracts `.result`
- `call_stream()`: runs `claude -p <prompt> --model <model>`, pipes stdout line-by-line
- stderr from `claude` is inherited (passes through)
- Checks for `claude` in PATH at construction time

**DirectApiBackend:**
- Holds API key
- `call()`: POST to `https://api.anthropic.com/v1/messages` with `"stream": false`
- `call_stream()`: POST with `"stream": true`, parse SSE events, write text deltas
- Hand-rolled SSE parser (~30 lines, BufRead-based)

Factory:
```rust
fn create_backend(config: &ResolvedConfig) -> Result<Box<dyn ApiBackend>, TaiError> {
    match config.api_mode {
        ApiMode::Cli => {
            which::which("claude").map_err(|_| TaiError::ClaudeNotFound)?;
            Ok(Box::new(ClaudeCliBackend))
        }
        ApiMode::Direct => {
            let key = config.api_key.as_ref()
                .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok().as_ref())
                .ok_or(TaiError::Config("direct mode requires api_key or $ANTHROPIC_API_KEY"))?;
            Ok(Box::new(DirectApiBackend::new(key.clone())))
        }
    }
}
```

### Step 6: Response Parsing

```rust
#[derive(Deserialize)]
struct LlmResponse {
    command: Option<String>,
    explanation: String,
}
```

Strict validation: if the response is not valid JSON with these fields, fail with exit code 69 (response parse error). Do not attempt to extract meaning from malformed responses.

### Step 7: Output/Execute

| command field | action=propose | action=inform | action=act |
|---|---|---|---|
| **Some(cmd)** | Show command + explanation on stderr, prompt `[Y/n/?]` | Print command to stdout (raw if piped, with explanation if TTY) | Log command to stderr, execute, passthrough exit code |
| **None** | Print explanation to stdout | Print explanation to stdout | Print explanation to stdout |

**Propose mode `[Y/n/?]` loop:**
- `Y` (or Enter): execute command, passthrough exit code
- `n`: exit with code 74 (user declined)
- `?`: make a second API call asking for a detailed explanation of the command, stream to stderr, then re-prompt `[Y/n/?]`

The `?` explanation uses the same model. Prompt:
```
Explain this command in detail. What does each part do? Any risks?
Environment: {env_json}
Command: {command}
```

**Act mode security:**
- Log every command to stderr before execution: `tai: running: <command>`
- First-run warning: the first time `--act` is used, print a one-time warning to stderr. Store acknowledgment flag in config dir.

**Inform mode output:**
- stdout is a TTY: print command + explanation, formatted
- stdout is piped: print raw command only (suitable for `$(tai --inform ...)` or piping)

### Step 8: Exit

**Execution:** `Command::new("sh").arg("-c").arg(&command)` with stdin/stdout/stderr inherited. Passthrough child exit code directly.

**stdout/stderr separation (firm rule):**
- **stdout:** Command text (inform mode), command output (act/propose-accepted), explanation text (when no command)
- **stderr:** Everything from tai itself — proposed command display, `[Y/n/?]` prompt, explanations from `?`, errors, warnings, act-mode command log

---

## Error Handling

```rust
#[derive(thiserror::Error, Debug)]
pub enum TaiError {
    #[error("no prompt provided")]
    NoPrompt,
    #[error("config error: {0}")]
    Config(String),
    #[error("claude CLI not found in PATH")]
    ClaudeNotFound,
    #[error("API request failed: {0}")]
    ApiRequest(String),
    #[error("failed to parse response: {0}")]
    ResponseParse(String),
    #[error("command execution failed: {0}")]
    Exec(#[from] std::io::Error),
}
```

**Exit codes** (BSD sysexits.h convention, 64+ to avoid collision with child codes):

| Code | Meaning |
|------|---------|
| 0 | Success |
| 64 | No prompt / usage error |
| 65 | Config file parse error |
| 69 | Claude CLI not found |
| 76 | API request failed |
| 77 | Response parse error (invalid JSON from LLM) |
| 74 | User declined in propose mode |
| 0-255 | Passthrough from child (in act/propose-accepted) |

---

## Dependencies

```toml
[dependencies]
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
ureq = { version = "3", features = ["tls"] }
owo-colors = "4"
supports-color = "3"
anyhow = "1"
thiserror = "2"
which = "7"
```

No tokio, no dialoguer, no crossterm, no atty, no SSE library, no git2. Target binary: 3-5MB stripped.

---

## Security Model

1. **LLM output is untrusted code.** Propose mode (default) is the primary guardrail.
2. **Act mode logs every command to stderr** before execution.
3. **Non-TTY propose degrades to inform** (not act) by default. Configurable.
4. **Config permissions checked** when API key is present. Warn if world-readable.
5. **API key sourcing:** CLI flag > config file > `$ANTHROPIC_API_KEY` env var. Never logged.
6. **SSH IPs stripped** from env context sent to API.
7. **Strict JSON schema validation** on LLM responses. Malformed = hard fail.
8. **`--debug` flag** shows full prompt and raw response for auditing.
9. **Prompt injection documented** as a known risk. Untrusted input should not be piped to tai.

---

## Testing Strategy

### Unit tests (in-module)

- **`env/` detectors:** Each takes parameters (file contents, env var values, PATH string) not globals. Test parsing logic with fixtures.
- **`config.rs`:** Construct `Cli` + `FileConfig` structs directly, verify merge.
- **`prompt.rs`:** Verify assembled prompt contains expected content for various configs.
- **`api/response.rs`:** Parse sample JSON (valid, malformed, missing fields).
- **`api/sse.rs`:** Parse sample SSE streams as byte slices.

### Integration tests

- **CLI args:** Run binary with various flags, verify via `--env-json` (no API call).
- **Config loading:** Write temp TOML, set `$XDG_CONFIG_HOME`, verify behavior.
- **E2E with mock API:** Create a fake `claude` script returning canned JSON, put first in PATH, run tai against it.

### Not tested

- LLM classification quality (prompt engineering, not code testing)
- `exec.rs` (thin wrapper around std::process::Command)
- Platform-specific detection on the other platform
