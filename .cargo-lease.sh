#!/bin/sh
cd /home/tom/code/galileo/worktrees/claude-2.21v
exec bun $(dirname "$(agents path machine-capacity)")/scripts/machine-capacity.mjs run --class heavy --owner claude-2.21v --timeout-seconds 1800 -- depbox run -- nix develop -c env CARGO_BUILD_JOBS=4 RUST_TEST_THREADS=4 cargo "$@"
