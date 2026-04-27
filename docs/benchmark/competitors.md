# agix vs Competitors

## Positioning

agix is a **universal package manager for AI agent skills/plugins**, designed to work across multiple CLI drivers (Claude Code, Codex, Cursor, Windsurf, …) with a single manifest (`Agentfile`), lock file, and install lifecycle.

---

## Primary competitors

### AGPM (ex-CCPM) — Concurrent frontal #1

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

### MCS (Managed Claude Stack) — Concurrent frontal #2

- **Site:** https://mcs-cli.dev
- **Manifest:** `techpack.yaml` (concept "tech pack" = bundle declaratif multi-types)
- **Resource types:** MCP servers, plugins, hooks, skills, commands, agents, settings
- **Lock file:** ❓ not confirmed
- **Doctor:** ✅ health checks intégrés
- **Hooks/lifecycle:** ✅ event-based automation mentionné
- **Multi-CLI:** ❓ Claude-centric a priori
- **Export:** ❓ not documented
- **Adoption:** inconnu

### skills (vercel-labs) — Overlap partiel, non-concurrent direct

- **Repo:** https://github.com/vercel-labs/skills
- **Docs:** https://mintlify.com/vercel-labs/skills
- **Language:** TypeScript/Node, MIT
- **Purpose:** installer/retirer des "Agent Skills" (dossiers `SKILL.md` + frontmatter YAML) dans 45+ coding agents
- **Manifest:** ❌ pas d'Agentfile/manifeste projet déclaratif
- **Lock file:** ✅ deux lock files (ajouté mars 2026)
  - **Global:** `~/.agents/.skill-lock.json` (v3) — par skill: source, sourceType, sourceUrl, skillPath, `skillFolderHash` (GitHub Trees API SHA), installedAt, updatedAt
  - **Local:** `./skills-lock.json` (v1) — à committer en VCS — par skill: source, sourceType, `computedHash` (SHA-256 des contenus de fichiers)
- **Sources:** GitHub (`owner/repo`), git/GitLab URL, `node_modules`, chemin local
- **Resource types:** skills uniquement (pas agents/hooks/MCP comme types distincts)
- **Multi-CLI:** ✅ 45+ agents mappés (Claude Code, Codex, Cursor, Windsurf, Goose, Copilot, Cline…)
- **Install model:** symlinks vers copie canonique (fallback `--copy`)
- **Scopes:** `-g` (global `~/.agents/`) vs project
- **Commands:** `add`, `list`/`ls`, `find`, `remove`/`rm`, `check`, `update`, `init`, `experimental_sync`
- **Export:** ❌
- **Verdict:** overlap sur l'action de base (`add SKILL.md dans .claude/skills/`), adresse des problèmes différents. Skills-only, pas de reproductibilité cross-types, pas de manifeste déclaratif projet.

---

## Secondary / adjacent

| Tool | Type | Lock | Multi-CLI | Resource types | Notes |
|------|------|------|-----------|----------------|-------|
| **CraftDesk** (`mensfeld/craftdesk`) | Package manager | ✅ SHA-256 | ❓ | skills, agents | GitHub |
| **AGR** (`computerlovetech/agr`) | Package manager | ⚠️ pinned commits | ✅ 6+ agents | skills (GitHub-based) | Python, `-g` flag |
| **ccpi / tonsofskills** (`jeremylongshore`) | Marketplace CLI | ❌ | ✅ | 423 plugins / 2849 skills / 177 agents | Thin wrapper |
| **mcpm.sh** | MCP manager | ❌ | ✅ multi-clients | MCP servers only | Unified config + smart router |
| **Install-MCP** | MCP installer | ❌ | ✅ | MCP servers only | One-liner, très léger |

---

## Feature comparison matrix

| Feature | **agix** | **AGPM** | **MCS** | **skills** | **AGR** |
|---------|----------|----------|---------|------------|---------|
| **Language** | Rust | Rust | ❓ | TypeScript | Python |
| **Manifest** | `Agentfile` (TOML) | `agpm.toml` | `techpack.yaml` | ❌ | `agr.toml` |
| **Lock file** | ✅ SHA-based | ✅ Cargo-style | ❓ | ✅ dual (global + local, mars 2026) | ⚠️ pinned commits |
| **Multi-CLI** | ✅ (roadmap: 11 drivers) | ❌ Claude only | ❓ | ✅ 45+ agents | ✅ 6+ agents |
| **Walk-up scope** | ✅ | ❌ | ❌ | ❌ | ❌ |
| **Local + global scopes** | ✅ | ✅ | ❓ | ✅ `-g` | ✅ `-g` |
| **Skills** | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Agents** | ✅ | ✅ | ✅ | ❌ | ❌ |
| **Commands** | ✅ | ✅ | ✅ | ❌ | ❌ |
| **Hooks** | ✅ (Claude) | ✅ | ✅ | ❌ | ❌ |
| **MCP servers** | ✅ (roadmap injection) | ✅ | ✅ | ❌ | ❌ |
| **Rules / instructions** | 🗺 | ❌ | ❌ | ❌ | ❌ |
| **Convention-based install** | 🗺 | ❌ | ❌ | ✅ (SKILL.md) | ❌ |
| **Declarative + overrides** | 🗺 | ❌ | ❓ | ❌ | ❌ |
| **Export / portability** | ✅ zip `--all` | ❌ | ❌ | ❌ | ❌ |
| **Doctor / health checks** | ✅ | ❌ | ✅ | ❌ | ❌ |
| **Marketplace** | ✅ (Claude) | ❌ | ❌ | ❌ | ❌ |
| **`outdated` check** | ✅ | ❓ | ❌ | ❌ | ❌ |

---

## agix differentiators

1. **Multi-CLI, single manifest** — un `Agentfile` pour Claude, Codex, Cursor, Windsurf, etc. AGPM reste Claude-only.
2. **Scope walk-up** — résolution projet-level comme `.gitignore`, aucun concurrent ne fait ça.
3. **Export `--all`** — snapshot local + global dans un seul zip, seul agix implémente ça.
4. **Convention + déclaratif + overrides** — install out-of-the-box sans Agentfile, configurable avec.
5. **Rich file lifecycle** — MCP injection, rules, hooks, instructions merge avec tracking SHA + conflict detection.
6. **Doctor étendu** — vérification active des mutations managed, aucun concurrent ne fait les deux (install + health check des fichiers managed).
