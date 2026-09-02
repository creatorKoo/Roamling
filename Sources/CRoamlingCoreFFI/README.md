Generated. `scripts/build-rust-core.sh` writes the uniffi C header and its
module map here.

Separate from `Sources/RoamlingCoreRs` because SwiftPM only propagates a C
module to other targets when it is declared as a `systemLibrary`, and one of
those is a directory of its own.
