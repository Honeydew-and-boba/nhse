#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Builds the Rust part of nhse.nro: crates/nhse-switch as a static library for the Switch 2
# (target/aarch64-switch-horizon/release/libnhse_switch.a), with the Rust standard library
# built for Horizon from source.
#
# Needs Rust 1.97 or newer with the rust-src component (rustup component add rust-src).
# The target (aarch64-switch-horizon.json, from OunceVK) is a custom target and build-std is
# unstable, so RUSTC_BOOTSTRAP lets a stable toolchain build them.
set -eu

HERE=$(cd "$(dirname "$0")" && pwd)
export RUSTC_BOOTSTRAP=1
export RUST_TARGET_PATH="$HERE"
cd "$HERE/.."
exec cargo build --release -p nhse-switch --lib \
    --target "$HERE/aarch64-switch-horizon.json" -Zjson-target-spec \
    -Zbuild-std=core,alloc,std,panic_abort "$@"
