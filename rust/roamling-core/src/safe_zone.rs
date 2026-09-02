// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Ported from `Sources/RoamlingCore/BasicSafeZone.swift`.
//!
//! `naps_in_place` stays in Swift for now: it reads a luminance field, which
//! arrives with unit 3.

use crate::geometry::{swift_max, swift_min, WorldPoint, WorldRect, WorldSize};
use crate::world::{last_maximum, DesktopWorldSnapshot, DisplaySnapshot, SafeZone};

#[derive(Debug, Clone, PartialEq)]
pub struct RestDestination {
    pub point: WorldPoint,
    pub display_id: String,
    pub reason: String,
    pub score: f64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DockEdge {
    Left,
    Right,
    Bottom,
}

/// Permission-free sleep placement from platform-neutral display snapshots.
/// `visible_frame` already excludes whatever the platform reserves at the edges
/// -- the menu bar and Dock on macOS, the taskbar on Windows.
pub struct BasicSafeZonePlanner;

impl BasicSafeZonePlanner {
    pub fn safe_zones(world: &DesktopWorldSnapshot) -> Vec<SafeZone> {
        world.displays.iter().flat_map(Self::safe_zones_on).collect()
    }

    pub fn destination(
        world: &DesktopWorldSnapshot,
        current_position: WorldPoint,
        pointer_position: Option<WorldPoint>,
        object_size: WorldSize,
    ) -> Option<RestDestination> {
        let owned;
        let zones: &[SafeZone] = if world.safe_zones.is_empty() {
            owned = Self::safe_zones(world);
            &owned
        } else {
            &world.safe_zones
        };

        let current_display = world
            .display_containing(current_position)
            .or_else(|| world.nearest_display(current_position));
        let current_display_id = current_display.map(|display| display.id.clone());

        let candidates: Vec<RestDestination> = zones
            .iter()
            .filter_map(|zone| {
                let center = zone.frame.center();
                let display = world
                    .display_containing(center)
                    .or_else(|| world.nearest_display(center))?;
                let point = display.visible_frame.clamped_center(center, object_size);
                let same_display_bonus =
                    if Some(&display.id) == current_display_id.as_ref() { 38.0 } else { 0.0 };
                let travel_penalty = swift_min(16.0, current_position.distance(point) / 180.0);
                let pointer_penalty = match pointer_position {
                    Some(pointer) => swift_max(0.0, 260.0 - pointer.distance(point)) / 11.0,
                    None => 0.0,
                };
                Some(RestDestination {
                    point,
                    display_id: display.id.clone(),
                    reason: zone.reason.clone(),
                    score: zone.score + same_display_bonus - travel_penalty - pointer_penalty,
                })
            })
            .collect();

        // Swift's `max(by:)` keeps the last of equal elements, and the
        // comparator falls back to distance when scores tie.
        let best = last_maximum(&candidates, |lhs, rhs| {
            if lhs.score == rhs.score {
                current_position.distance(lhs.point) > current_position.distance(rhs.point)
            } else {
                lhs.score < rhs.score
            }
        });
        if let Some(best) = best {
            return Some(best.clone());
        }

        let display = current_display?;
        let safe = display
            .visible_frame
            .inset_by(object_size.width / 2.0 + 18.0, object_size.height / 2.0 + 14.0);
        Some(RestDestination {
            point: WorldPoint::new(safe.max_x(), safe.max_y()),
            display_id: display.id.clone(),
            reason: "display-corner-fallback".to_string(),
            score: 0.0,
        })
    }

    fn safe_zones_on(display: &DisplaySnapshot) -> Vec<SafeZone> {
        let visible = display.visible_frame;
        if visible.is_empty() {
            return Vec::new();
        }
        let bounds = visible.inset_by(
            swift_min(18.0, visible.size.width / 4.0),
            swift_min(16.0, visible.size.height / 4.0),
        );
        let width = swift_min(220.0, bounds.size.width);
        let height = swift_min(160.0, bounds.size.height);
        if !(width > 0.0) || !(height > 0.0) {
            return Vec::new();
        }

        let left_inset = swift_max(0.0, visible.min_x() - display.frame.min_x());
        let right_inset = swift_max(0.0, display.frame.max_x() - visible.max_x());
        let bottom_inset = swift_max(0.0, display.frame.max_y() - visible.max_y());
        let side_insets = [
            (DockEdge::Left, left_inset),
            (DockEdge::Right, right_inset),
            (DockEdge::Bottom, bottom_inset),
        ];
        // Swift's `max(by:)` again: on a tie the later edge wins, so an equal
        // inset on left and bottom is read as the Dock being at the bottom.
        let largest = last_maximum(&side_insets, |lhs, rhs| lhs.1 < rhs.1);
        let dock_edge = match largest {
            Some(&(edge, inset)) if inset > 4.0 => Some(edge),
            _ => None,
        };

        let corners: [(&str, DockEdge, WorldRect); 4] = [
            (
                "top-left",
                DockEdge::Left,
                WorldRect::new(bounds.min_x(), bounds.min_y(), width, height),
            ),
            (
                "top-right",
                DockEdge::Right,
                WorldRect::new(bounds.max_x() - width, bounds.min_y(), width, height),
            ),
            (
                "bottom-left",
                if dock_edge == Some(DockEdge::Bottom) { DockEdge::Bottom } else { DockEdge::Left },
                WorldRect::new(bounds.min_x(), bounds.max_y() - height, width, height),
            ),
            (
                "bottom-right",
                if dock_edge == Some(DockEdge::Bottom) { DockEdge::Bottom } else { DockEdge::Right },
                WorldRect::new(bounds.max_x() - width, bounds.max_y() - height, width, height),
            ),
        ];

        corners
            .into_iter()
            .map(|(name, adjacent_edge, frame)| {
                let is_dock_adjacent = dock_edge.is_some() && Some(adjacent_edge) == dock_edge;
                SafeZone::new(
                    frame,
                    (if name.starts_with("bottom") { 38.0 } else { 34.0 })
                        + (if is_dock_adjacent { 4.0 } else { 0.0 }),
                    0.72,
                    if is_dock_adjacent {
                        format!("dock-adjacent-{name}")
                    } else {
                        format!("display-corner-{name}")
                    },
                )
            })
            .collect()
    }
}
