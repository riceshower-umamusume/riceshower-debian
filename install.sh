#!/usr/bin/env bash
set -Eeuo pipefail

ROOT=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
CARGO=${CARGO:-}

if [[ -x "$ROOT/target/release/noctalia-setup" ]]; then
  exec "$ROOT/target/release/noctalia-setup" --manifest-dir "$ROOT/manifest" "$@"
fi

if [[ -z $CARGO ]]; then
  if command -v cargo >/dev/null 2>&1; then
    CARGO=$(command -v cargo)
  elif [[ -x "$HOME/.cargo/bin/cargo" ]]; then
    CARGO="$HOME/.cargo/bin/cargo"
  fi
fi

if [[ -n $CARGO ]]; then
  exec "$CARGO" run --quiet --release --manifest-path "$ROOT/Cargo.toml" -- \
    --manifest-dir "$ROOT/manifest" "$@"
fi

printf 'No noctalia-setup binary and no cargo found.\n' >&2
printf 'Install Rust or use a release build.\n' >&2
exit 1
