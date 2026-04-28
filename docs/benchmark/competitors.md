# agix vs Competitors

## Positioning

agix is a **universal package manager for AI agent skills/plugins**, designed to work across multiple CLI drivers (Claude Code, Codex, Cursor, Windsurf, …) with a single manifest (`Agentfile`), lock file, and install lifecycle.

---

## Primary competitors

### AGPM (ex-CCPM) — Direct competitor #1

- **Repo:** https://github.com/aig787/agpm (ex https://github.com/aig787/ccpm)
- **Language:** Rust (99.8%), MIT
- **History:** Published as `ccpm` (Claude Code Package Manager) on crates.io, renamed `agpm` (Agentic Package Manager) autumn 2025. v0.4.14 (Dec 2025), ~25 releases.
- **Manifest:** `agpm.toml` (TOML, per-type keys: agents/snippets/commands/skills…, `[sources]` section)
- **Lock file:** ✅ Cargo-style lockfile
- **Sources:** Git-based (GitHub, git URLs)
- **Resource types:** agents, snippets, commands, scripts, hooks, MCP servers
- **Multi-CLI:** ❌ Claude Code only
- **Scopes:** local vs global (`~/.claude`)
- **Export:** ❌ not documented
- **Adoption:** ~1 star, ~1 fork — technically mature, near-zero adoption

### MCS (Managed Claude Stack) — Direct competitor #2

- **Site:** https://mcs-cli.dev
- **Manifest:** `techpack.yaml` (concept of a "tech pack" = declarative multi-resource bundle)
- **Resource types:** MCP servers, plugins, hooks, skills, commands, agents, settings
- **Lock file:** ❓ not confirmed
- **Doctor:** ✅ built-in health checks
- **Hooks/lifecycle:** ✅ event-based automation referenced
- **Multi-CLI:** ❓ Claude-centric on first read
- **Export:** ❓ not documented
- **Adoption:** unknown

### skills (vercel-labs) — Partial overlap, not a direct competitor

- **Repo:** https://github.com/vercel-labs/skills
- **Docs:** https://mintlify.com/vercel-labs/skills
- **Language:** TypeScript/Node, MIT
- **Purpose:** install/remove "Agent Skills" (folders containing a `SKILL.md` with YAML frontmatter) into 45+ coding agents
- **Manifest:** ❌ no `Agentfile`-style declarative project manifest
- **Lock file:** ✅ two lock files (added March 2026)
  - **Global:** `~/.agents/.skill-lock.json` (v3) — per skill: source, sourceType, sourceUrl, skillPath, `skillFolderHash` (GitHub Trees API SHA), installedAt, updatedAt
  - **Local:** `./skills-lock.json` (v1) — meant to be committed to VCS — per skill: source, sourceType, `computedHash` (SHA-256 of file contents)
- **Sources:** GitHub (`owner/repo`), git/GitLab URL, `node_modules`, local path
- **Resource types:** skills only (no agents/hooks/MCP as distinct types)
- **Multi-CLI:** ✅ 45+ agents mapped (Claude Code, Codex, Cursor, Windsurf, Goose, Copilot, Cline…)
- **Install model:** symlinks to a canonical copy (`--copy` fallback)
- **Scopes:** `-g` (global `~/.agents/`) vs project
- **Commands:** `add`, `list`/`ls`, `find`, `remove`/`rm`, `check`, `update`, `init`, `experimental_sync`
- **Export:** ❌
- **Verdict:** overlap on the base operation (`add SKILL.md to .claude/skills/`) but tackles a different problem. Skills-only, no cross-type reproducibility, no declarative project manifest.

---

## Secondary / adjacent

| Tool | Type | Lock | Multi-CLI | Resource types | Notes |
|------|------|------|-----------|----------------|-------|
| **CraftDesk** (`mensfeld/craftdesk`) | Package manager | ✅ SHA-256 | ❓ | skills, agents | GitHub |
| **AGR** (`computerlovetech/agr`) | Package manager | ⚠️ pinned commits | ✅ 6+ agents | skills (GitHub-based) | Python, `-g` flag |
| **ccpi / tonsofskills** (`jeremylongshore`) | Marketplace CLI | ❌ | ✅ | 423 plugins / 2849 skills / 177 agents | Thin wrapper |
| **mcpm.sh** | MCP manager | ❌ | ✅ multi-clients | MCP servers only | Unified config + smart router |
| **Install-MCP** | MCP installer | ❌ | ✅ | MCP servers only | One-liner, very lightweight |

---

## Feature comparison matrix

| Feature | **agix** | **AGPM** | **MCS** | **skills** | **AGR** |
|---------|----------|----------|---------|------------|---------|
| **Language** | Rust | Rust | ❓ | TypeScript | Python |
| **Manifest** | `Agentfile` (TOML) | `agpm.toml` | `techpack.yaml` | ❌ | `agr.toml` |
| **Lock file** | ✅ SHA-based | ✅ Cargo-style | ❓ | ✅ dual (global + local, March 2026) | ⚠️ pinned commits |
| **Multi-CLI** | ✅ (roadmap: 11 drivers) | ❌ Claude only | ❓ | ✅ 45+ agents | ✅ 6+ agents |
| **Walk-up scope** | ✅ | ❌ | ❌ | ❌ | ❌ |
| **Local + global scopes** | ✅ | ✅ | ❓ | ✅ `-g` | ✅ `-g` |
| **Skills** | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Agents** | ✅ | ✅ | ✅ | ❌ | ❌ |
| **Commands** | ✅ | ✅ | ✅ | ❌ | ❌ |
| **Hooks** | ✅ (Claude) | ✅ | ✅ | ❌ | ❌ |
| **MCP servers** | ✅ (roadmap injection) | ✅ | ✅ | ❌ | ❌ |
| **Rules / instructions** | 🗺 | ❌ | ❌ | ❌ | ❌ |
| **Convention-based install** | ✅ | ❌ | ❌ | ✅ (SKILL.md) | ❌ |
| **Declarative + overrides** | 🗺 | ❌ | ❓ | ❌ | ❌ |
| **Export / portability** | ✅ zip `--all` | ❌ | ❌ | ❌ | ❌ |
| **Doctor / health checks** | ✅ | ❌ | ✅ | ❌ | ❌ |
| **Marketplace** | ✅ (Claude) | ❌ | ❌ | ❌ | ❌ |
| **`outdated` check** | ✅ | ❓ | ❌ | ❌ | ❌ |

---

## agix differentiators

1. **Multi-CLI, single manifest** — one `Agentfile` for Claude, Codex, Cursor, Windsurf, etc. AGPM stays Claude-only.
2. **Walk-up scope resolution** — project-level lookup like `.gitignore`. No competitor does it.
3. **Export `--all`** — local + global snapshot in a single zip. Only agix implements it.
4. **Conventions + declarative + overrides** — works out of the box without an Agentfile, configurable when one is present, with per-key overrides.
5. **Rich file lifecycle** — MCP injection, rules, hooks, instructions merge with SHA tracking + conflict detection.
6. **Extended doctor** — active health checks on managed mutations. No competitor combines install + integrity verification on the fragments it has injected.
