# Research Summary: tai (Terminal AI)

Synthesized from three domain research documents, 2026-03-26.

---

## Market Position

tai occupies a genuinely underserved niche. 13 tools were surveyed across two tiers (command-focused assistants and full agentic CLIs). tai's three core differentiators are validated as market gaps:

1. **Automatic intent classification** — No existing tool auto-classifies info vs action requests. All require explicit mode flags. This is tai's single biggest differentiator.
2. **Command complexity control** (`human` vs `agent`) — Entirely novel. No competitor offers this. Addresses a real pain point: LLMs generate complex one-liners that are hard to verify.
3. **Deep environment detection** — Most tools detect only OS + shell. tai's planned detection (SSH, tmux, containers, package managers, git state) would be best-in-class.

The closest competitor is **aichat** (Rust, 9.5k stars, 20+ model providers) but it lacks all three differentiators.

---

## Architecture Decisions (Research-Informed)

### Language & Runtime
**Rust** — confirmed as the right choice. Aichat, Codex CLI, and Amazon Q CLI are all Rust. Startup is ~50ms vs 200-500ms for Python tools.

### Dependency Strategy
Minimal, sync-only stack. No tokio runtime needed.

| Dependency | Purpose |
|-----------|---------|
| clap 4 (derive) | CLI arg parsing |
| serde + serde_json | Serialization |
| serde_yaml_ng | Config file (serde_yaml is archived) |
| ureq 3 | HTTP client (sync, no tokio) |
| owo-colors + supports-color | Terminal colors |
| anyhow + thiserror | Error handling |
| which | PATH scanning for package managers |

**Notable omissions:** No tokio, no dialoguer, no crossterm, no atty, no SSE library, no git2/gix. Estimated binary: 3-5MB stripped.

### API Architecture
Single merged LLM call for classification + command generation using structured JSON output:
```json
{
  "type": "action",
  "confidence": 0.95,
  "command": "lsof -i :3000",
  "explanation": "Lists all processes using port 3000"
}
```
This avoids a separate classification round-trip. When confidence is low, default to the safer mode.

Model routing:
- **Opus:** Classification + command crafting, info fulfillment
- **Sonnet:** Explanations (on `?` in propose mode), surrounding output
- All configurable via `--model` flag and config file

API backend: Default to `claude` CLI (piggybacks on existing auth). Support direct Anthropic API for power users/CI.

### Config Resolution Pipeline
```
hardcoded defaults → config file (~/.config/tai.yaml) → TTY-based overrides → CLI flags
```
Manual merge with Option fields. ~6 config fields — framework is overkill.

### Environment Detection
Deterministic Rust codepaths, no LLM calls. JSON structure injected into prompt template.

| Signal | Method | Platform |
|--------|--------|----------|
| OS/distro | `/etc/os-release` (Linux), `sw_vers` (macOS) | Both |
| Shell | Walk parent process tree via PPID, NOT `$SHELL` | Both |
| Interactive | `std::io::IsTerminal` on stdin/stdout | Both |
| tmux/screen/zellij | `$TMUX`, `$STY`, `$ZELLIJ` env vars | Both |
| SSH | `$SSH_CONNECTION` env var | Both |
| Mosh | Walk process tree for `mosh-server` | Both |
| Container | Layered: `/.dockerenv`, `/run/.containerenv`, `$container`, `/proc/self/mountinfo` | Linux |
| Package managers | Scan `$PATH` dirs for known binaries (no process spawning) | Both |
| Git | Shell out to `git` CLI (branch, dirty) | Both |

**Key findings:**
- `$SHELL` is the login shell, not the running shell — must walk process tree
- cgroups v2 broke `/proc/self/cgroup` container detection — use `/proc/self/mountinfo`
- `/.dockerenv` misses containerd containers — need layered detection
- Mosh has no env var (project rejected it) — must check process tree
- Gate `/proc`-based detection behind `cfg!(target_os = "linux")`
- Total detection budget: <50ms

### SSE Streaming
Hand-rolled with `BufRead::lines()`. The Anthropic SSE format is simple `event:`/`data:` lines. ~30 lines of code. No library needed.

---

## Risk Register

| Risk | Severity | Mitigation |
|------|----------|------------|
| Misclassification (info↔action) | High | Confidence scores, default to safer mode, structured JSON output forces explicit classification |
| Hallucinated commands | High | Deep env detection reduces wrong-OS errors, propose mode catches before execution |
| serde_yaml ecosystem instability | Low | serde_yaml_ng is actively maintained; could pivot to TOML if needed |
| Scope creep toward agentic territory | Medium | Hard constraint: single-turn only, no file editing, no multi-step workflows |
| Anthropic-only limits adoption | Medium | claude CLI piggybacking for zero-friction setup; --model flag escape hatch |

---

## Open Questions Resolved by Research

1. **Should `?` explanation be pre-generated or on-demand?** → On-demand (separate Sonnet call). Pre-generating wastes tokens on explanations rarely requested.
2. **How to handle classification + generation latency?** → Merge into single structured-output call. One round-trip.
3. **Which YAML crate?** → serde_yaml_ng (drop-in replacement for archived serde_yaml).
4. **Async or sync?** → Sync (ureq). 1-3 calls per invocation doesn't justify tokio.
5. **Hand-roll or library for Y/n/? prompt?** → Hand-roll (~10 lines). Not worth a dependency.
6. **git2 or shell out?** → Shell out. Saves 2-4MB binary size and significant compile time.

---

## Sources

See individual research documents:
- `existing-tools.md` — 13 tools surveyed, market gaps identified
- `rust-patterns.md` — Crate recommendations, API patterns, pitfalls
- `env-detection.md` — Detection methods for 8 environment categories
