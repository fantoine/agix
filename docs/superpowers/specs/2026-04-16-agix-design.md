# Agix — Design Spec

**Date**: 2026-04-16
**Status**: Approved

## Overview

Agix is a Rust CLI tool that acts as a universal package manager for AI CLI tools (Claude Code, Codex, Gemini, Opencode, etc.). It manages the installation, update, and removal of skills, agents, plugins, and other extensions across multiple AI CLIs, using a familiar package manager workflow (Agentfile, lock file, standard commands).

## Scope — v1

Two CLI drivers in v1: **Claude Code** and **Codex**. The architecture anticipates additional CLIs via the `CliDriver` trait without requiring a rewrite.

## Architecture

### Project Structure

```
agix/
├── src/
│   ├── main.rs
│   ├── commands/
│   ├── core/
│   │   ├── resolver.rs
│   │   ├── installer.rs
│   │   └── lock.rs
│   ├── sources/
│   │   ├── github.rs
│   │   ├── git.rs
│   │   └── local.rs
│   ├── drivers/
│   │   ├── mod.rs
│   │   ├── claude_code.rs
│   │   └── codex.rs
│   └── manifest/
│       └── agentfile.rs
```

### Installation Flow

```
Agentfile → Resolver → Sources (fetch) → package Agentfile (optional, hooks) → Driver(s) → Installer → Lock file
```

### CliDriver Trait

```rust
trait CliDriver {
    fn name(&self) -> &str;
    fn detect(&self) -> bool;
    fn install_dir(&self, scope: Scope) -> PathBuf;
    fn install(&self, pkg: &Package) -> Result<Vec<InstalledFile>>;
    fn uninstall(&self, files: &[InstalledFile]) -> Result<()>;
    fn install_from_marketplace(&self, source: &str) -> Result<Vec<InstalledFile>>;
}
```

## Agentfile Format

The `Agentfile` (TOML) is a unified format serving two roles:

- **Project manifest**: declares which packages to install
- **Package manifest**: declares package metadata and its own dependencies

### Project Agentfile

```toml
[agix]
cli = ["claude-code", "codex"]

[dependencies]
rtk = { source = "github:org/rtk", exclude = ["codex"] }

[claude-code.dependencies]
superpowers = { source = "github:claude-plugins-official/superpowers", version = "^2.0" }
rtk-marketplace = { source = "claude:marketplace@org/rtk" }

[codex.dependencies]
some-plugin = { source = "git:https://git.example.com/org/plugin.git", version = "main" }
local-tool = { source = "local:../my-tool" }
```

### Package Agentfile

```toml
[agix]
name = "superpowers"
version = "2.1.0"
description = "Supercharge your Claude Code workflow"
cli = ["claude-code"]

[dependencies]
shared-dep = { source = "github:org/dep" }

[claude-code.dependencies]
claude-only-dep = { source = "github:org/other" }
```

### Dependency Fields

| Field | Description |
|---|---|
| `source` | Package source (see Sources) |
| `version` | Version constraint (optional) |
| `exclude` | List of CLIs to exclude from installation |

## Agentfile.lock

The lock file combines version pinning and installation state into a single source of truth.

```toml
[[package]]
name = "superpowers"
source = "github:claude-plugins-official/superpowers"
sha = "a3f8c12d..."
cli = ["claude-code"]
scope = "global"

[[package.files]]
dest = "~/.claude/skills/brainstorming"

[[package.files]]
dest = "~/.claude/skills/writing-plans"

[[package]]
name = "local-tool"
source = "local:../my-tool"
content_hash = "b7e2a91f..."
cli = ["codex"]
scope = "local"

[[package.files]]
dest = "./.agix/codex/local-tool"
```

**Fields**:
- `sha`: resolved git commit SHA (git/github sources)
- `content_hash`: blake3 hash of content (local sources)
- `scope`: `global` or `local`
- `files`: list of installed file destinations, used for clean uninstall/update

## Sources

### `github:org/repo`

Resolves via GitHub API. Downloads a zip archive of the resolved ref. No local git dependency required.

### `git:https://...`

Arbitrary git repository. Resolves branches, tags, and commit hashes. Lock stores the resolved commit SHA.

### `local:./path`

Local filesystem path. No version concept. Lock stores a blake3 content hash to detect changes on `agix update`.

### `claude:marketplace@org/plugin`

Delegated entirely to the Claude Code driver, which handles marketplace-native installation. Each CLI driver may define its own marketplace source scheme.

### Version Resolution

```
version = "^2.0"    → lists repo tags, applies SemVer resolution, picks latest match
version = "main"    → resolves HEAD SHA of branch main
version = "abc1234" → used directly as commit SHA
(absent)            → HEAD of default branch
```

Version conflicts (two packages requiring incompatible versions of the same dependency) result in a hard error with a clear message. No silent resolution.

## CLI Drivers

### Claude Code

**Detection**: presence of `~/.claude/` or `claude` binary.

**Default installation conventions** (when package has no Agentfile):

```
skills/       → ~/.claude/skills/
agents/       → ~/.claude/agents/
hooks/        → ~/.claude/hooks/
mcp-servers/  → ~/.claude/mcp/
*.md          → ~/.claude/
```

**Marketplace**: delegates to Claude Code's native marketplace mechanism.

### Codex

**Detection**: presence of `codex` binary or `~/.codex/`.

**Installation**: Agix manages a dedicated directory `~/.codex/agix/<package-name>/` and configures Codex to reference installed files via whatever mechanism Codex exposes (config file, instruction includes, etc.). The exact binding mechanism is determined by Codex's actual configuration surface.

**No native marketplace**: Agix acts as Codex's plugin manager directly.

### Package-level Hooks (via Agentfile)

When a package includes an `Agentfile` with hooks, the driver executes them at the appropriate lifecycle points:

```toml
[hooks]
post-install = "scripts/setup.sh"
pre-uninstall = "scripts/cleanup.sh"
```

## Scope: Global vs Local

| | Global | Local |
|---|---|---|
| Agentfile location | `~/.agix/Agentfile` | `./Agentfile` |
| Lock file location | `~/.agix/Agentfile.lock` | `./Agentfile.lock` |
| Init command | `agix init --global` | `agix init` |
| Default for `agix add` | no | yes |

The two scopes are independent: `agix install` installs from the local Agentfile, `agix install --global` from the global one. If a package name exists in both scopes, the local installation takes precedence at runtime (the CLI loads it last). CLIs must be declared in `[agix] cli = [...]` and be detected on the machine to receive installations. A warning is displayed for declared-but-undetected CLIs.

## Commands

### Core

```
agix install                  # install all dependencies from Agentfile
agix install --global         # install from global Agentfile

agix add <source>             # add dependency and install it
agix add <source> --cli claude-code
agix add <source> --global
agix add <source> --version "^2.0"

agix remove <name>            # uninstall and remove from Agentfile
agix remove <name> --global

agix update                   # update all local packages (respects version constraints)
agix update --global          # update all global packages
agix update <name>            # update a specific local package
agix update <name> --global

agix outdated                 # list local packages with newer versions available
agix outdated --global        # list global packages with newer versions available

agix list                     # list installed packages (local + global, scope indicated)
agix list --global            # list global packages only
```

### Utility

```
agix init                     # create empty Agentfile in current directory
agix init --global            # initialize ~/.agix/Agentfile

agix check                    # validate current package's Agentfile (no upload)

agix doctor                   # check installation health (detected CLIs, broken packages)

agix export                   # zip: Agentfile + Agentfile.lock + local sources
agix export --global          # zip: global Agentfile + global local sources
agix export --all             # both combined
agix export --output <file>
```

### Lock Behavior

| Command | Lock behavior |
|---|---|
| `agix install` | Respects lock if present (exact SHAs), otherwise resolves and generates lock |
| `agix update` | Ignores lock, re-resolves, rewrites lock |
| `agix add` / `agix remove` | Modifies Agentfile then updates lock |

### Output

Human-readable by default:

```
agix install
  ✓ superpowers 2.1.0  →  claude-code  (github:claude-plugins-official/superpowers@a3f8c12)
  ✓ rtk 1.4.0          →  claude-code  (claude:marketplace@org/rtk)
  ⚠ codex not detected  —  2 packages skipped
```

Machine-readable via `--json` flag on all commands.

## Export Format

```
agix-export/
├── Agentfile
├── Agentfile.lock
└── local-sources/
    └── my-tool/
```

Remote sources (`github:`, `git:`) are not included — they are re-fetchable. Only `local:` sources are bundled as they may not be versioned elsewhere.

## Technical Stack

```toml
clap       = "4"      # CLI argument parsing
tokio      = "1"      # async runtime (parallel downloads)
reqwest    = "0.12"   # HTTP (GitHub API, downloads)
git2       = "0.19"   # git operations
toml       = "0.8"    # Agentfile parse/serialize
serde      = "1"      # serialization
blake3     = "1"      # content hashing (local sources)
indicatif  = "0.17"   # progress bars
thiserror  = "1"      # typed internal errors
anyhow     = "1"      # CLI surface error propagation
```

Parallel downloads via `tokio`. Sequential installation per CLI to avoid write conflicts on CLI config files.

## Distribution

```
cargo install agix              # via crates.io
brew install agix               # via Homebrew tap (homebrew-agix)
curl -fsSL https://agix.sh/install.sh | sh
```

Homebrew tap maintained as a separate `homebrew-agix` repository with a formula pointing to GitHub releases. Built and released via `cargo-dist` targeting `x86_64` and `aarch64` on macOS, Linux, and Windows.
