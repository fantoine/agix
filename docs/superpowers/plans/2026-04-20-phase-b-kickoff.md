# Phase B Kickoff — Agix Command Review

> **Status at kickoff:** main branch, pushed to origin, 71 tests green, 21 commits ahead of v0.1.0 tag.
> **Last commit:** `4482b10 refactor: rename claude-code driver to claude`

This doc is a resume-cold memo for the controller agent driving Phase B (Tasks 8–19 of the command-review plan). It assumes no prior conversation context — only the state of the repo.

---

## 1. Checkpoint

- **Branch:** `main` (user explicitly approved direct work on main during v0.1.0 dev; no worktree).
- **Push state:** `origin/main` == `HEAD` at kickoff.
- **Test count:** 71 passing across 17 integration suites + inline unit tests.
- **Clippy:** clean except one pre-existing `manual_split_once` warning in `src/sources/github.rs` (out of scope — do NOT fix).
- **MSRV / toolchain:** stable Rust 2021.

### Phase A recap (landed)

| Task | Commit | What shipped |
|------|--------|--------------|
| 1    | `6e13f80` | Review infrastructure: findings log, test fixtures, helpers |
| 2    | `fd6081b` + `a51bc13` | `Source::suggested_name` (Local/GitHub/Git/Marketplace) |
| 3    | `368d2a2` | CLI syntax `add <type> <value>` (separate positional args) |
| 4    | `8c8ccff` | Marketplace install delegated to CLI drivers via `claude` CLI |
| 5    | `ae8196b` | Interactive `MultiSelect` at `init`; `AGIX_NO_INTERACTIVE` env var |
| 6    | `d92ac8e` | `CliDriver::detect_local_config`; doctor prints per-driver local config status |
| 7    | `b654db0` | `export` zip is self-contained (local-sources vendored, paths rewritten) |
| A1   | `ad947a5` | **Source trait + scheme registry** (replaced `SourceSpec` enum; mirrors `CliDriver` pattern) |
| A2   | `4482b10` | **Driver rename:** `claude-code` → `claude` (module, type `ClaudeDriver`, all strings, `examples/claude/`) |

### Architectural deltas since the plan was written

- `SourceSpec` enum **deleted**. Replaced by `Source` trait + `SourceScheme` registry in `src/sources/mod.rs`. `parse_source(&str) -> Result<Box<dyn Source>>`, `scheme_names()`, `all_schemes()`.
- `FetchOutcome::{Fetched{path, sha, content_hash} | DelegateToDriver{marketplace, plugin}}` unifies source results.
- Driver name is now `"claude"` (not `"claude-code"`). TOML sections are `[claude.dependencies]`. `--cli claude`.
- `which::which("claude")` and `~/.claude/` paths in the driver are UNCHANGED — they refer to Anthropic's real binary/config, not the internal driver name.
- `AGIX_NO_INTERACTIVE=1` and `--no-interactive` (on `init`) are the test escape hatches for TTY prompts.

---

## 2. Deferred findings — Phase B targets

From `docs/superpowers/plans/2026-04-19-findings.md`. Each entry tags a Phase B task. When executing that task, re-read the finding and decide: fix + regression test, or keep deferred with rationale.

| Target | Finding | Severity |
|--------|---------|----------|
| Task 10 (`add`) | `add --scope global` auto-init ignores non-interactive caller state | minor |
| Task 10 (`add`) | Marketplace total-failure returns Ok; should propagate non-zero | minor |
| Task 11 (`remove`) | Mangled lock `source` aborts `uninstall` before file cleanup | minor |
| Task 12 (`install`) | `claude plugin install` on already-installed plugin warns non-zero | cosmetic |
| Task 16 (`doctor`) | Marketplace packages silently pass (no files to check) | minor |
| Task 18 (cross-cutting) | `AGIX_NO_INTERACTIVE` undocumented (no `--help`, no README) | minor |
| Task 18 (cross-cutting) | No unified test helper for non-interactive mode | minor |
| Task 18 (cross-cutting) | Integration tests leak into real `$HOME` (`~/.claude`, `~/.codex`) | minor |

Also queued via `later_push` (follow-up, not Phase B): extract repeated scheme/driver string literals into shared constants.

---

## 3. Phase B task list

All tasks are in `docs/superpowers/plans/2026-04-19-command-review.md`. Line numbers:

| # | Task | Line | Command | Notes |
|---|------|------|---------|-------|
| 8  | Review `init`     | 1238 | `init`     | Tasks 8 covers the `--cli unknown` scenario etc. Seam-sensitive to Task 5 changes. |
| 9  | Review `check`    | 1295 | `check`    | Read `src/commands/check.rs` first. |
| 10 | Review `add`      | 1350 | `add`      | Address Task 5 `agentfile_paths` auto-init seam + marketplace total-failure finding. |
| 11 | Review `remove`   | 1377 | `remove`   | Address mangled-lock-source finding. |
| 12 | Review `install`  | 1395 | `install`  | Address claude idempotency-noise finding. Golden path for `AGIX_NO_INTERACTIVE`. |
| 13 | Review `update`   | 1417 | `update`   | |
| 14 | Review `list`     | 1435 | `list`     | |
| 15 | Review `outdated` | 1452 | `outdated` | |
| 16 | Review `doctor`   | 1472 | `doctor`   | Address marketplace-silent-pass finding. |
| 17 | Review `export`   | 1490 | `export`   | Read roundtrip test first; Phase A already hardened. |
| 18 | Cross-cutting     | 1511 | — | `AGIX_NO_INTERACTIVE` docs, test helper, `$HOME` leakage, stray `println!`, exit codes. |
| 19 | Final sweep       | 1535 | — | Full `cargo test` / `cargo fmt --check` / `cargo clippy -D warnings` + findings summary. |

Plan task numbering note: Phase B tasks in the plan doc are numbered 8–19 as shown above. **The internal TaskList IDs (`#36`, `#37`, `#38`, ...) do NOT match the plan task numbers** — they're a separate tracking index. When dispatching, always reference the plan task number, not the TaskList ID.

---

## 4. Execution model — mega-subagent per task

To preserve main context, dispatch ONE subagent per task that runs the full subagent-driven-development cycle internally. It dispatches its own implementer + spec reviewer + code quality reviewer and only surfaces a consolidated result.

### Per-task dispatch template

```
Agent (general-purpose):
  description: "Phase B Task N review"
  prompt: (see template below)
```

**Mandatory brief content for the mega-subagent:**

1. **Working directory:** `/Users/fantoine/Documents/Perso/Code/IA/Agix`, branch `main`. No worktrees.
2. **Current test count:** `<N>` (update per batch).
3. **Task reference:** full task text extracted from the plan at lines `<start>-<end>`. Do NOT tell the subagent to read the plan file — extract and paste.
4. **Deferred findings for this task:** paste verbatim from findings log. Tell the subagent: "fix if easy + add regression test; otherwise keep deferred and re-annotate with new rationale."
5. **Post-Phase-A context:** enumerate the architectural deltas relevant to the command under review (e.g. for `add`: "sources use `parse_source()`, not `SourceSpec`. Validation is via `scheme_names()`. Driver name is `claude`, not `claude-code`.").
6. **Out of scope (strict):** pre-existing `manual_split_once` warning; historical docs under `docs/superpowers/plans/*.md` (except the findings log).
7. **Workflow (required):** dispatch implementer → read report → dispatch spec reviewer → fix loop until ✅ → dispatch code quality reviewer → fix loop until Approved. Commit once per task.
8. **Conventions:**
   - Stage only touched files. NEVER `git add -A` (`.idea/` and `.claude/` sit untracked).
   - Commit message prefix: `review(<command>):` for Phase B task commits.
   - New findings go into `docs/superpowers/plans/2026-04-19-findings.md` and get their own commit with `docs: log Task <N> review findings`.
   - Fixed findings: update their `**Fix commit:**` line to the SHA that fixed them.
9. **Reporting back (single message):**
   - STATUS (DONE / DONE_WITH_CONCERNS / BLOCKED)
   - Review commit SHA (or "no code changes — deferrals only")
   - Findings commit SHA (if any)
   - New test count
   - Findings added/fixed/re-deferred (1-line each)
   - Any Phase A decisions that became wrong in light of what was reviewed

### What stays in main context

- Reading the mega-subagent report (concise).
- Updating the TaskList (in_progress → completed).
- Optional: spot-checking the commit with `git show --stat`.
- Deciding batch transitions (commit/push/compact).

That's it. All the plan reads, scenario runs, subagent orchestration, review iterations — pushed into the subagent. Main sees ~200 words per task instead of ~2000.

---

## 5. Batching plan

| Batch | Tasks | Expected new commits | End-of-batch action |
|-------|-------|----------------------|---------------------|
| B1 | 8, 9, 10, 11  | 4 review commits + 1-2 findings commits | push to origin/main |
| B2 | 12, 13, 14, 15 | same | push |
| B3 | 16, 17, 18, 19 | Task 18 may be large (multi-file sweep); Task 19 is the close-out summary | push, tag `v0.1.0-rc1` if green |

Between batches: if main-session context is tight, `/compact` is acceptable — this kickoff doc + the findings log together let the next batch resume cold. The mega-subagent model means each task is self-contained, so compaction mid-batch is also safe (you lose the subagent reports but the repo state + TaskList + findings log remain authoritative).

---

## 6. Known traps / conventions

- **Driver name is `"claude"`.** Every Agentfile TOML key, `--cli` arg, and test fixture uses it. `"claude-code"` is NOT valid anywhere in code/tests/examples.
- **`SourceSpec` is gone.** Use `crate::sources::parse_source()` and the `Source` trait. `src.local_path()` replaces `if let SourceSpec::Local`. `src.as_marketplace()` replaces `if let SourceSpec::Marketplace`.
- **Async sources:** `Source::fetch` is `async fn` via `async_trait`. Callers await it.
- **Main branch work is OK for v0.1.0 dev.** Do not spin up worktrees during Phase B.
- **`AGIX_NO_INTERACTIVE=1`** required in every integration test that invokes `init`/`add`/`install` without explicit `--no-interactive`. Missing this flag deadlocks CI.
- **Commit messages:** `review(<command>): <summary>` for Phase B per-command commits. `docs: log Task <N> review findings` for findings commits. `review(core): cross-cutting consistency fixes` for Task 18. `review: summary and close-out` for Task 19.
- **Never touch** `docs/superpowers/plans/2026-04-19-command-review.md` (plan itself), `docs/superpowers/plans/2026-04-16-*.md` (original design), or `docs/superpowers/specs/*.md` (historical specs). Only `docs/superpowers/plans/2026-04-19-findings.md` is a living document.
- **Test helper status:** not yet created. Task 18 should create `tests/helpers/mod.rs` with a `cmd_non_interactive()` helper and migrate existing tests.
- **`$HOME` leakage in tests:** Task 18 should add `HOME` override to `cmd_non_interactive()` so integration tests become hermetic.
