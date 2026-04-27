# agix vs Competitors

## Positioning

agix is a **universal package manager for AI agent skills/plugins**, designed to work across multiple CLI drivers (Claude Code, Codex, Cursor, Windsurf, …) with a single manifest (`Agentfile`), lock file, and install lifecycle.

---

## Comparison table

| Feature | **agix** | **skills (Vercel)** | **gh skill** | **mise / asdf** | **pip / npm / cargo** |
|---------|----------|---------------------|--------------|------------------|-----------------------|
| **Primary purpose** | AI skill/plugin manager (multi-CLI) | AI skill registry + install (Vercel SDK) | Copilot skill install (GitHub-only) | Developer tool version manager | Language package manager |
| **Multi-CLI support** | ✅ Claude, Codex, Cursor, Windsurf, Gemini, Vibe… | ❌ Vercel AI SDK only | ❌ GitHub Copilot only | ❌ Dev tools, not AI agents | ❌ Single ecosystem |
| **Agentfile manifest** | ✅ TOML, per-project + global | ❌ | ❌ | ✅ `.mise.toml` | ✅ `package.json` / `Cargo.toml` |
| **Lock file** | ✅ `Agentfile.lock` | ❌ | ❌ | ✅ | ✅ |
| **Local + global scopes** | ✅ walk-up + `~/.agix/` | ❌ | ❌ | ✅ | ⚠️ user vs project |
| **Scope walk-up resolution** | ✅ cwd → parent → `$HOME` | ❌ | ❌ | ❌ | ❌ |
| **Export / portability** | ✅ `agix export` (zip, `--all`) | ❌ | ❌ | ❌ | ⚠️ tarball only |
| **Multiple sources** | ✅ local, git, github, marketplace | ⚠️ registry only | ⚠️ GitHub only | ✅ | ✅ |
| **CLI-specific file install** | ✅ (roadmap: rules, MCP, hooks…) | ❌ | ❌ | ❌ | ❌ |
| **Convention-based install** | ✅ (roadmap) | ❌ | ❌ | ❌ | ❌ |
| **MCP server install** | ✅ (roadmap) | ❌ | ❌ | ❌ | ❌ |
| **Marketplace integration** | ✅ (Claude marketplace via CLI) | ⚠️ own registry | ❌ | ❌ | ✅ |
| **`doctor` / health check** | ✅ | ❌ | ❌ | ⚠️ | ❌ |
| **`outdated` check** | ✅ | ❌ | ❌ | ✅ | ✅ |
| **AI-agent aware** | ✅ (designed for it) | ✅ (Vercel AI) | ✅ (Copilot) | ❌ | ❌ |
| **Open source** | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Language** | Rust | TypeScript | Go | Rust | varies |

---

## Notes per competitor

### skills (Vercel)
Registry and install tool for Vercel AI SDK skills. Focused exclusively on the Vercel/Next.js AI ecosystem. Not a direct competitor — different target audience (web developers using Vercel AI SDK vs developers managing AI agent tooling). No multi-CLI concept, no lock file, no local/global scopes.

### gh skill
GitHub CLI extension (`gh skill install …`) for installing Copilot-compatible skills from GitHub repositories. Tightly coupled to GitHub Copilot. No universal manifest, no lock file, no lifecycle management. More of a convenience wrapper than a package manager.

### mise / asdf
General-purpose developer tool version managers. Not AI-agent aware. No concept of skill install, MCP config, rules injection, etc. Comparable only on manifest + lock + version management mechanics — agix borrows the UX pattern but specialises it for AI tooling.

### pip / npm / cargo
Language-specific package managers. Manage code dependencies, not AI agent skills/config. Not competitors — agix is complementary (a skill package installed by agix may itself have npm dependencies).

---

## agix differentiators

1. **Multi-CLI, single manifest** — one `Agentfile` works for Claude, Codex, Cursor, Windsurf, etc.
2. **Rich install lifecycle** — not just file copy: MCP config injection, rules injection, hooks, instructions merge, with update/remove tracking.
3. **Convention + declarative** — works out of the box for any skill package, configurable for complex cases.
4. **Scope walk-up** — project-level resolution like `.gitignore` / `package.json` semantics.
5. **Portability** — `export --all` snapshots both local and global state into a single zip for backup/migration.
6. **Doctor** — active health checks, conflict detection on managed file fragments.
