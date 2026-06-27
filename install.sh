#!/bin/sh
set -eu

REPO_URL="${ECDLP_REPO_URL:-https://github.com/ecdlp-contest/ecdlp-5-bit.git}"
REPO_REF="${ECDLP_REPO_REF:-main}"
INSTALL_DIR="${ECDLP_HOME:-$HOME/.local/share/ecdlp/ecdlp-5-bit}"
BIN_DIR="${ECDLP_BIN_DIR:-$HOME/.local/bin}"
BIN_PATH="$BIN_DIR/ecdlp"

say() { printf '%s\n' "$1"; }
fail() { printf 'error: %s\n' "$1" >&2; exit 1; }
shell_quote() { printf "'%s'" "$(printf '%s' "$1" | sed "s/'/'\\\\''/g")"; }

command -v git >/dev/null 2>&1 || fail "git is required"
command -v node >/dev/null 2>&1 || fail "node is required"
command -v sed >/dev/null 2>&1 || fail "sed is required"

mkdir -p "$(dirname "$INSTALL_DIR")" "$BIN_DIR"

if [ ! -d "$INSTALL_DIR/.git" ]; then
  if [ -e "$INSTALL_DIR" ]; then
    fail "$INSTALL_DIR exists but is not a git checkout; set ECDLP_HOME to another directory"
  fi
  say "Cloning ECDLP contest repo..."
  git clone --depth 1 --branch "$REPO_REF" "$REPO_URL" "$INSTALL_DIR"
else
  say "Updating ECDLP contest repo..."
  if [ -n "$(git -C "$INSTALL_DIR" status --porcelain)" ]; then
    fail "$INSTALL_DIR has local changes; commit/stash them or set ECDLP_HOME to a fresh directory"
  fi
  git -C "$INSTALL_DIR" fetch --depth 1 origin "$REPO_REF"
  git -C "$INSTALL_DIR" checkout -q "$REPO_REF"
  git -C "$INSTALL_DIR" pull --ff-only origin "$REPO_REF"
fi

mkdir -p "$INSTALL_DIR/.workspace"

{
  printf '%s\n' '#!/bin/sh'
  printf 'REPO=%s\n' "$(shell_quote "$INSTALL_DIR")"
  printf 'if [ "$1" = "repo" ]; then printf "%%s\\n" "$REPO"; exit 0; fi\n'
  printf 'cd "$REPO"\n'
  printf 'exec node ./ecdlp.js "$@"\n'
} > "$BIN_PATH"
chmod +x "$BIN_PATH"

say "Installed ecdlp to $BIN_PATH"
say "Contest repo: $INSTALL_DIR"
if ! command -v ecdlp >/dev/null 2>&1; then
  say "Add $BIN_DIR to your PATH to run ecdlp from any shell."
fi
say "Try:"
say "  cd $(shell_quote "$INSTALL_DIR")"
say "  ecdlp --help"
say "  ecdlp package --help"
