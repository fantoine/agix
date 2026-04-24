<div align="center">
  <img src="assets/icon.svg" width="96" height="96" />

  # Agix

  **Agent Graph IndeX — a universal package manager for AI CLI tools.**

  ![version](https://img.shields.io/badge/version-0.1.3-2b8abf)
  ![license](https://img.shields.io/badge/license-Apache%202.0-blue)
</div>

---

Agix installs, updates, and versions skills, plugins, and agents across the AI CLIs you use (Claude Code, Codex, …) from a single `Agentfile`. Declare what your project needs, commit the manifest, and let Agix reconcile every CLI to match.

## 💡 Why?

Each AI CLI has its own way of loading extensions — `.claude/`, `~/.codex/`, marketplace installs, hand-copied skill files. Sharing a setup with a teammate or reproducing it on a new machine means a lot of manual plumbing.

Agix treats your AI tooling like any other dependency stack:

- **One manifest, many CLIs** — the same `Agentfile` provisions Claude Code, Codex, and more from the same sources.
- **Reproducible installs** — an `Agentfile.lock` pins exact revisions, so a teammate running `agix install` gets the same state you have.
- **Heterogeneous sources** — pull from local paths, GitHub repos, plain git URLs, or a CLI's native marketplace, all through the same commands.
- **Self-contained exports** — package a full working setup (manifest + lock + vendored local sources) as a zip you can drop onto another machine.

## 📦 Installation

### Homebrew

```bash
brew install fantoine/fantoine/agix
```

### Cargo

```bash
cargo install agix
```

Or grab a prebuilt binary from the [Releases page](https://github.com/fantoine/agix/releases/latest).

## 🚀 Getting started

```bash
# Scaffold an Agentfile for the CLIs you use
agix init

# Add a dependency from any supported source
agix add github fantoine/claude-later
agix add local ./path/to/local-skill
agix add marketplace fantoine/claude-plugins@roundtable

# Reproduce the declared state
agix install

# Check what's installed, what's drifted, and what's broken
agix list
agix outdated
agix doctor
```

Further commands: `agix check`, `agix update`, `agix remove`, `agix export`. Run `agix <command> --help` for details.

## Scripting / non-interactive use

Some commands (`agix init`, and first-time global setup triggered by walk-up fallback or `-g`) open an interactive CLI picker. Agix skips the picker automatically when stderr is not a terminal (piped stdin, CI runners, etc.). You can also force non-interactive mode explicitly:

- **`agix init --no-interactive --cli claude --cli codex`** — skip the menu and write exactly the passed CLIs into the new Agentfile.
- **`AGIX_NO_INTERACTIVE=1 agix <command>`** — skip every interactive prompt for the duration of the command. Pass pre-selected CLIs with `--cli` where supported.

When no CLIs are preselected in non-interactive mode the manifest is created with an empty `cli = []`, which you can edit by hand or via `agix add --cli <name>`.

## License

Apache 2.0
