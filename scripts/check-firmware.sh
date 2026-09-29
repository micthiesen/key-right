#!/bin/sh
set -eu

# Cross-compilation requires the rustup toolchain's compiler and target libraries.
export PATH="$HOME/.cargo/bin:$PATH"

# Run from the repository root so the storage tests use the host Cargo target.
cargo fmt --manifest-path firmware/app/host-tests/Cargo.toml --check
cargo clippy --manifest-path firmware/app/host-tests/Cargo.toml --all-targets --locked -- -D warnings
cargo test --manifest-path firmware/app/host-tests/Cargo.toml --locked

# The application directory selects the ESP32-C3 target for both images.
cd firmware/app
cargo fmt --check
# Pin the target and artifact directory used by the following stack checks.
export CARGO_BUILD_TARGET=riscv32imc-unknown-none-elf
export CARGO_TARGET_DIR="$PWD/target"
cargo clippy --bin key-right-bench --features bench-light --locked -- -D warnings
cargo build --release --bin key-right-bench --features bench-light --locked
python3 ../../scripts/flash.py --check-stack target/riscv32imc-unknown-none-elf/release/key-right-bench

# Real stock PCA9635 adapter, independent of the commissioning bench simulation.
cargo clippy --bin key-right --features hardware-light --locked -- -D warnings
cargo build --release --bin key-right --features hardware-light --locked
python3 ../../scripts/flash.py --check-stack target/riscv32imc-unknown-none-elf/release/key-right
