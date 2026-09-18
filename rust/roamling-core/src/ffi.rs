// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Shared boundary for the Swift and Kotlin shells.
//!
//! macOS and Android cross this boundary; the Windows shell links the crate and
//! calls the same functions directly. Kept coarse on purpose -- one call with a
//! whole world rather than a call per rectangle, which is the shape that
//! measured 0.03% of a frame in `docs/history/windows.md` section 12.

mod activity;
mod attention;
mod director;
mod focus;
mod models;
mod palette;
mod runtime;
mod tuning;
mod updater;
mod world;

pub use activity::*;
pub use attention::*;
pub use director::*;
pub use focus::*;
pub use models::*;
pub use palette::*;
pub use runtime::*;
pub use tuning::*;
pub use updater::*;
pub use world::*;
