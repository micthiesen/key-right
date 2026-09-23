#!/bin/sh
set -eu

# Homebrew's standalone Cargo/rustc can shadow rustup and ignore the toolchain.
export PATH="$HOME/.cargo/bin:$PATH"

# Run from the repository root. This is also the CI validation gate.
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo run -p key-right-cli --locked -- simulate on preset-2 brightness=100 off
python3 -m unittest discover -s scripts/tests -v
