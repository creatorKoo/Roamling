// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Ported from `Sources/RoamlingCore/CoordinateSpace.swift`.

use crate::geometry::{WorldPoint, WorldRect};

/// Converts a host screen plane with a bottom-left, y-up origin into the core
/// world plane, which is top-left and y-down. Values are logical points, never
/// backing pixels.
///
/// On Windows the host plane is already top-left, so this becomes the identity
/// -- which is why the world plane was defined this way in the first place.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DesktopCoordinateSpace {
    pub world_top: f64,
}

impl DesktopCoordinateSpace {
    pub fn new(world_top: f64) -> Self {
        Self { world_top }
    }

    pub fn from_host_frames(frames: &[WorldRect]) -> Self {
        // Swift's `map(\.maxY).max() ?? 0`: an empty desktop anchors at zero.
        let top = frames
            .iter()
            .map(|frame| frame.max_y())
            .fold(None::<f64>, |best, value| {
                Some(match best {
                    Some(current) if current >= value => current,
                    _ => value,
                })
            })
            .unwrap_or(0.0);
        Self::new(top)
    }

    /// The conversion is its own inverse, which is why both directions exist
    /// under different names rather than one being spelled backwards.
    pub fn point_from_host(&self, point: WorldPoint) -> WorldPoint {
        WorldPoint::new(point.x, self.world_top - point.y)
    }

    pub fn point_to_host(&self, point: WorldPoint) -> WorldPoint {
        WorldPoint::new(point.x, self.world_top - point.y)
    }

    pub fn rect_from_host(&self, rect: WorldRect) -> WorldRect {
        WorldRect::new(
            rect.min_x(),
            self.world_top - rect.max_y(),
            rect.size.width,
            rect.size.height,
        )
    }

    pub fn rect_to_host(&self, rect: WorldRect) -> WorldRect {
        WorldRect::new(
            rect.min_x(),
            self.world_top - rect.max_y(),
            rect.size.width,
            rect.size.height,
        )
    }

    /// Window and accessibility APIs report top-left rects on a plane anchored
    /// to the primary display, which stops matching the world plane as soon as
    /// a second display sits above it. Both adapters fold them in through here.
    pub fn rect_from_primary_anchored(&self, rect: WorldRect, primary_top: f64) -> WorldRect {
        self.rect_from_host(WorldRect::new(
            rect.min_x(),
            primary_top - rect.max_y(),
            rect.size.width,
            rect.size.height,
        ))
    }
}
