// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Runtime placement policy. Physical display frames still define transit;
//! only the usable area offered to destination planners excludes shared edges.

use crate::geometry::{WorldPoint, WorldRect, WorldSize};
use crate::topology::DisplayTopology;
use crate::world::{DesktopWorldSnapshot, DisplaySnapshot};

const SEAM_MARGIN: f64 = 120.0;

pub(crate) fn placement_frame(
    display: &DisplaySnapshot,
    displays: &[DisplaySnapshot],
) -> WorldRect {
    let a = display.frame;
    let v = display.visible_frame;
    let (mut left, mut right, mut top, mut bottom) = (v.min_x(), v.max_x(), v.min_y(), v.max_y());
    // Leave room even on narrow displays, including a display with neighbours
    // on both sides. Corner-only contacts and mirrored displays are not seams.
    let dx = SEAM_MARGIN.min(v.size.width / 4.0);
    let dy = SEAM_MARGIN.min(v.size.height / 4.0);
    for other in displays.iter().filter(|other| other.id != display.id) {
        let b = other.frame;
        if a.min_y().max(b.min_y()) < a.max_y().min(b.max_y()) {
            if (a.min_x() - b.max_x()).abs() <= 1.0 {
                left = left.max(a.min_x() + dx);
            }
            if (a.max_x() - b.min_x()).abs() <= 1.0 {
                right = right.min(a.max_x() - dx);
            }
        }
        if a.min_x().max(b.min_x()) < a.max_x().min(b.max_x()) {
            if (a.min_y() - b.max_y()).abs() <= 1.0 {
                top = top.max(a.min_y() + dy);
            }
            if (a.max_y() - b.min_y()).abs() <= 1.0 {
                bottom = bottom.min(a.max_y() - dy);
            }
        }
    }
    WorldRect::new(left, top, (right - left).max(0.0), (bottom - top).max(0.0))
}

pub(crate) fn placement_world(world: &DesktopWorldSnapshot) -> DesktopWorldSnapshot {
    let mut result = world.clone();
    for display in &mut result.displays {
        display.visible_frame = placement_frame(display, &world.displays);
    }
    result
}

/// Commit only near the next real shared edge, not for a whole screen-long
/// trip. The inserted waypoint takes the entire pet clear before replanning.
pub(crate) fn crossing(
    displays: &[DisplaySnapshot],
    position: WorldPoint,
    remaining: &[WorldPoint],
    size: WorldSize,
) -> Option<(WorldPoint, Vec<WorldPoint>)> {
    let next = *remaining.first()?;
    let source = displays
        .iter()
        .find(|display| display.frame.contains(position))?;
    let topology = DisplayTopology::new(displays.to_vec());
    for target in displays.iter().filter(|display| display.id != source.id) {
        let a = source.frame;
        let b = target.frame;
        let shares_edge = (a.min_y().max(b.min_y()) < a.max_y().min(b.max_y())
            && ((a.min_x() - b.max_x()).abs() <= 1.0 || (a.max_x() - b.min_x()).abs() <= 1.0))
            || (a.min_x().max(b.min_x()) < a.max_x().min(b.max_x())
                && ((a.min_y() - b.max_y()).abs() <= 1.0 || (a.max_y() - b.min_y()).abs() <= 1.0));
        if !shares_edge {
            continue;
        }
        let portal = topology.portal(source, target, Some(next));
        if portal.gap() > 1.0
            || next.distance(portal.exit) > 0.5
            || position.distance(portal.exit) > SEAM_MARGIN + size.width.max(size.height) / 2.0
        {
            continue;
        }
        let clear = placement_frame(target, displays).clamped_center(portal.entry, size);
        let mut route = vec![portal.exit, portal.entry, clear];
        route.extend(remaining.iter().copied().skip_while(|point| {
            point.distance(portal.exit) <= 0.5 || point.distance(portal.entry) <= 0.5
        }));
        return Some((clear, route));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn display(id: &str, frame: WorldRect) -> DisplaySnapshot {
        DisplaySnapshot {
            id: id.into(),
            name: id.into(),
            visible_frame: frame,
            frame,
            scale: 1.0,
        }
    }

    #[test]
    fn only_shared_edges_reserve_space_in_offset_and_small_layouts() {
        let a = display("a", WorldRect::new(-1000.0, 0.0, 1000.0, 800.0));
        let b = display("b", WorldRect::new(0.0, 200.0, 1000.0, 800.0));
        assert_eq!(
            placement_frame(&a, &[a.clone(), b]),
            WorldRect::new(-1000.0, 0.0, 880.0, 800.0)
        );
        for frame in [
            WorldRect::new(0.0, 800.0, 1000.0, 800.0),
            WorldRect::new(20.0, 0.0, 1000.0, 800.0),
            a.frame,
        ] {
            let other = display("other", frame);
            assert_eq!(placement_frame(&a, &[a.clone(), other]), a.visible_frame);
        }
        let small = display("small", WorldRect::new(0.0, 0.0, 200.0, 150.0));
        let right = display("right", WorldRect::new(200.0, 0.0, 200.0, 150.0));
        let usable = placement_frame(&small, &[a, small.clone(), right]);
        assert_eq!(usable.size.width, 100.0);
        assert!(!usable.inset_by(48.0, 52.0).is_empty());
    }
}
