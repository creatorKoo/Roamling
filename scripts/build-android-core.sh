#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 GooBeom Jeoung
# SPDX-License-Identifier: GPL-3.0-only
set -euo pipefail

repository_dir="$(cd "$(dirname "$0")/.." && pwd)"
generated="$repository_dir/android/core/src/main/kotlin"
native="$repository_dir/android/core/src/main/jniLibs"
jobs="${CARGO_BUILD_JOBS:-2}"
command -v cargo >/dev/null || { echo 'Install Rust first.' >&2; exit 1; }
cargo ndk --version
: "${ANDROID_NDK_HOME:?Set ANDROID_NDK_HOME to an installed Android NDK directory}"
test -f "$ANDROID_NDK_HOME/source.properties"
mkdir -p "$generated" "$native"
cd "$repository_dir/rust"

cargo build --locked --release -p roamling-android -p roamling-core --jobs "$jobs"
# Cargo metadata respects a caller-specified CARGO_TARGET_DIR too.
target_dir="$(cargo metadata --locked --format-version 1 --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
case "$(uname -s)" in
    Darwin) host_library="$target_dir/release/libroamling_android.dylib" ;;
    Linux) host_library="$target_dir/release/libroamling_android.so" ;;
    *) echo 'Use build-android-core.ps1 on Windows.' >&2; exit 1 ;;
esac
"$target_dir/release/uniffi-bindgen" generate --library "$host_library" \
    --language kotlin --out-dir "$generated" --no-format
# The working directory makes Cargo read rust/.cargo/config.toml.
cargo ndk -t arm64-v8a -t x86_64 --platform 30 build \
    --locked --release -p roamling-android --lib --jobs "$jobs"
# -o would also copy the dependency's cdylib, duplicating the core in the APK.
mkdir -p "$native/arm64-v8a" "$native/x86_64"
cp "$target_dir/aarch64-linux-android/release/libroamling_android.so" "$native/arm64-v8a/"
cp "$target_dir/x86_64-linux-android/release/libroamling_android.so" "$native/x86_64/"
for abi in arm64-v8a x86_64; do
    test -f "$native/$abi/libroamling_android.so"
done
for component in roamling_core roamling_android; do
    test -f "$generated/uniffi/$component/$component.kt"
done
echo 'Android libraries and both Kotlin binding components are ready.'
