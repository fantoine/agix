# CLI Drivers — Supported Elements

Reference for what each CLI driver supports (current + roadmap).
Legend: ✅ supported today · 🗺 roadmap · ❌ not applicable / unknown

---

## Element support matrix

| Element | claude | codex | cursor | windsurf | gemini | vibe | goose | cline | copilot | opencode | amp |
|---------|--------|-------|--------|----------|--------|------|-------|-------|---------|----------|-----|
| **Skills dir** | ✅ | ❌ | ❌ | ✅ | ❌ | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |
| **Agents dir** | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ | ❌ |
| **Commands dir** | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| **Rules / instructions** | ✅ | 🗺 | 🗺 | 🗺 | 🗺 | 🗺 | 🗺 | 🗺 | 🗺 | 🗺 | 🗺 |
| **MCP server config** | ✅ | ❌ | 🗺 | 🗺 | 🗺 | 🗺 | 🗺 | 🗺 | 🗺 | 🗺 | 🗺 |
| **Hooks / events** | 🗺 | ❌ | ❌ | 🗺 | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| **Instructions merge** | 🗺 | 🗺 | ❌ | 🗺 | 🗺 | ❌ | 🗺 | ❌ | 🗺 | 🗺 | ❌ |
| **Prompts / system prompt** | ❌ | ❌ | ❌ | ❌ | ❌ | 🗺 | ❌ | ❌ | ❌ | ❌ | ❌ |
| **Marketplace plugin** | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| **Detect local config** | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| **doctor checks** | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |

---

## Per-driver details

### Claude Code (`claude`)
- **Config dir:** `~/.claude/`
- **Skills:** `~/.claude/` (convention: `skills/` subdir)
- **Agents:** `~/.claude/` (convention: `agents/` subdir)
- **Commands:** `~/.claude/commands/*.md`
- **Rules:** `~/.claude/rules/*.md`
- **MCP:** `~/.claude/settings.json` → `mcpServers` object (JSON key injection)
- **Hooks:** `~/.claude/settings.json` → `hooks` array (JSON key injection) — events: PreToolUse, PostToolUse, Stop, Notification
- **Instructions merge:** `~/.claude/CLAUDE.md` (marker injection)
- **allowedTools:** `~/.claude/settings.json` (JSON key injection)
- **Marketplace:** via `claude` CLI — `install_marketplace_plugin` driver method

### Codex (`codex`)
- **Config dir:** `~/.codex/`
- **Agents:** convention `agents/` dir
- **Instructions merge:** `AGENTS.md` at project root (marker injection)
- **MCP:** unknown / not documented

### Cursor (`cursor`)
- **Config dir:** `~/.cursor/`
- **Rules:** `.cursorrules` (project root, marker injection)
- **MCP:** `~/.cursor/mcp.json` (JSON key injection, `mcpServers`)
- **Skills/agents:** no dedicated concept

### Windsurf (`windsurf`)
- **Config dir:** `~/.codeium/windsurf/`
- **Rules:** `.windsurfrules` (project root, marker injection); also reads `AGENTS.md`
- **MCP:** Cascade MCP settings (JSON key injection)
- **Hooks:** Cascade Hooks + Workflows (JSON/YAML config — format TBD)
- **Skills:** `~/.codeium/windsurf/` skills directory
- **Instructions merge:** `AGENTS.md` (marker injection)
- **Memories:** Cascade Memories — read-only from agix perspective

### Gemini CLI (`gemini`)
- **Config dir:** `~/.gemini/`
- **Rules / instructions merge:** `GEMINI.md` (marker injection)
- **MCP:** `~/.gemini/settings.json` → `[[mcpServers]]` (TOML or JSON, format TBD)
- **Skills:** no dedicated concept

### Mistral Vibe (`vibe`)
- **Config dir:** `~/.vibe/` (global) · `.vibe/` (project)
- **Skills:** `~/.vibe/skills/` (global) · `.vibe/skills/` (project)
- **Prompts:** `~/.vibe/prompts/*.md` (marker injection)
- **MCP:** `~/.vibe/config.toml` → `[[mcp_servers]]` (TOML array entry injection)
- **Tools config:** `~/.vibe/config.toml` → `disabled_tools` / `[tools.*]`
- **Agents:** `~/.vibe/agents/*.toml` (copy)

### Goose (`goose`)
- **Config dir:** `~/.goose/` (probable — not fully documented)
- **MCP:** extensions config (format TBD — likely YAML or JSON)
- **Instructions merge:** `AGENTS.md` (marker injection)
- **Skills:** `.agents/skills/` (agentskills.io standard)
- **Custom distributions:** bundled provider + extensions config — agix could generate a distribution config

### Cline (`cline`)
- **Config dir:** VS Code extension settings
- **Rules:** `.clinerules` (project root, marker injection)
- **MCP:** VS Code `settings.json` → `cline.mcpServers` (JSON key injection)

### GitHub Copilot (`copilot`)
- **Config dir:** `.github/` (project) · `~/.vscode/` (global)
- **Instructions:** `.github/copilot-instructions.md` (marker injection)
- **MCP:** `~/.vscode/mcp.json` (JSON key injection, `servers`)
- **Skills:** `gh skill install` command — agix calls `gh` CLI (like Claude marketplace pattern)

### OpenCode (`opencode`)
- **Config dir:** `~/.opencode/`
- **Instructions merge:** `AGENTS.md` (marker injection)
- **MCP:** `~/.opencode/config.json` → `mcp` section (JSON key injection)

### Amp (`amp`)
- **Config dir:** `~/.amp/`
- **Rules:** `.amp/rules.md` (marker injection)
- **MCP:** `~/.amp/settings.json` (JSON key injection)

---

## Convention scan defaults

When a package has no explicit `[install.<driver>]`, agix scans for these dirs/files:

```
skills/        → all drivers that have a skills dir
agents/        → claude, codex, opencode
commands/      → claude
rules/         → claude, cursor, windsurf, cline, amp
mcp/           → all drivers (*.json files → MCP config injection)
hooks/         → claude
prompts/       → vibe
CLAUDE.md      → claude (marker inject)
AGENTS.md      → codex, opencode, windsurf, goose (marker inject)
GEMINI.md      → gemini (marker inject)
```

---

## Open questions / TBD

- **Windsurf Hooks:** config format not publicly documented. May need reverse-engineering or waiting for official docs.
- **Goose config:** exact config file path and MCP format not confirmed — likely `~/.config/goose/` or `~/.goose/`.
- **Gemini CLI MCP:** format may be JSON or TOML — need to verify against latest CLI release.
- **gh skill:** whether agix should call `gh skill install` (like marketplace pattern) or inject into Copilot config directly.
- **Cline:** VS Code extension settings path is user-platform-dependent; Windows paths differ.
