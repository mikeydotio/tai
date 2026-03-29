# Domain Research: Terminal Environment Detection for tai

Research conducted 2026-03-26 for the `tai` (Terminal AI) project.

---

## 1. Host OS Detection

### Recommended Approach

**Linux:** Parse `/etc/os-release` (Confidence: **HIGH**)

Key fields: `ID` (distro), `ID_LIKE` (family), `VERSION_ID`, `PRETTY_NAME`. Simple `KEY=VALUE` format, trivial to parse without a crate.

**macOS:** Run `sw_vers` (Confidence: **HIGH**)

`sw_vers -productName` and `sw_vers -productVersion`. Always present on macOS.

**Kernel-level fallback:** `std::env::consts::OS` gives `"linux"` or `"macos"` at compile time. For WSL detection, check if `/proc/version` contains `"microsoft"`.

### Recommendation: Roll your own (~20 lines of std)

Crates like `os_info` and `os-release` exist but are unnecessary for tai's needs.

### Edge Cases
- **NixOS:** Has `/etc/os-release` with `ID=nixos` but package management is fundamentally different
- **Alpine/musl:** `ID=alpine`; important because many CLI tools behave differently under musl
- **WSL:** Looks like Ubuntu/Debian in `/etc/os-release`. Check `/proc/version` for `microsoft` or `$WSL_DISTRO_NAME`
- **Containers:** `/etc/os-release` reflects container image OS, not host — correct behavior for tai

---

## 2. Shell Detection

### Recommended Approach: Walk the Parent Process Tree (Confidence: **HIGH**)

`$SHELL` is the login shell, not the current shell. **Do not use it.**

**Correct method:** Get PPID, read parent executable name.

- **Linux:** `std::os::unix::process::parent_id()` + read `/proc/{ppid}/exe` via `std::fs::read_link`
- **macOS:** Same PPID, but use `ps -p {ppid} -o comm=` (no `/proc`)

**Fallback cascade:**
1. `/proc/{ppid}/exe` (Linux)
2. `ps -p {ppid} -o comm=` (cross-platform)
3. Shell-specific env vars: `$BASH_VERSION`, `$ZSH_VERSION`, `$FISH_VERSION`
4. `$SHELL` (last resort)

### Edge Cases
- **Nested shells:** PPID gives immediate parent — correct for command generation
- **Non-shell parents:** If invoked from `python`/`node`, walk up further or report `unknown`
- **macOS:** Must use `ps` or `sysinfo` crate (no `/proc`)

---

## 3. Interactive vs Script Detection

### Recommended Approach: `std::io::IsTerminal` (Confidence: **HIGH**)

```rust
use std::io::IsTerminal;
let stdin_tty = std::io::stdin().is_terminal();
let stdout_tty = std::io::stdout().is_terminal();
```

`atty` crate is **unmaintained** — use stdlib.

### TTY Matrix

| stdin | stdout | Interpretation |
|-------|--------|----------------|
| TTY | TTY | Interactive; full propose/prompt UI |
| TTY | pipe | Output captured; suppress prompts |
| pipe | TTY | Input piped in; read stdin as query |
| pipe | pipe | Fully scripted; act mode, raw output |

---

## 4. Terminal Multiplexer Detection

### Recommended Approach: Check Environment Variables (Confidence: **HIGH**)

| Multiplexer | Env Var | Value When Set |
|-------------|---------|----------------|
| tmux | `$TMUX` | Socket path and window/pane info |
| screen | `$STY` | Session name |
| zellij | `$ZELLIJ` | `0` when inside a session |

### Edge Cases
- **Nested multiplexers:** Both `$TMUX` and `$STY` may be set — report both
- **Detached/reattached:** Env vars persist correctly
- **tmux env var staleness:** Variables can be stale from old SSH sessions — document limitation

---

## 5. SSH / Mosh Detection

### SSH: Check `$SSH_CONNECTION` (Confidence: **HIGH**)

Contains `client_ip client_port server_ip server_port`. Most reliable single indicator.

### Mosh: Walk Process Tree (Confidence: **MEDIUM**)

Mosh deliberately does NOT set an environment variable (project rejected this in issue #738). Must walk process tree looking for `mosh-server`.

### Detection Priority
```
1. Walk process tree for mosh-server -> "mosh"
2. $SSH_CONNECTION is set -> "ssh"
3. Neither -> "local"
```

### Edge Cases
- **VS Code Remote SSH:** Sets `$SSH_CONNECTION` + `$VSCODE_*` variables
- **Nested SSH:** `$SSH_CONNECTION` reflects outermost session

---

## 6. Container Runtime Detection

### Recommended Approach: Layered Detection (Confidence: **HIGH**)

**Critical finding:** On cgroups v2, `/proc/self/cgroup` contains only `0::/` — useless for container detection. Use `/proc/self/mountinfo` instead.

**Also:** `/.dockerenv` does NOT exist in containerd-managed containers.

**Detection cascade:**

| Check | Detects |
|-------|---------|
| `/.dockerenv` exists | Docker |
| `/run/.containerenv` exists | Podman/OCI |
| `$container` env var | Podman, systemd-nspawn, LXC |
| `$KUBERNETES_SERVICE_HOST` env var | Kubernetes pod |
| `/proc/1/sched` first line not init/systemd | Generic container |
| `/proc/self/mountinfo` contains docker/containerd/kubepods/lxc | Various runtimes |

### Edge Cases
- **WSL2:** Not a container — check `/proc/version` for `microsoft` first
- **Docker-in-Docker:** `/.dockerenv` exists in inner container — correct
- **Rootless Podman:** `/run/.containerenv` still created

---

## 7. Package Manager Detection

### Recommended Approach: Scan PATH Directories (Confidence: **HIGH**)

**Do not fork a process per binary.** Read `$PATH`, split on `:`, check file existence via `std::fs::metadata`.

**Managers to detect:** apt, dnf, yum, pacman, zypper, apk, emerge, brew, port, nix, snap, flatpak

```rust
fn available_package_managers() -> Vec<String> {
    let managers = ["apt", "dnf", "yum", "pacman", "zypper", "apk",
                    "emerge", "brew", "port", "nix", "snap", "flatpak"];
    let path = std::env::var("PATH").unwrap_or_default();
    let dirs: Vec<&str> = path.split(':').collect();
    managers.iter().filter(|mgr| {
        dirs.iter().any(|dir| {
            std::fs::metadata(format!("{}/{}", dir, mgr))
                .map(|m| m.is_file()).unwrap_or(false)
        })
    }).map(|s| s.to_string()).collect()
}
```

Or use the `which` crate for cleaner cross-platform PATH scanning.

---

## 8. CWD + Git Info

### Recommended Approach: Shell Out to `git` (Confidence: **HIGH**)

Avoid `git2` (adds 2-4MB, significant compile time) and `gix` (overkill for status checks).

**Commands:**
```
git rev-parse --is-inside-work-tree    # "true" if in a repo
git rev-parse --show-toplevel          # repo root
git rev-parse --abbrev-ref HEAD        # branch name
git status --porcelain                 # empty = clean
```

All complete in <10ms on typical repos. Set a 200ms timeout for `git status` on monorepos.

**Fallback if no git:** Walk up directories looking for `.git`, read `.git/HEAD` for branch name.

### Edge Cases
- **Detached HEAD:** Returns literal `HEAD` — fall back to `git rev-parse --short HEAD`
- **Large repos:** `git status --porcelain` can be slow — use `--untracked-files=no` or timeout
- **Deleted CWD:** `std::env::current_dir()` returns `Err` — handle gracefully

---

## Recommended JSON Output Structure

```json
{
  "os": "ubuntu",
  "os_version": "24.04",
  "os_family": "debian",
  "kernel": "6.18.7",
  "shell": "zsh",
  "interactive": true,
  "multiplexer": "tmux",
  "remote": "ssh",
  "container": "containerd",
  "package_managers": ["apt", "snap"],
  "cwd": "/home/mikey/cli-helpers",
  "git_repo": true,
  "git_branch": "main",
  "git_dirty": false
}
```

---

## Key Recommendations

1. **Minimize dependencies.** Most detection uses std only. Pull in `which` for PATH scanning and `sysinfo` for macOS process tree.
2. **Structure as independent `detect_*()` functions.** Each returns a struct/enum, can be run in parallel.
3. **Make detection infallible.** Return `Unknown`/`None`/empty vec — never propagate errors. LLM works with partial context.
4. **Total detection budget: under 50ms.** File reads and env checks are sub-millisecond. Only `git status` might be slow.
5. **Gate `/proc`-based detection behind `cfg!(target_os = "linux")`** with macOS fallbacks.
