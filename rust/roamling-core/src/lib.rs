// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Roamling's platform-independent core, being ported from Swift one unit at a
//! time. See `docs/windows.md`, "조각내서 갈아탄다", for the order and for why
//! the Swift original stays beside each unit until their outputs agree.

uniffi::setup_scaffolding!();

pub mod coordinate_space;
pub mod emptiness;
pub mod ffi;
pub mod interest;
pub mod geometry;
pub mod safe_zone;
pub mod topology;
pub mod world;

pub use coordinate_space::DesktopCoordinateSpace;
pub use geometry::{clamped, swift_max, swift_min, WorldPoint, WorldRect, WorldSize, WorldVector};
pub use emptiness::{
    CandidatePositionScorer, LuminanceField, PositionCandidate, VisualEmptiness,
};
pub use interest::{BasicInterestPositionPlanner, InterestDestination, SeatEvaluation};
pub use safe_zone::{BasicSafeZonePlanner, RestDestination};
pub use topology::{DisplayPortal, DisplayRoute, DisplayTopology};
pub use world::{DesktopWorldSnapshot, DisplaySnapshot, FocusSnapshot, LocationHint, SafeZone};
