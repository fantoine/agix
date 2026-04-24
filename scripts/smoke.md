# Smoke test coverage

Tracking list for `scripts/smoke.sh`. Tick a box when the corresponding
assertion lands in the script. Keep entries short — details live in the
script itself.

## Implemented

### Binary sanity
- [x] `agix --help` — shows product tagline + command list
- [x] `agix --version` — mentions `agix`

### Core commands (happy path)
- [x] `agix init` — creates `Agentfile`
- [x] `agix check` — validates a minimal manifest
- [x] `agix doctor` — reports Agentfile status + libgit2 version + git CLI
- [x] `agix list` — empty-state message
- [x] `agix add local <dir>` — updates `Agentfile` and writes `Agentfile.lock`
- [x] `agix list` — shows the added dep
- [x] `agix outdated` — labels `local` deps as not remotely checkable
- [x] `agix export --output <zip>` — produces the archive
- [x] `agix export --all --output <zip>` — both local + global scopes under `local/` and `global/` prefixes
- [x] `agix remove <name>` — removes from manifest and lock

## Pending

### Sources
- [ ] `agix add git <file://bare-repo>` — bootstrap a bare repo in-script
- [ ] `agix add github <org/repo>` — via `AGIX_GITHUB_BASE_URL` + HTTP mock
  (may be out of scope for smoke; unit tests already cover it via mockito)
- [ ] `agix add marketplace <m/p@plugin>` — with a `claude` shim on `PATH`

### Flows
- [ ] `agix install` — manifest pre-written, no prior `add`
- [ ] `agix update <name>` — full refresh cycle
- [ ] `agix update` — all-deps refresh with multiple packages
- [ ] `agix add … --cli claude --cli codex` — writes to each `[<cli>.dependencies]`
- [ ] `agix remove … --cli claude` — partial filter
- [x] `-g/--global` — walk-up scope + global flag (walk-up from subdir, `-g init`, `-g add`, `-g list`, precedence, `--scope` rejected)

### Error paths (exit code ≠ 0)
- [ ] `agix check` in a dir without `Agentfile` — actionable error
- [ ] `agix add ftp:whatever` — "unknown source type"
- [ ] `agix add local /tmp/x --cli unknown` — "unknown CLI"
- [ ] `agix outdated` without lock — points at `agix install`
- [ ] `agix remove <missing>` — "not in lock file"

### Roundtrips
- [ ] Extract `export.zip` in a fresh dir + run `agix install` there
- [ ] Lock file stays parseable after `remove` (re-open, re-parse)
- [ ] Confirm `AGIX_NO_INTERACTIVE=1` keeps every command non-interactive
