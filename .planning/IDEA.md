# tai (Terminal AI)

## Vision
A fast, context-aware terminal assistant that classifies user requests and either answers questions or formulates and optionally executes shell commands — all with configurable safety, complexity, and model preferences. Built as a tight Rust binary with deterministic environment detection and multi-model Claude pipeline.

## Problem Statement
Terminal users frequently need help with command syntax, system tasks, and quick automation. Existing solutions either require leaving the terminal (web search), lack environment awareness (generic LLM wrappers), or don't distinguish between "tell me" and "do it for me" requests. tai bridges this gap by understanding the user's environment and intent, then acting appropriately.

## Target Users
- Power terminal users who want fast, context-aware command help
- Developers working across multiple environments (local, SSH, containers, tmux)
- Script/tool authors who want to pipe AI-generated commands into automation
- Anyone who knows *what* they want to do but not the exact *how*

## Key Requirements
- [ ] Deterministic environment detection: OS, shell, interactive vs piped, tmux/screen, SSH/mosh, container (Docker/Podman/LXC), package managers (apt/brew/dnf/pacman), cwd + git repo info
- [ ] Environment context assembled as JSON and injected into a monolithic prompt template
- [ ] Request classification: information request vs action request (single Claude call, optimized)
- [ ] **command-action** setting with three modes:
  - `act` — execute the formulated command immediately, passthrough stdout/stderr
  - `propose` — show command, prompt `[Y/n/?]`, execute on Y, explain on `?` then re-prompt
  - `inform` — print command and exit (raw if piped, brief context if TTY)
- [ ] **command-complexity** setting with two modes:
  - `agent` — arbitrarily complex, chained, interpolated commands optimized for one-shot execution
  - `human` — straightforward commands, no concatenation, multiple sequential commands preferred over complex one-liners
- [ ] Config file at `~/.config/tai.yaml` for defaults; CLI flags override config
- [ ] Out-of-the-box defaults: propose + human (safest)
- [ ] Non-TTY fallback: propose degrades to act (caller wants results)
- [ ] Multi-model pipeline:
  - Classification + command generation: merged into single call where feasible (Opus)
  - Explanation/surrounding output: Sonnet
  - Info requests: Opus
  - All model choices configurable and overridable via `--model` flag
- [ ] API access configurable: default to `claude` CLI (piggyback on existing auth), support direct Anthropic API mode
- [ ] Rust binary — own repo, own build/release pipeline
- [ ] Streaming output support for info responses
- [ ] Exit codes: passthrough from executed commands, distinct codes for tai-internal errors

## Assumptions (Examined)
| Assumption | Challenged? | Status |
|-----------|------------|--------|
| Users have claude CLI installed | Asked about API access — configurable, CLI is default | Validated |
| One-shot is the primary mode | Asked about use cases — confirmed "anything in one turn" | Validated |
| Environment detection is cheap enough to run every invocation | User said: deterministic codepaths, no caching needed, no Claude for detection | Validated |
| Opus is worth the cost/latency for command crafting | User explicitly chose Opus for commands and info | Validated |
| Propose is the right default | User confirmed propose + human | Validated |
| Non-TTY should fall back to act, not inform | User chose act — scripts calling tai want results | Validated |

## Constraints
- Must work on macOS and Linux (primary targets)
- Depends on either `claude` CLI or an ANTHROPIC_API_KEY for direct API mode
- Config format: YAML at `~/.config/tai.yaml`
- Rust toolchain required for building from source

## What "Done" Looks Like
- `tai how do I find large files in this repo` → prints a command, prompts Y/n/?, executes on confirmation
- `tai what's using port 3000` → detects this is an info request, answers directly
- `tai tarball this directory` → proposes a tar command, executes on Y
- `echo $(tai --inform compress all PNGs in this dir)` → outputs raw command for capture
- `tai --act --agent rename all .jpeg to .jpg recursively` → just does it, complex command is fine
- Config file customizes defaults; flags override per-invocation

## Open Questions
- Should tai support a `--dry-run` flag that shows what command it *would* execute without running it? (Overlaps with inform mode)
- Should the `?` explanation in propose mode use a separate Sonnet call, or should the explanation be pre-generated and cached from the initial Opus call?
- Distribution strategy: cargo install, GitHub releases with prebuilt binaries, or both?
- Should tai have a `--history` or `--last` feature to recall/re-run the last proposed command?

## Prior Art
- `claude-oneshot` (this repo) — simple `claude -p` wrapper, no env detection or classification
- `claude-yolo` (this repo) — bypasses Claude Code permissions, different use case
- Various "AI shell" tools exist (shell-gpt, aichat, etc.) — research phase will catalog these
