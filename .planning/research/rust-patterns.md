# Domain Research: Rust CLI Patterns for tai

Research conducted 2026-03-26 for the `tai` (Terminal AI) project.

---

## Prior Art and Competitive Landscape

### aichat (Rust, 29k+ GitHub stars)
The dominant Rust-based terminal AI tool. Multi-provider (20+ LLMs), REPL mode, shell assistant, RAG, function calling. Uses tokio async runtime, YAML config, and streams responses.

Key takeaway: aichat is a general-purpose multi-provider tool. tai differentiates by being Claude-specific, environment-aware, and focused on the classify-then-act pipeline (propose/act/inform) rather than open-ended chat. aichat does not do environment detection or intent classification.

### shell-gpt (Python)
Python-based shell assistant. Slower startup. Does shell command generation but lacks the propose/act/inform distinction and environment-aware context assembly that tai targets.

### claude-oneshot / claude-yolo (this repo)
Simple bash wrappers around `claude -p`. No environment detection, no classification, no config system. tai replaces and supersedes these.

---

## HTTP Client: ureq vs reqwest

### Recommendation: ureq (v3.x)

| Factor | ureq 3.x | reqwest |
|--------|-----------|---------|
| Async model | Blocking (sync) | Async (tokio) or blocking feature |
| Binary size | Small (~200KB contribution) | Large (pulls in tokio, hyper, h2) |
| Compile time | Fast | Slow (heavy dep tree) |
| Streaming | `Body::as_reader()` -> `impl Read` | Async stream / blocking `Read` |
| TLS | rustls or native-tls | rustls or native-tls |
| HTTP/2 | No | Yes |
| Unsafe code | None (safe Rust) | Indirect (via hyper, tokio) |
| Maintenance | Active (v3.2.0 latest) | Active |

**Trade-offs:**

- tai makes 1-3 API calls per invocation. No concurrency requirement. ureq's blocking model is simpler and avoids the entire tokio ecosystem.
- ureq 3.x returns a `Body` that implements `std::io::Read` via `as_reader()`. Wrapping with `BufReader` enables line-by-line SSE parsing without async machinery.
- reqwest's blocking feature still pulls in tokio under the hood.
- If tai ever needs concurrent requests, reqwest would be justified. For the current one-shot design, ureq is sufficient.

### Streaming SSE with ureq

The Anthropic Messages API uses standard SSE format. Parsing pattern with ureq:

```rust
use std::io::BufRead;

let response = ureq::post(url).send(body)?;
let reader = std::io::BufReader::new(response.body_mut().as_reader());
let mut event_type = String::new();

for line in reader.lines() {
    let line = line?;
    if line.starts_with("event: ") {
        event_type = line[7..].to_string();
    } else if line.starts_with("data: ") {
        let data = &line[6..];
        // Parse JSON, extract text deltas, print to terminal
    }
}
```

No need for an SSE client library. The format is simple enough to parse with `BufRead::lines()`.

---

## YAML Parsing

### The serde_yaml Deprecation Problem

`serde_yaml` (by dtolnay) was **archived March 2024**. Replacements:

| Crate | Status | Backend | Notes |
|-------|--------|---------|-------|
| serde_yaml_ng | Active, maintained | unsafe-libyaml (migrating to libyaml-safer) | Drop-in serde_yaml replacement |
| serde-saphyr | Active | Pure Rust (saphyr) | Currently deserialization only |
| serde_yml | **Unsound, unmaintained** (RUSTSEC-2025-0068) | -- | Do NOT use |
| yaml-rust2 | Active | Pure Rust | Lower-level, no serde integration |

### Recommendation: serde_yaml_ng

Drop-in replacement, actively maintained. tai's config is simple so YAML edge cases are unlikely.

Alternative: TOML (`toml` crate) avoids the YAML deprecation mess entirely. But IDEA.md specifies YAML.

---

## CLI Argument Parsing: clap

### Recommendation: clap v4 with derive macros

```rust
use clap::Parser;

#[derive(Parser)]
#[command(name = "tai", about = "Terminal AI assistant")]
struct Cli {
    /// What to do (rest args joined as prompt)
    #[arg(trailing_var_arg = true)]
    prompt: Vec<String>,

    /// Action mode: act, propose, inform
    #[arg(long, short = 'a')]
    action: Option<ActionMode>,

    /// Complexity mode: agent, human
    #[arg(long, short = 'c')]
    complexity: Option<ComplexityMode>,

    /// Override model
    #[arg(long, short = 'm')]
    model: Option<String>,
}
```

---

## Config File + CLI Flag Layering

### Recommended Pattern: Manual merge with Option fields

```rust
use serde::Deserialize;

#[derive(Deserialize, Default)]
struct FileConfig {
    action: Option<ActionMode>,
    complexity: Option<ComplexityMode>,
    model: Option<String>,
    api_mode: Option<ApiMode>,
}

struct ResolvedConfig {
    action: ActionMode,
    complexity: ComplexityMode,
    model: String,
    api_mode: ApiMode,
}

impl ResolvedConfig {
    fn resolve(cli: &Cli, file: &FileConfig) -> Self {
        Self {
            // CLI flag > config file > hardcoded default
            action: cli.action
                .or(file.action)
                .unwrap_or(ActionMode::Propose),
            complexity: cli.complexity
                .or(file.complexity)
                .unwrap_or(ComplexityMode::Human),
            // ...
        }
    }
}
```

tai has ~6 config fields. A config framework is overkill — manual merge gives explicit control over layering and TTY-based overrides.

---

## Terminal Interaction and Prompting

### Recommendation: Hand-roll the Y/n/? prompt

```rust
use std::io::{self, Write};

fn prompt_ync() -> io::Result<char> {
    eprint!("[Y/n/?] ");
    io::stderr().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().chars().next().unwrap_or('Y'))
}
```

~10 lines of code. No dependency needed.

---

## Colored Output

### Recommendation: owo-colors

| Crate | Alloc-free | NO_COLOR | FORCE_COLOR |
|-------|-----------|----------|-------------|
| owo-colors | Yes | Yes | Yes |
| colored | No | Yes | No |

owo-colors is the modern recommendation. Zero allocations, trait-based API. Use `supports-color` crate for environment detection.

---

## TTY Detection

### std::io::IsTerminal (stdlib, since Rust 1.70)

```rust
use std::io::IsTerminal;
let is_tty = std::io::stdout().is_terminal();
let is_interactive = std::io::stdin().is_terminal();
```

The `atty` crate is **deprecated** in favor of this stdlib trait.

---

## Shelling Out to `claude -p`

```rust
use std::process::{Command, Stdio};
use std::io::{BufRead, BufReader};

fn call_claude_cli(prompt: &str, stream: bool) -> Result<(String, i32)> {
    let mut child = Command::new("claude")
        .args(["-p", prompt])
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())  // Let claude's errors pass through
        .spawn()?;

    let stdout = child.stdout.take().unwrap();
    let reader = BufReader::new(stdout);
    let mut output = String::new();

    for line in reader.lines() {
        let line = line?;
        if stream { eprintln!("{}", line); }
        output.push_str(&line);
        output.push('\n');
    }

    let status = child.wait()?;
    Ok((output, status.code().unwrap_or(1)))
}
```

Key: Use `stderr(Stdio::inherit())` to avoid deadlocks.

---

## Error Handling

### Recommendation: anyhow + thiserror

```rust
#[derive(thiserror::Error, Debug)]
pub enum TaiError {
    #[error("config file error: {0}")]
    Config(#[from] ConfigError),
    #[error("claude CLI not found in PATH")]
    ClaudeNotFound,
    #[error("API request failed: {0}")]
    Api(#[from] ureq::Error),
    #[error("command execution failed: {0}")]
    CommandFailed(#[from] std::io::Error),
}
```

Exit codes: 0-9 reserved for tai-internal status, passthrough child exit codes in act mode.

---

## Direct API Client

### Recommendation: Hand-roll with ureq

```rust
fn call_anthropic_api(api_key: &str, model: &str, prompt: &str, stream: bool) -> Result<String> {
    let body = serde_json::json!({
        "model": model,
        "max_tokens": 4096,
        "stream": stream,
        "messages": [{"role": "user", "content": prompt}]
    });

    let response = ureq::post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .send_json(&body)?;

    if stream { parse_sse_stream(response) } else { parse_json_response(response) }
}
```

Unofficial Rust Anthropic SDKs exist but are async/tokio-based. Hand-rolling with ureq keeps things sync.

---

## Recommended Cargo.toml Dependencies

```toml
[dependencies]
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml_ng = "0.10"
ureq = { version = "3", features = ["tls"] }
owo-colors = "4"
supports-color = "3"
anyhow = "1"
thiserror = "2"
which = "7"
```

Notable omissions: No tokio, no dialoguer, no crossterm, no atty. Estimated binary: ~3-5MB stripped.

---

## Common Pitfalls

1. **Blocking on subprocess stdout without reading stderr** — use `Stdio::inherit()` for stderr
2. **TTY check on wrong descriptor** — check stdin for interactivity, stdout for formatting
3. **NO_COLOR and FORCE_COLOR** — owo-colors handles automatically
4. **macOS has no /proc** — gate `/proc`-based detection behind `cfg!(target_os = "linux")`
5. **Missing config file** — treat as "all defaults", don't error
6. **Exit code truncation** — 8 bits on Unix, signals produce None
7. **$SHELL vs actual shell** — $SHELL is login shell, not necessarily current session
8. **tmux env var staleness** — document limitation rather than working around it
