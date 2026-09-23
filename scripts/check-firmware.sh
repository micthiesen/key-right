#!/bin/sh
set -eu

# Cross-compilation requires the rustup toolchain's compiler and target libraries.
export PATH="$HOME/.cargo/bin:$PATH"

# Run from the repository root so the storage tests use the host Cargo target.
cargo fmt --manifest-path firmware/app/host-tests/Cargo.toml --check
cargo clippy --manifest-path firmware/app/host-tests/Cargo.toml --all-targets --locked -- -D warnings
cargo test --manifest-path firmware/app/host-tests/Cargo.toml --locked

# The application directory selects the ESP target for the commissioning bench image.
cd firmware/app
cargo fmt --check
cargo clippy --all-targets --features bench-light --locked -- -D warnings
cargo build --release --features bench-light --locked
