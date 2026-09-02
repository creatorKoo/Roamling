// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Ported from `Sources/RoamlingCore/DisplayTopology.swift`.

use crate::geometry::{swift_max, swift_min, WorldPoint, WorldSize, WorldVector};
use crate::world::{first_minimum, DisplaySnapshot};

#[derive(Debug, Clone, PartialEq)]
pub struct DisplayPortal {
    pub from_display_id: String,
    pub to_display_id: String,
    pub exit: WorldPoint,
    pub entry: WorldPoint,
}

impl DisplayPortal {
    pub fn gap(&self) -> f64 {
        self.exit.distance(self.entry)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DisplayRoute {
    pub display_ids: Vec<String>,
    pub waypoints: Vec<WorldPoint>,
}

pub struct DisplayTopology {
    pub displays: Vec<DisplaySnapshot>,
}

impl DisplayTopology {
    pub fn new(displays: Vec<DisplaySnapshot>) -> Self {
        Self { displays }
    }

    pub fn route(&self, start: WorldPoint, destination: WorldPoint) -> DisplayRoute {
        if self.displays.is_empty() {
            return DisplayRoute { display_ids: Vec::new(), waypoints: vec![destination] };
        }
        let (Some(start_index), Some(end_index)) = (
            self.display_index(start),
            self.display_index(destination),
        ) else {
            return DisplayRoute { display_ids: Vec::new(), waypoints: vec![destination] };
        };
        if start_index == end_index {
            return DisplayRoute {
                display_ids: vec![self.displays[start_index].id.clone()],
                waypoints: vec![destination],
            };
        }

        let indices = self.shortest_display_path(start_index, end_index);
        let mut waypoints: Vec<WorldPoint> = Vec::new();
        for pair in indices.windows(2) {
            let portal = self.portal(&self.displays[pair[0]], &self.displays[pair[1]], None);
            append_if_distinct(portal.exit, &mut waypoints);
            append_if_distinct(portal.entry, &mut waypoints);
        }
        append_if_distinct(destination, &mut waypoints);

        DisplayRoute {
            display_ids: indices.iter().map(|i| self.displays[*i].id.clone()).collect(),
            waypoints,
        }
    }

    /// With `near`, the seam point closest to a preferred location. Wander
    /// routes use the stable seam midpoint; edge-pressure evade uses a nearby
    /// point so the creature does not run halfway around the display first.
    pub fn portal(
        &self,
        from: &DisplaySnapshot,
        to: &DisplaySnapshot,
        near: Option<WorldPoint>,
    ) -> DisplayPortal {
        let a = from.frame;
        let b = to.frame;
        let overlap_x_min = swift_max(a.min_x(), b.min_x());
        let overlap_x_max = swift_min(a.max_x(), b.max_x());
        let overlap_y_min = swift_max(a.min_y(), b.min_y());
        let overlap_y_max = swift_min(a.max_y(), b.max_y());
        let has_x_overlap = overlap_x_min <= overlap_x_max;
        let has_y_overlap = overlap_y_min <= overlap_y_max;

        let seam_y = || match near {
            Some(point) => swift_min(swift_max(point.y, overlap_y_min), overlap_y_max),
            None => (overlap_y_min + overlap_y_max) / 2.0,
        };
        let seam_x = || match near {
            Some(point) => swift_min(swift_max(point.x, overlap_x_min), overlap_x_max),
            None => (overlap_x_min + overlap_x_max) / 2.0,
        };

        let (exit, entry) = if has_y_overlap && a.max_x() <= b.min_x() {
            let y = seam_y();
            (WorldPoint::new(a.max_x(), y), WorldPoint::new(b.min_x(), y))
        } else if has_y_overlap && b.max_x() <= a.min_x() {
            let y = seam_y();
            (WorldPoint::new(a.min_x(), y), WorldPoint::new(b.max_x(), y))
        } else if has_x_overlap && a.max_y() <= b.min_y() {
            let x = seam_x();
            (WorldPoint::new(x, a.max_y()), WorldPoint::new(x, b.min_y()))
        } else if has_x_overlap && b.max_y() <= a.min_y() {
            let x = seam_x();
            (WorldPoint::new(x, a.min_y()), WorldPoint::new(x, b.max_y()))
        } else if has_x_overlap && has_y_overlap {
            let common = WorldPoint::new(seam_x(), seam_y());
            (common, common)
        } else {
            (a.closest_point(b.center()), b.closest_point(a.center()))
        };

        DisplayPortal {
            from_display_id: from.id.clone(),
            to_display_id: to.id.clone(),
            exit,
            entry,
        }
    }

    /// A quick, visible escape through a real neighbouring seam when the
    /// pointer keeps pressing the creature against an edge. Gapped displays are
    /// excluded on purpose: ordinary wander routing is the only place an
    /// invisible gap may be crossed.
    pub fn evade_transition(
        &self,
        start: WorldPoint,
        direction: WorldVector,
        object_size: WorldSize,
        maximum_portal_distance: f64,
        maximum_gap: f64,
    ) -> Option<DisplayRoute> {
        if !(direction.length() > 0.001) {
            return None;
        }
        let source_index = self.display_index(start)?;
        let source = &self.displays[source_index];
        let safe_source = source
            .visible_frame
            .inset_by(object_size.width / 2.0, object_size.height / 2.0);
        let reach = swift_max(object_size.width, object_size.height) / 2.0 + 2.0;
        let pressure_probe = start.offset(direction.normalized().scaled(reach));
        if safe_source.contains(pressure_probe) {
            return None;
        }

        let normalized_direction = direction.normalized();
        let candidates: Vec<(usize, DisplayPortal, f64)> = self
            .displays
            .iter()
            .enumerate()
            .filter_map(|(index, target)| {
                if index == source_index {
                    return None;
                }
                let portal = self.portal(source, target, Some(start));
                if !(portal.gap() <= maximum_gap) {
                    return None;
                }
                let toward = target.frame.center().vector_from(source.frame.center()).normalized();
                let alignment = normalized_direction.dot(toward);
                if !(alignment >= 0.18) {
                    return None;
                }
                let portal_distance = start.distance(portal.exit);
                if !(portal_distance <= maximum_portal_distance) {
                    return None;
                }
                let score = portal_distance + (1.0 - alignment) * maximum_portal_distance;
                Some((index, portal, score))
            })
            .collect();

        let best = first_minimum(&candidates, |candidate| candidate.2)?;
        let target = &self.displays[best.0];
        let target_safe = target
            .visible_frame
            .inset_by(object_size.width / 2.0, object_size.height / 2.0);
        let inward = target.visible_frame.center().vector_from(best.1.entry).normalized();
        let raw_target = best.1.entry.offset(
            inward.scaled(swift_max(object_size.width, object_size.height) / 2.0 + 14.0),
        );
        let entry_target = target_safe.closest_point(raw_target);
        let source_approach = safe_source.closest_point(best.1.exit);

        let mut waypoints = Vec::new();
        append_if_distinct(source_approach, &mut waypoints);
        append_if_distinct(best.1.exit, &mut waypoints);
        append_if_distinct(best.1.entry, &mut waypoints);
        append_if_distinct(entry_target, &mut waypoints);
        Some(DisplayRoute {
            display_ids: vec![source.id.clone(), target.id.clone()],
            waypoints,
        })
    }

    fn display_index(&self, point: WorldPoint) -> Option<usize> {
        if let Some(index) = self.displays.iter().position(|d| d.frame.contains(point)) {
            return Some(index);
        }
        let indices: Vec<usize> = (0..self.displays.len()).collect();
        first_minimum(&indices, |index| self.displays[*index].frame.distance(point)).copied()
    }

    /// Complete graph + Dijkstra. Touching displays cost a token 1, so a chain
    /// of real seams beats a direct jump across an intervening gap.
    ///
    /// Ties go to the lower display index. The Swift original drew from a
    /// `Set`, whose order Swift randomizes per process, so a symmetric desk
    /// sent the pet left or right by coin flip; that was fixed before this
    /// port, and this reproduces the fix rather than the old behaviour.
    fn shortest_display_path(&self, source: usize, target: usize) -> Vec<usize> {
        let count = self.displays.len();
        let mut distance = vec![f64::INFINITY; count];
        let mut previous: Vec<Option<usize>> = vec![None; count];
        let mut visited = vec![false; count];
        distance[source] = 0.0;

        loop {
            let mut current: Option<usize> = None;
            for index in 0..count {
                if visited[index] {
                    continue;
                }
                match current {
                    Some(best) if distance[index] >= distance[best] => {}
                    _ => current = Some(index),
                }
            }
            let Some(current) = current else { break };
            visited[current] = true;
            if current == target {
                break;
            }
            if !distance[current].is_finite() {
                break;
            }
            for neighbor in 0..count {
                if visited[neighbor] || neighbor == current {
                    continue;
                }
                let crossing = self.portal(&self.displays[current], &self.displays[neighbor], None);
                let weight = swift_max(1.0, crossing.gap());
                let candidate = distance[current] + weight;
                if candidate < distance[neighbor] {
                    distance[neighbor] = candidate;
                    previous[neighbor] = Some(current);
                }
            }
        }

        let mut result = vec![target];
        let mut cursor = target;
        while cursor != source {
            let Some(parent) = previous[cursor] else { break };
            result.push(parent);
            cursor = parent;
        }
        if result.last() != Some(&source) {
            return vec![source, target];
        }
        result.reverse();
        result
    }
}

/// Half a point apart counts as the same place: consecutive waypoints that
/// close would make the mover spin rather than walk.
fn append_if_distinct(point: WorldPoint, points: &mut Vec<WorldPoint>) {
    if let Some(last) = points.last() {
        if last.distance(point) < 0.5 {
            return;
        }
    }
    points.push(point);
}
