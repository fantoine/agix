#!/usr/bin/env bash
# Smoke tests for the agix CLI binary.
#
# Runs a happy-path scenario against the release binary in an isolated
# workspace (tempdir cwd + tempdir HOME + AGIX_NO_INTERACTIVE=1) and asserts
# key strings in each command's output. Cleans up every tempdir on exit,
# including on failure.
#
# Usage:
#   scripts/smoke.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="$REPO_ROOT/target/release/agix"

WORKSPACE="$(mktemp -d -t agix-smoke.XXXXXX)"
FAKE_HOME="$(mktemp -d -t agix-smoke-home.XXXXXX)"
LOCAL_PKG="$(mktemp -d -t agix-smoke-pkg.XXXXXX)"

cleanup() {
  rm -rf "$WORKSPACE" "$FAKE_HOME" "$LOCAL_PKG"
}
trap cleanup EXIT

PASS=0
FAIL=0

step() { printf '\n▶ %s\n' "$*"; }
pass() { PASS=$((PASS + 1)); printf '  \033[32m✓\033[0m %s\n' "$*"; }
fail() { FAIL=$((FAIL + 1)); printf '  \033[31m✗\033[0m %s\n' "$*"; }

assert_contains() {
  local needle="$1" haystack="$2" desc="$3"
  if printf '%s' "$haystack" | grep -qF -- "$needle"; then
    pass "$desc"
  else
    fail "$desc (expected substring: $needle)"
    printf '    got:\n'
    printf '%s\n' "$haystack" | sed 's/^/      /'
  fi
}

assert_file_exists() {
  local path="$1" desc="$2"
  if [ -e "$path" ]; then
    pass "$desc"
  else
    fail "$desc (missing: $path)"
  fi
}

agix() {
  env HOME="$FAKE_HOME" AGIX_NO_INTERACTIVE=1 "$BIN" "$@"
}

# ---------------------------------------------------------------------------
# Build
# ---------------------------------------------------------------------------
step "Build release binary"
(cd "$REPO_ROOT" && cargo build --release --quiet)
pass "cargo build --release"

# ---------------------------------------------------------------------------
# --help / --version
# ---------------------------------------------------------------------------
step "agix --help"
OUT="$(agix --help)"
assert_contains "Agent Graph IndeX" "$OUT" "help shows product tagline"
assert_contains "Commands:" "$OUT" "help lists commands"

step "agix --version"
OUT="$(agix --version)"
assert_contains "agix" "$OUT" "version output mentions agix"

# ---------------------------------------------------------------------------
# init → check → doctor
# ---------------------------------------------------------------------------
cd "$WORKSPACE"

step "agix init"
OUT="$(agix init)"
assert_contains "Agentfile" "$OUT" "init announces Agentfile creation"
assert_file_exists "$WORKSPACE/Agentfile" "Agentfile written to cwd"

step "agix check (after seeding [agix].cli)"
printf '[agix]\ncli = ["claude"]\n' > Agentfile
OUT="$(agix check)"
assert_contains "Agentfile valid" "$OUT" "check reports valid manifest"

step "agix doctor"
OUT="$(agix doctor)"
assert_contains "Agentfile: valid" "$OUT" "doctor validates Agentfile"
assert_contains "libgit2" "$OUT" "doctor reports libgit2 version"
assert_contains "git CLI" "$OUT" "doctor reports git CLI detection"

# ---------------------------------------------------------------------------
# list (empty)
# ---------------------------------------------------------------------------
step "agix list (empty manifest)"
OUT="$(agix list)"
assert_contains "No dependencies" "$OUT" "empty-state message"

# ---------------------------------------------------------------------------
# add local → list → outdated
# ---------------------------------------------------------------------------
printf '# smoke skill\n' > "$LOCAL_PKG/skill.md"
NAME="$(basename "$LOCAL_PKG")"

step "agix add local <pkg>"
OUT="$(agix add local "$LOCAL_PKG" 2>&1)"
assert_contains "Added" "$OUT" "add local succeeds"
assert_contains "local:" "$(cat Agentfile)" "Agentfile gains local: source"
assert_file_exists "$WORKSPACE/Agentfile.lock" "lock file was written"

step "agix list (with dep)"
OUT="$(agix list)"
assert_contains "$NAME" "$OUT" "list shows the added dep"

step "agix outdated"
OUT="$(agix outdated)"
assert_contains "local (not checkable)" "$OUT" "local dep gets the expected label"

# ---------------------------------------------------------------------------
# export
# ---------------------------------------------------------------------------
step "agix export"
OUT="$(agix export --output "$WORKSPACE/export.zip" 2>&1)"
assert_contains "Exported" "$OUT" "export command reports success"
assert_file_exists "$WORKSPACE/export.zip" "export.zip produced"

# ---------------------------------------------------------------------------
# remove
# ---------------------------------------------------------------------------
step "agix remove <name>"
OUT="$(agix remove "$NAME" 2>&1)"
assert_contains "Removed" "$OUT" "remove succeeds"

step "agix list (empty again)"
OUT="$(agix list)"
assert_contains "No dependencies" "$OUT" "dep is gone from manifest"

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
printf '\n==============================\n'
printf 'Smoke: \033[32m%d passed\033[0m, ' "$PASS"
if [ "$FAIL" -eq 0 ]; then
  printf '%d failed\n' "$FAIL"
else
  printf '\033[31m%d failed\033[0m\n' "$FAIL"
fi
printf '==============================\n'

[ "$FAIL" -eq 0 ]
