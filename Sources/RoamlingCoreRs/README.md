Generated. Do not edit, and do not commit anything else here.

`scripts/build-rust-core.sh` writes `roamling_core.swift` and `include/` from
`rust/roamling-core` with uniffi, and drops the static archive in `.build/rust/`.
Run it after changing anything under `rust/`.

This directory exists because SwiftPM compiles what it finds under `Sources/`,
and the bindings have to land somewhere it looks. Everything in it is a build
product, which is why `.gitignore` keeps all of it but this file.
