#!/bin/zsh
# SPDX-FileCopyrightText: 2026 GooBeom Jeoung
# SPDX-License-Identifier: GPL-3.0-only
#
# Builds the Rust core and generates its Swift bindings into
# Sources/RoamlingCoreRs, which SwiftPM compiles and links statically.
#
# Static on purpose. A dylib works -- docs/windows.md W0m.3 measured it through
# rpath and codesign -- but it means bundling, an install-name fixup, and a
# second signature. A static archive is one file the linker eats.
#
# Always release: the core is arithmetic, a debug build of it is slow for no
# benefit, and one configuration means one path.

set -euo pipefail

REPOSITORY_DIR="${0:A:h:h}"
cd "$REPOSITORY_DIR"

if ! command -v cargo >/dev/null 2>&1; then
  print -u2 "cargo not found: install Rust (https://rustup.rs) -- the core needs it"
  exit 1
fi

GENERATED="$REPOSITORY_DIR/Sources/RoamlingCoreRs"
HEADERS="$REPOSITORY_DIR/Sources/CRoamlingCoreFFI"
LIB_DIR="$REPOSITORY_DIR/.build/rust"
mkdir -p "$GENERATED" "$HEADERS" "$LIB_DIR"

cargo build --quiet --release --manifest-path "$REPOSITORY_DIR/rust/Cargo.toml" -p roamling-core

ARCHIVE="$REPOSITORY_DIR/rust/target/release/libroamling_core.a"
DYLIB="$REPOSITORY_DIR/rust/target/release/libroamling_core.dylib"
[[ -f "$ARCHIVE" && -f "$DYLIB" ]] || { print -u2 "cargo produced no library"; exit 1; }

# uniffi-bindgen reads the FFI surface out of a built library, and wants cargo
# metadata from its own working directory.
( cd "$REPOSITORY_DIR/rust" \
  && cargo run --quiet --release --manifest-path "$REPOSITORY_DIR/rust/Cargo.toml" \
    -p roamling-core --bin uniffi-bindgen -- \
    generate --library "$DYLIB" --language swift --out-dir "$GENERATED" --no-format )

mv "$GENERATED"/roamling_coreFFI.h "$HEADERS/"
# clang looks for this name; uniffi writes one named after the crate.
mv "$GENERATED"/roamling_coreFFI.modulemap "$HEADERS/module.modulemap"
cp "$ARCHIVE" "$LIB_DIR/libroamling_core.a"
