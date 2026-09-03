// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! What the Swift shell calls while the port is under way.
//!
//! Only macOS crosses this boundary; the Windows shell links the crate and
//! calls the same functions directly. Kept coarse on purpose -- one call with a
//! whole world rather than a call per rectangle, which is the shape that
//! measured 0.03% of a frame in `docs/windows.md` section 12.

use crate::emptiness::LuminanceField;
use crate::geometry::{WorldPoint, WorldRect, WorldSize};
use crate::safe_zone::BasicSafeZonePlanner;
use crate::world::{DesktopWorldSnapshot, DisplaySnapshot, SafeZone};

#[derive(uniffi::Record)]
pub struct FfiRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl From<&FfiRect> for WorldRect {
    fn from(value: &FfiRect) -> Self {
        WorldRect::new(value.x, value.y, value.width, value.height)
    }
}

impl From<WorldRect> for FfiRect {
    fn from(value: WorldRect) -> Self {
        FfiRect {
            x: value.min_x(),
            y: value.min_y(),
            width: value.size.width,
            height: value.size.height,
        }
    }
}

#[derive(uniffi::Record)]
pub struct FfiDisplay {
    pub id: String,
    pub frame: FfiRect,
    pub visible_frame: FfiRect,
}

#[derive(uniffi::Record)]
pub struct FfiSafeZone {
    pub frame: FfiRect,
    pub score: f64,
    pub confidence: f64,
    pub reason: String,
}

impl From<SafeZone> for FfiSafeZone {
    fn from(value: SafeZone) -> Self {
        FfiSafeZone {
            frame: value.frame.into(),
            score: value.score,
            confidence: value.confidence,
            reason: value.reason,
        }
    }
}

fn snapshot(displays: &[FfiDisplay]) -> DesktopWorldSnapshot {
    DesktopWorldSnapshot::new(
        displays
            .iter()
            .map(|display| DisplaySnapshot {
                id: display.id.clone(),
                name: String::new(),
                frame: (&display.frame).into(),
                visible_frame: (&display.visible_frame).into(),
                scale: 1.0,
            })
            .collect(),
        Vec::new(),
    )
}

#[uniffi::export]
pub fn safe_zones(displays: Vec<FfiDisplay>) -> Vec<FfiSafeZone> {
    BasicSafeZonePlanner::safe_zones(&snapshot(&displays))
        .into_iter()
        .map(FfiSafeZone::from)
        .collect()
}

#[derive(uniffi::Record)]
pub struct FfiRestDestination {
    pub x: f64,
    pub y: f64,
    pub display_id: String,
    pub reason: String,
    pub score: f64,
}

#[uniffi::export]
pub fn rest_destination(
    displays: Vec<FfiDisplay>,
    zones: Vec<FfiSafeZone>,
    current_x: f64,
    current_y: f64,
    pointer: Option<Vec<f64>>,
    object_width: f64,
    object_height: f64,
) -> Option<FfiRestDestination> {
    let mut world = snapshot(&displays);
    world.safe_zones = zones
        .into_iter()
        .map(|zone| SafeZone::new((&zone.frame).into(), zone.score, zone.confidence, zone.reason))
        .collect();
    let pointer = pointer.and_then(|values| {
        (values.len() == 2).then(|| WorldPoint::new(values[0], values[1]))
    });
    BasicSafeZonePlanner::destination(
        &world,
        WorldPoint::new(current_x, current_y),
        pointer,
        WorldSize::new(object_width, object_height),
    )
    .map(|destination| FfiRestDestination {
        x: destination.point.x,
        y: destination.point.y,
        display_id: destination.display_id,
        reason: destination.reason,
        score: destination.score,
    })
}

/// The luminance grid, flattened. It is already the small downsampled thing --
/// 64 columns at most -- so passing it whole costs less than holding a handle
/// that has to be invalidated every time the capture refreshes.
#[derive(uniffi::Record)]
pub struct FfiLuminanceField {
    pub bounds: FfiRect,
    pub columns: u32,
    pub rows: u32,
    pub samples: Vec<f64>,
}

#[uniffi::export]
pub fn naps_in_place(
    x: f64,
    y: f64,
    object_width: f64,
    object_height: f64,
    field: Option<FfiLuminanceField>,
    threshold: f64,
) -> bool {
    let field = field.and_then(|value| {
        LuminanceField::new(
            (&value.bounds).into(),
            value.columns as usize,
            value.rows as usize,
            value.samples,
        )
    });
    BasicSafeZonePlanner::naps_in_place(
        WorldPoint::new(x, y),
        WorldSize::new(object_width, object_height),
        field.as_ref(),
        threshold,
    )
}
