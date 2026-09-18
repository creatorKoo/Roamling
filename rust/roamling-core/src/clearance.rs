// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Shared runtime ranking: known content is excluded before preferences apply.
//! Distances use the existing coarse luminance grid, never OCR or screenshots.
use crate::emptiness::{LuminanceField, VisualEmptiness};
use crate::geometry::{WorldPoint, WorldRect, WorldSize};

pub(crate) const CLEARANCE_STEP: f64 = 24.0;
const EMPTY: f64 = 0.55;

pub(crate) struct ClearanceMap<'a> {
    field: &'a LuminanceField,
    busy: Vec<WorldRect>,
}

impl<'a> ClearanceMap<'a> {
    pub(crate) fn new(field: &'a LuminanceField) -> Self {
        let cell = field.cell_size();
        let mut busy = Vec::new();
        for row in 0..field.rows {
            for col in 0..field.columns {
                let x = field.bounds.min_x() + col as f64 * cell.width;
                let y = field.bounds.min_y() + row as f64 * cell.height;
                let neighbourhood = WorldRect::new(
                    x - cell.width,
                    y - cell.height,
                    cell.width * 3.0,
                    cell.height * 3.0,
                );
                if VisualEmptiness::score(neighbourhood, field).is_some_and(|score| score < EMPTY) {
                    busy.push(WorldRect::new(x, y, cell.width, cell.height));
                }
            }
        }
        Self { field, busy }
    }

    /// None is unobserved; negative means content; otherwise distance from the
    /// pet's body to the nearest busy cell, without the old two-body-size cap.
    pub(crate) fn distance(&self, point: WorldPoint, size: WorldSize) -> Option<f64> {
        let rect = WorldRect::new(
            point.x - size.width / 2.0,
            point.y - size.height / 2.0,
            size.width,
            size.height,
        );
        if rect.min_x() < self.field.bounds.min_x()
            || rect.max_x() > self.field.bounds.max_x()
            || rect.min_y() < self.field.bounds.min_y()
            || rect.max_y() > self.field.bounds.max_y()
        {
            return None;
        }
        let score = VisualEmptiness::score(rect, self.field)?;
        if score < EMPTY {
            return Some(-1.0);
        }
        let mut distance = self
            .field
            .bounds
            .size
            .width
            .hypot(self.field.bounds.size.height);
        for busy in &self.busy {
            let dx = (busy.min_x() - rect.max_x())
                .max(rect.min_x() - busy.max_x())
                .max(0.0);
            let dy = (busy.min_y() - rect.max_y())
                .max(rect.min_y() - busy.max_y())
                .max(0.0);
            if dx == 0.0 && dy == 0.0 {
                return Some(-1.0);
            }
            distance = distance.min(dx.hypot(dy));
        }
        Some(distance)
    }

    /// Return the best clearance band. Existing preferences break ties only
    /// within this band; unobserved points are a fallback, never an advantage.
    pub(crate) fn best(&self, points: &[WorldPoint], size: WorldSize) -> Vec<usize> {
        let mut best = Vec::new();
        let mut unknown = Vec::new();
        let mut band = -1.0;
        let mut judged = false;
        for (index, point) in points.iter().enumerate() {
            match self.distance(*point, size) {
                None => unknown.push(index),
                Some(distance) if distance >= 0.0 => {
                    judged = true;
                    let candidate = (distance / CLEARANCE_STEP).floor();
                    if candidate > band {
                        best.clear();
                        band = candidate;
                    }
                    if candidate == band {
                        best.push(index);
                    }
                }
                _ => judged = true,
            }
        }
        if best.is_empty() && !judged {
            unknown
        } else {
            best
        }
    }

    pub(crate) fn improves(&self, from: WorldPoint, to: WorldPoint, size: WorldSize) -> bool {
        match (self.distance(from, size), self.distance(to, size)) {
            (Some(old), Some(new)) if new >= 0.0 => old < 0.0 || new >= old + CLEARANCE_STEP,
            (None, Some(new)) => new >= 0.0,
            _ => false,
        }
    }
}
