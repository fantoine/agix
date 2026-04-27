# Rich Plugin Installation — Design Spec

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** agix installs not only skill files but also all "annexe" elements a plugin/skill declares — MCP server entries, rules/instructions fragments, hooks, slash-commands, and more — across all supported CLI drivers, with full update/remove lifecycle.

**Architecture:** Declarative `[install.cli]` table in `Agentfile` per dependency; `CliDriver` trait extended with `install_files`/`uninstall_files`; tracked mutations stored in `Agentfile.lock`; marker-based injection in shared files; conflict detection via SHA fingerprinting.

**Tech Stack:** Rust, existing `CliDriver` trait, `serde_json`/`toml_edit` for structured patching, SHA-256 (already in tree via `sha2` or similar), `zip` crate already used.

---

## 1. Installation modes

agix supports two complementary modes, composable per CLI:

### 1a. Convention-based (default)

agix scans the downloaded package for known directory/file layouts and installs them automatically. No `Agentfile` required in the package.

| Convention dir | Installed to | Drivers |
|----------------|-------------|---------|
| `skills/` | driver skills dir | `claude`, `vibe`, `goose` |
| `commands/` | `~/.claude/commands/` | `claude` |
| `rules/` | rules file / dir | `claude`, `cursor`, `windsurf`, `cline`, `amp` |
| `agents/` | agents dir | `codex`, `opencode` |
| `prompts/` | `~/.vibe/prompts/` | `vibe` |
| `mcp/*.json` | MCP config | all drivers |
| `hooks/` | `~/.claude/hooks/` | `claude` |
| `CLAUDE.md` | marker inject → `~/.claude/CLAUDE.md` | `claude` |
| `AGENTS.md` | marker inject → project `AGENTS.md` | `codex`, `opencode`, `windsurf`, `goose` |
| `GEMINI.md` | marker inject → `~/.gemini/GEMINI.md` | `gemini` |

Convention scan runs when: (a) package has no `Agentfile`, OR (b) package has `Agentfile` with no `[install.<driver>]` for this driver.

### 1b. Declarative (Agentfile `[install.cli]`)

When `[install.<driver>]` is present, conventions still apply **unless overridden or disabled**. The table enriches or replaces convention entries.

```toml
[dependencies.my-skill]
source = "github:org/repo@main"

# Convention scan runs (skills/, commands/, rules/, mcp/ etc.)
# Nothing extra needed for basic installs.

[dependencies.my-skill.install.claude]
# Key present = override that convention's source path:
skills   = "my-skills/"          # scan my-skills/ instead of skills/
commands = "tools/commands/"     # scan tools/commands/ instead of commands/
rules    = ["rules/claude.md"]   # explicit file list instead of dir scan

# Non-conventional elements (always explicit):
mcp       = "config/mcp.json"
claude_md = "docs/fragment.md"
hooks     = ["hooks/pre.sh", "hooks/post.sh"]

# Disable all conventions for this driver, only install what's listed:
mode = "explicit"

# OR: keep conventions but suppress specific items:
exclude = ["skills/", "CLAUDE.md"]

[dependencies.my-skill.install.vibe]
skills = "my-skills/"            # override source dir
mcp    = "config/vibe-mcp.json"

[dependencies.my-skill.install.windsurf]
rules = ["rules/windsurf.md"]    # override → .windsurfrules
mcp   = "mcp/windsurf-mcp.json"
```

### 1c. Value forms for any key

| Form | Meaning |
|------|---------|
| `"dir/"` (trailing slash) | Scan entire directory |
| `"file.md"` | Single file |
| `["a.md", "b.md"]` | Explicit file list |
| `{ src = "path/", target_subdir = "sub/" }` | Override source dir + target subdirectory |

### 1d. Resolution rules

1. No `[install.<driver>]` → run convention scan only.
2. `[install.<driver>]` present, no `mode` key → convention scan + listed extras; listed keys override their convention counterpart.
3. `mode = "explicit"` → skip convention scan entirely; only install listed keys.
4. `exclude = ["skills/"]` → convention scan minus listed items; listed keys still add/override.

Absent `[install.cli]` table = conventions only (backward compat; packages without Agentfile work out of the box).

---

## 2. CliDriver trait extension

```rust
pub trait CliDriver {
    // existing
    fn install(&self, pkg: &Package, lock: &mut Lock) -> Result<()>;
    fn uninstall(&self, name: &str, lock: &mut Lock) -> Result<()>;

    // new
    fn install_files(&self, pkg: &Package, install: &InstallSpec, lock: &mut Lock) -> Result<()>;
    fn uninstall_files(&self, pkg_name: &str, lock: &mut Lock) -> Result<()>;
    fn check_files(&self, pkg_name: &str, lock: &Lock) -> Vec<FileIssue>;
}
```

`InstallSpec` = resolved install config for one driver, after merging conventions + Agentfile overrides:

```rust
pub struct InstallSpec {
    pub mode: InstallMode,        // Layered (default) | Explicit
    pub exclude: Vec<String>,     // convention items to suppress
    pub entries: Vec<InstallEntry>,
}

pub enum InstallMode { Layered, Explicit }

pub struct InstallEntry {
    pub kind: EntryKind,          // Skills | Commands | Rules | Mcp | Hooks | Fragment | ...
    pub src: PathBuf,             // path inside the package
    pub target_subdir: Option<String>, // override target subdirectory
}
```

`InstallSpec::resolve(conventions: &DriverConventions, agentfile_table: Option<&Table>) -> InstallSpec`
builds the merged spec: start from conventions, apply excludes, overlay explicit entries.

Default implementation: no-op (drivers that don't support file install yet compile fine).

---

## 3. File installation strategies

### 3a. Copy-to-dedicated-dir (simple case)
Target dir is owned by CLI and agix can write freely:

| CLI | Element | Target |
|-----|---------|--------|
| Claude Code | `commands/` | `~/.claude/commands/` |
| Claude Code | `rules/` | `~/.claude/rules/` |
| Claude Code | `hooks/` scripts | `~/.claude/hooks/` |
| Mistral Vibe | `skills/` | `~/.vibe/skills/` |

Strategy: copy file, record `{path, sha}` in lock under `installs.<pkg>.<driver>.files[]`.  
Update: re-copy, update sha.  
Remove: delete file, remove lock entry.

### 3b. Marker injection in markdown files
Target: `~/.claude/CLAUDE.md`, `~/.gemini/GEMINI.md`, `AGENTS.md`, `.windsurfrules`, `.vibe/prompts/*.md`, etc.

Injection pattern:
```markdown
<!-- agix:my-skill:start -->
## My Skill Instructions
...content from fragment...
<!-- agix:my-skill:end -->
```

- Install: append block at end of file (create file if absent).
- Update: replace content between markers; update sha in lock.
- Remove: delete lines from start-marker to end-marker inclusive.
- Conflict: sha in lock ≠ sha of current content between markers → `doctor` warning, `--force` to overwrite.

### 3c. Structured patching in JSON/TOML
Target: `~/.claude/settings.json` (mcpServers, hooks, allowedTools), `~/.cursor/mcp.json`, `~/.vibe/config.toml`, `~/.goose/config.yaml`, etc.

Key convention: top-level key named `__agix_<pkg>__` (or namespaced within `mcpServers`).

JSON example:
```json
{
  "mcpServers": {
    "__agix_my-skill__": {
      "command": "npx",
      "args": ["-y", "@org/my-mcp-server"]
    }
  }
}
```

TOML example (Vibe):
```toml
[[mcp_servers]]
# agix:my-skill
name = "my-skill"
transport = "stdio"
command = "npx"
args = ["-y", "@org/my-mcp-server"]
```

TOML arrays use a comment marker `# agix:<pkg>` on the line preceding the `[[section]]` entry.

- Install: insert key/entry; record `{file, key_path, sha}` in lock.
- Update: replace value at key_path; update sha.
- Remove: delete key/entry; remove lock entry.
- Conflict: sha stale → warning.

---

## 4. Lock file extension

```toml
[installs.my-skill.claude.files]
# copy-to-dir entries
"~/.claude/commands/my-command.md" = { kind = "copy", sha = "abc123" }
"~/.claude/rules/my-rules.md"      = { kind = "copy", sha = "def456" }

# marker injection
"~/.claude/CLAUDE.md" = { kind = "marker", sha = "ghi789" }

# structured patch
"~/.claude/settings.json" = { kind = "json_key", key = "mcpServers.__agix_my-skill__", sha = "jkl012" }

[installs.my-skill.windsurf.files]
".windsurfrules" = { kind = "marker", sha = "mno345" }
```

SHA = SHA-256 of the managed content only (not full file), hex-encoded.

---

## 5. Supported CLIs — driver map

| CLI | Driver ID | Config dir | Supported strategies |
|-----|-----------|-----------|----------------------|
| Claude Code | `claude` | `~/.claude/` | copy-dir, marker (CLAUDE.md), json_key (settings.json) |
| Codex | `codex` | `~/.codex/` | marker (AGENTS.md) |
| Cursor | `cursor` | `~/.cursor/` | marker (.cursorrules), json_key (mcp.json) |
| Windsurf | `windsurf` | `~/.codeium/windsurf/` | marker (.windsurfrules, AGENTS.md), json_key (MCP settings) |
| Gemini CLI | `gemini` | `~/.gemini/` | marker (GEMINI.md), json_key (settings.json) |
| Mistral Vibe | `vibe` | `~/.vibe/` | toml_entry (config.toml mcp_servers), copy-dir (skills/) |
| Goose | `goose` | `~/.goose/` | json_key (config MCP), marker (AGENTS.md) |
| Cline | `cline` | VS Code settings | marker (.clinerules), json_key (settings.json) |
| GitHub Copilot | `copilot` | `.github/` | marker (copilot-instructions.md), json_key (mcp.json) |
| OpenCode | `opencode` | `~/.opencode/` | marker (AGENTS.md), json_key (config.json) |
| Amp | `amp` | `~/.amp/` | marker (.amp/rules.md), json_key (settings.json) |

---

## 6. Command changes

### `agix install` / `agix add`
After resolving + downloading package:
1. Detect which CLIs are active on machine (via existing `detect_local_config`).
2. For each active CLI: read `[install.<driver_id>]` from package; call `driver.install_files()`.
3. Write mutations to lock.

### `agix remove <pkg>`
1. For each CLI in lock `installs.<pkg>`: call `driver.uninstall_files()`.
2. Remove `installs.<pkg>` from lock.

### `agix update <pkg>`
1. Download new version.
2. For each CLI: diff managed content; call `driver.install_files()` (re-inject between markers or replace key).

### `agix doctor`
Extended checks per installed package:
- File exists at recorded path?
- SHA still matches managed content? (conflict detection)
- CLI config still parseable?

### `agix install --cli <driver>`
Install file elements for a specific CLI only (e.g. after adding a new CLI to the machine).

---

## 7. Error handling

| Scenario | Behavior |
|----------|----------|
| Target file not writable | Error with path, suggest `sudo` or manual |
| Marker missing on update/remove | Warning: "managed section not found, may have been manually removed" — skip, clean lock |
| SHA mismatch (conflict) | Warning in `doctor`, `--force` flag to overwrite |
| JSON/TOML parse error | Error with file path + line hint |
| CLI not detected | Skip silently; `--cli <driver>` forces install anyway |
| Duplicate key on install | Idempotent: update in place |

---

## 8. Testing

- Unit: `FileInstaller` for each strategy (copy, marker, json_key, toml_entry) — inject, update, remove, conflict detect.
- Integration per driver: temp HOME, install package with `[install.claude]`, assert file content + lock entries; then remove, assert cleanup.
- Roundtrip: install → manual edit → `doctor` reports conflict → `--force` reinstall → clean state.
- Conflict: inject, corrupt managed section, assert SHA mismatch detected.

---

## 9. Out of scope (this iteration)

- GUI / interactive conflict resolution (text diff shown, user picks)
- Automatic merge of conflicting markdown sections
- Windows path support (tracked separately)
- Non-file driver actions (e.g. calling a CLI's plugin API directly)
