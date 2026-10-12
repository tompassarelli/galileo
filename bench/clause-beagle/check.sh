#!/usr/bin/env bash
set -euo pipefail

if (( $# != 0 )); then
  printf 'usage: FIRN_ROOT=<bench-pin scratch> BEAGLE_PATH=<compiler> %s\n' "$0" >&2
  exit 2
fi
: "${FIRN_ROOT:?Set FIRN_ROOT to the detached bench-pin scratch}"
: "${BEAGLE_PATH:?Set BEAGLE_PATH to the compiler used to stage the pin runtime}"
pin=10740a8ae2fd9aa327a7d30f7e7109ea797b9fa9
scratch="$HOME/code/nixos-config/worktrees/bench-pin"
if [[ $(readlink -f "$FIRN_ROOT") != "$scratch" ||
      $(git -C "$FIRN_ROOT" rev-parse HEAD) != "$pin" ]] ||
   git -C "$FIRN_ROOT" symbolic-ref -q HEAD >/dev/null; then
  printf 'clause-beagle: create the detached scratch:\ngit -C ~/code/nixos-config/main worktree add --detach ~/code/nixos-config/worktrees/bench-pin %s\nremove it after checking:\ngit -C ~/code/nixos-config/main worktree remove ~/code/nixos-config/worktrees/bench-pin\n' "$pin" >&2
  exit 2
fi
export FIRN_REPO="$FIRN_ROOT"
export FIRN_RUNTIME_ROOT="$FIRN_ROOT/.firn-build/runtime/$pin"
manifest="$FIRN_RUNTIME_ROOT/current/manifest"
if [[ ! -f "$manifest" ]] ||
   ! rg -q -x "firn_revision=$pin" "$manifest" ||
   ! rg -q -x "clause_revision=$(cat "$FIRN_ROOT/config/clause-revision")" "$manifest" ||
   ! rg -q -x "beagle_revision=$(git -C "$BEAGLE_PATH" rev-parse HEAD)" "$manifest"; then
  printf 'clause-beagle: stage this pin:\nFIRN_REPO=$FIRN_ROOT FIRN_RUNTIME_ROOT=$FIRN_RUNTIME_ROOT BEAGLE_PATH=$BEAGLE_PATH $FIRN_ROOT/dotfiles/bin/firn-runtime-update repo\n' >&2
  exit 2
fi
unset FIRN_BUN FIRN_DISPATCHER_MODULE FIRN_REPO_BUILD_MODULE FIRN_SCHEMA_MODULE FIRN_CLAUSE_WORKBENCH
SECONDS=0
trap 'printf "wall seconds: %s\n" "$SECONDS"' EXIT
printf 'firn repo build incremental: no; every source is compiled, and identical outputs retain their timestamps.\n'
printf 'command: FIRN_REPO=%q FIRN_RUNTIME_ROOT=%q BEAGLE_PATH=%q %q repo build\n' \
  "$FIRN_REPO" "$FIRN_RUNTIME_ROOT" "$BEAGLE_PATH" "$FIRN_ROOT/dotfiles/bin/firn"
"$FIRN_ROOT/dotfiles/bin/firn" repo build
printf 'command: FIRN_REPO=%q FIRN_RUNTIME_ROOT=%q BEAGLE_PATH=%q %q repo validate\n' \
  "$FIRN_REPO" "$FIRN_RUNTIME_ROOT" "$BEAGLE_PATH" "$FIRN_ROOT/dotfiles/bin/firn"
"$FIRN_ROOT/dotfiles/bin/firn" repo validate
