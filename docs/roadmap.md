# agix Roadmap

This document tracks the long-term direction for **agix** based on a competitive
benchmark against AGPM (ex-CCPM), MCS (Managed Claude Stack), `skills`
(vercel-labs), AGR and a handful of adjacent tools (CraftDesk, mcpm.sh,
Install-MCP, ccpi / tonsofskills). The benchmark itself lives in
[`docs/benchmark/competitors.md`](./benchmark/competitors.md) and the per-CLI
detail in [`docs/benchmark/cli-drivers.md`](./benchmark/cli-drivers.md).

The roadmap is split into three buckets:

1. **Must-have** — features that competitors already ship; agix needs them to
   feel complete to a developer evaluating it side-by-side with AGPM, MCS or
   `skills`.
2. **Key differentiators** — features that no competitor ships (or that agix
   does meaningfully better). These are the marketing story and the reason a
   user would pick agix over AGPM/MCS/skills.
3. **Nice-to-have** — quality-of-life features that improve adoption and
   confidence but aren't blocking.

Status legend: ✅ shipped · 🚧 in progress · 🗺 planned · 💭 considered

---

## 1. Must-have — competitive parity

These items address gaps where competitors already have a credible answer.
Without them, agix risks being dismissed as a less-mature alternative.

### 1.1 MCP injection across drivers — 🗺

> Today, MCP server registration only works for Claude Code (`~/.claude/settings.json`).
> AGPM does this for Claude as well; MCS does it across multiple targets.

**Goal:** When a package declares an MCP server, agix injects the entry in the
correct configuration file for *every* CLI driver active on the machine, with
the same lifecycle guarantees as for skills (install / update / remove /
conflict detection).

**Targets:** `~/.cursor/mcp.json`, `~/.gemini/settings.json`,
`~/.vibe/config.toml` (`[[mcp_servers]]` array), `~/.codeium/windsurf/`
(Cascade settings), `~/.opencode/config.json`, `~/.amp/settings.json`,
VS Code MCP file for Cline / Copilot, Goose extension config.

**Why must-have:** MCP is the unifying glue across AI agent tooling. Not
supporting it everywhere makes agix look Claude-only at first glance.

### 1.2 `agix find` — discovery — 🗺

> `skills find` lets the user search the public registry from the CLI;
> agix has no discovery mechanism today, so a user has to know the package
> URL ahead of time.

**Goal:** A first-class discovery command that searches across the sources
agix already supports — GitHub topics, the Claude Code marketplace, the
agentskills.io spec listing, and any future agix registry.

**MVP:** `agix find <query>` searches GitHub for repositories matching
agentskills.io conventions and prints `name • source • description` rows.

**Stretch:** `agix find --cli windsurf rules` searches by element type.

### 1.3 Drivers beyond Claude / Codex — 🗺

> Today only `claude` and `codex` drivers are implemented. AGPM is
> Claude-only. `skills` claims 45+ targets but with shallow per-CLI
> integration (it copies SKILL.md folders, no MCP, no rules).

**Goal:** Bring the next wave of drivers to a usable state — at minimum
covering the elements people actually want to install through agix
(skills/agents, rules/instructions, MCP, and where applicable hooks).

**Priority order (proposed):**
1. Cursor — large user base, well-documented `.cursorrules` + `~/.cursor/mcp.json`
2. Windsurf — richest extension surface (Hooks, Workflows, Skills, Memories)
3. Gemini CLI — `GEMINI.md` + MCP, Google adoption growing
4. Mistral Vibe — clean TOML model, similar shape to agix's manifest
5. Cline / Copilot / OpenCode / Amp / Goose — second wave once the foundation
   is stable

Per-driver detail and target paths are tracked in
[`docs/benchmark/cli-drivers.md`](./benchmark/cli-drivers.md).

### 1.4 Marketplace orchestration beyond Claude — 💭

> agix already calls `claude` to install marketplace plugins; the same
> pattern can apply to other CLIs that ship their own marketplace.

**Candidates:**
- `gh skill install <handle>` for GitHub Copilot skills
- Cursor extensions / MCP marketplace once the API stabilises
- Windsurf plugin store

**Why it matters:** Some skills are easier to install through the host CLI's
own command than by reinventing the file layout. agix should treat those
calls as a first-class source type, the same way it does for the Claude
marketplace today.

### 1.5 `snippets` resource type — 💭

> AGPM exposes `snippets` as a separate resource — small reusable text
> fragments (prompts, instructions, boilerplate) that aren't full skills.

**Open question:** does agix need a dedicated type, or is "a fragment that
gets injected via the rich-file-lifecycle mechanism" enough? Probably enough
in practice, but worth confirming before we commit one way or the other.

---

## 2. Key differentiators

These are the features that no competitor ships today (or where agix does
the job in a more complete way). They define the agix value proposition.

### 2.1 Rich file lifecycle — 🗺 (designed)

> No competitor manages the *full* lifecycle of fragments injected into
> shared configuration files. AGPM and MCS write to settings files but do
> not track ownership, detect drift, or clean up on remove.

**What it covers:**
- **Marker injection** — HTML comment markers in markdown
  (`<!-- agix:pkg:start -->`), prefixed JSON keys (`__agix_pkg__`),
  comment-marked TOML array entries, so each plugin owns a clearly-bounded
  region of every shared file.
- **SHA tracking in `Agentfile.lock`** — agix records the SHA of every
  managed fragment when it's written. On `update` or `remove` it can verify
  that the user hasn't manually edited the managed region.
- **Conflict detection** — if the SHA on disk no longer matches the SHA in
  the lock file, `agix doctor` warns and `agix install --force` is required
  to overwrite.
- **Targeted update / remove** — replace only the bytes between the markers
  (or the value at a key path), never the whole file.

**Spec:** [`docs/superpowers/specs/2026-04-27-rich-plugin-installation-design.md`](./superpowers/specs/2026-04-27-rich-plugin-installation-design.md)

### 2.2 Declarative `[install.cli]` overrides on top of conventions — 🗺 (designed)

> agix already installs by convention (it scans `skills/`, `commands/`,
> `rules/` etc.). The differentiator is layering a declarative escape hatch
> on top, the same way `Cargo.toml` overrides `cargo`'s defaults.

**What it covers:**
- Conventions run by default — packages without an `Agentfile` work out of
  the box, like `skills` does today.
- An `[install.<driver>]` table in the package's `Agentfile` can override
  source paths (`skills = "my-skills/"`), provide explicit file lists, or
  inject non-conventional elements (MCP server file, fragment for
  `CLAUDE.md`, etc.).
- `mode = "explicit"` disables the convention scan for a driver.
- `exclude = ["skills/"]` keeps conventions but suppresses specific items.

**Why no one else has it:** `skills` is conventions-only (no escape hatch);
AGPM is declarative-only (every file must be listed). agix combines both.

### 2.3 Multi-CLI, single manifest — ✅ (framework) / 🗺 (full driver coverage)

> The `CliDriver` trait, scope walk-up, and `[<cli>.dependencies]` per-driver
> filtering are already wired in. The remaining work is implementing the
> drivers themselves (see 1.3).

**Why it's the headline feature:** AGPM is Claude-only by design. `skills`
has 45+ targets but no manifest — every install is imperative. agix is the
only tool that combines reproducibility (`Agentfile` + `Agentfile.lock`)
with multi-CLI install in one fileset.

### 2.4 Doctor with managed-mutation health checks — 🗺

> Today `agix doctor` validates the `Agentfile` and reports git/libgit2
> status. The roadmap extends it to verify *every* fragment agix has injected
> still matches the SHA recorded in the lock file.

**Checks:**
- File still exists at the recorded path?
- SHA still matches the managed region (markers / JSON key / TOML entry)?
- Target CLI config still parses cleanly?
- Marker pair still present and balanced?

**Output:** structured report grouped by driver and by package, with
`agix doctor --json` for CI integration.

### 2.5 Export `--all` — ✅

Snapshots both local and global scopes into a single zip with `local/` and
`global/` prefixes, vendoring local file sources under `local/local-sources/`
and rewriting their paths so the archive can be unpacked and `agix install`-d
elsewhere. No competitor ships an equivalent.

### 2.6 Walk-up scope resolution — ✅

`agix` walks up from the current directory looking for an `Agentfile`,
falling back to `~/.agix/Agentfile` if none is found, the same way `git`,
`cargo` and `npm` resolve their manifests. AGPM and `skills` both require
the user to be in the project root (or to pass `-g` for global).

### 2.7 Multi-source unified syntax — ✅

A single `source = "..."` string handles `local:`, `git:`, `github:`,
`marketplace:` (and future `gitlab:`, `http:` schemes). `skills` only
supports GitHub; AGPM only supports git URLs.

---

## 3. Nice-to-have

These improve the day-to-day experience without being blocking.

### 3.1 `agix tree` — 💭
Visualise the dependency graph (one column per driver, sources, lock state).
Useful for debugging "why is this skill installed?".

### 3.2 Semver ranges in `Agentfile` — 💭
Accept `^1.2`, `~0.4`, `>=2.0,<3.0` for git-tagged packages, with the lock
file pinning the resolved SHA.

### 3.3 `agix publish` — 💭
Push a local skill folder to a registry (when one exists) or to a GitHub
release with the right metadata for `agix find` to discover it.

### 3.4 Interactive secrets prompts — 💭
When installing a package whose MCP server needs `${OPENAI_API_KEY}`, agix
detects the variable and prompts the user (or points at a `.env`) instead
of silently writing a config that won't work.

### 3.5 Timestamped backups on export — 💭
`agix export` defaults to `agix-export-2026-04-28T15-30.zip` (ISO timestamp)
so successive exports don't overwrite each other.

### 3.6 `--dry-run` on install / update / remove — 💭
Print the file mutations that *would* happen, without touching disk.
Pairs naturally with the rich file lifecycle in 2.1.

### 3.7 Windows path support — 💭
First-class handling for Windows config locations
(`%APPDATA%\Claude\settings.json`, `%USERPROFILE%\.cursor\…`). Currently a
known gap; tests are macOS/Linux-only.

### 3.8 `agix upgrade` — 💭
Self-update agix itself from GitHub releases, similar to `rustup self update`
or `gh extension upgrade`.

---

## Tracking

This roadmap is updated as items move between buckets and statuses. The
short-term roadmap (next minor release) is derived from the **must-have**
list above plus whatever differentiator is most valuable to ship next.
