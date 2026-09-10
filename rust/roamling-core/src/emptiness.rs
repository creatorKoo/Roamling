// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Ported from `Sources/RoamlingCore/VisualEmptiness.swift` and
//! `CandidateScoring.swift`.

use crate::geometry::{clamped, swift_min, WorldPoint, WorldRect, WorldSize};
use crate::world::last_maximum;

/// A downsampled grayscale view of part of the desktop.
///
/// Captured pixels never leave the capture adapter as an image. They arrive as
/// a small grid covering a known world rect, get scored, and are dropped. This
/// type cannot express what was on screen, only how busy each region looked.
#[derive(Debug, Clone, PartialEq)]
pub struct LuminanceField {
    pub bounds: WorldRect,
    pub columns: usize,
    pub rows: usize,
    /// Row-major, top-left first, each clamped to `0...1`.
    pub samples: Vec<f64>,
}

impl LuminanceField {
    pub fn new(
        bounds: WorldRect,
        columns: usize,
        rows: usize,
        samples: Vec<f64>,
    ) -> Option<Self> {
        if columns == 0 || rows == 0 || bounds.is_empty() || samples.len() != columns * rows {
            return None;
        }
        Some(Self {
            bounds,
            columns,
            rows,
            samples: samples.into_iter().map(|value| clamped(value, 0.0, 1.0)).collect(),
        })
    }

    pub fn sample(&self, column: i64, row: i64) -> Option<f64> {
        if column < 0 || row < 0 || column as usize >= self.columns || row as usize >= self.rows {
            return None;
        }
        Some(self.samples[row as usize * self.columns + column as usize])
    }

    pub fn cell_size(&self) -> WorldSize {
        WorldSize::new(
            self.bounds.size.width / self.columns as f64,
            self.bounds.size.height / self.rows as f64,
        )
    }
}

/// Scores how visually empty a candidate region looks.
///
/// Text, code and dense controls all raise the local gradient; photographs and
/// busy imagery raise the spread. Weighting the gradient higher keeps a smooth
/// wallpaper gradient -- fine to sit on -- from reading as busy.
pub struct VisualEmptiness;

impl VisualEmptiness {
    /// A mean neighbour difference at or above this reads as fully busy.
    ///
    /// Downsampling averages a couple of glyphs into one sample, so a page of
    /// text arrives far flatter than it looks: measured against rendered
    /// terminal output it lands near 0.025. The first calibration used 0.10 and
    /// scored solid body text at 0.79 -- indistinguishable from wallpaper --
    /// which let the pet park on the user's work.
    const GRADIENT_REFERENCE: f64 = 0.02;
    const SPREAD_REFERENCE: f64 = 0.05;
    const GRADIENT_WEIGHT: f64 = 0.7;

    /// `0...1`, where 1 is flat and safe to sit on. None when the region does
    /// not overlap enough of the field to judge, so callers can fall back
    /// rather than trust a guess made from two samples.
    pub fn score(rect: WorldRect, field: &LuminanceField) -> Option<f64> {
        let cell = field.cell_size();
        if !(cell.width > 0.0) || !(cell.height > 0.0) {
            return None;
        }

        // Swift's `Int(_:)` traps out of range where Rust's `as` saturates.
        // Every caller feeds screen-sized values, so the difference is only
        // reachable by a bug -- and saturating is the kinder failure.
        let first_column = 0i64.max(((rect.min_x() - field.bounds.min_x()) / cell.width).floor() as i64);
        let last_column = (field.columns as i64 - 1)
            .min(((rect.max_x() - field.bounds.min_x()) / cell.width).ceil() as i64 - 1);
        let first_row = 0i64.max(((rect.min_y() - field.bounds.min_y()) / cell.height).floor() as i64);
        let last_row = (field.rows as i64 - 1)
            .min(((rect.max_y() - field.bounds.min_y()) / cell.height).ceil() as i64 - 1);
        if last_column - first_column < 1 || last_row - first_row < 1 {
            return None;
        }

        let mut values: Vec<f64> = Vec::new();
        let mut gradient_total = 0.0;
        let mut gradient_count = 0usize;
        for row in first_row..=last_row {
            for column in first_column..=last_column {
                let Some(value) = field.sample(column, row) else { continue };
                values.push(value);
                if column < last_column {
                    if let Some(right) = field.sample(column + 1, row) {
                        gradient_total += (right - value).abs();
                        gradient_count += 1;
                    }
                }
                if row < last_row {
                    if let Some(below) = field.sample(column, row + 1) {
                        gradient_total += (below - value).abs();
                        gradient_count += 1;
                    }
                }
            }
        }
        if values.len() < 4 || gradient_count == 0 {
            return None;
        }

        let mean_gradient = gradient_total / gradient_count as f64;
        // Summed in the order Swift's `reduce` walks them, because floating
        // point addition is not associative and this has to match bit for bit.
        let mean = values.iter().fold(0.0, |sum, value| sum + value) / values.len() as f64;
        let variance = values
            .iter()
            .fold(0.0, |sum, value| sum + (value - mean) * (value - mean))
            / values.len() as f64;
        let spread = variance.sqrt();

        let gradient_term = swift_min(1.0, mean_gradient / Self::GRADIENT_REFERENCE);
        let spread_term = swift_min(1.0, spread / Self::SPREAD_REFERENCE);
        let busyness =
            gradient_term * Self::GRADIENT_WEIGHT + spread_term * (1.0 - Self::GRADIENT_WEIGHT);
        Some(clamped(1.0 - busyness, 0.0, 1.0))
    }

    /// Picks a spot from `points` that is not sitting on content.
    ///
    /// Roaming is supposed to look aimless, so this keeps the order it was
    /// given rather than always taking the emptiest point: the first candidate
    /// clear enough wins, and only when none clear the bar does the least bad
    /// one. None when the field cannot judge any of them, which leaves the
    /// caller's own pick alone.
    pub fn first_comfortable(
        points: &[WorldPoint],
        object_size: WorldSize,
        field: &LuminanceField,
        threshold: f64,
    ) -> Option<WorldPoint> {
        let mut best: Option<(WorldPoint, f64)> = None;
        for point in points {
            let frame = WorldRect::new(
                point.x - object_size.width / 2.0,
                point.y - object_size.height / 2.0,
                object_size.width,
                object_size.height,
            );
            let Some(score) = Self::score(frame, field) else { continue };
            if score >= threshold {
                return Some(*point);
            }
            if score > best.map(|(_, value)| value).unwrap_or(-1.0) {
                best = Some((*point, score));
            }
        }
        best.map(|(point, _)| point)
    }

    /// The frames a spot has to clear, each as a multiple of the pet's own,
    /// before it counts as having room around it. Passing the first means the
    /// pet is not on content; passing the last means there is a pet's width of
    /// nothing on every side. A spot beside a paragraph passes the first only,
    /// and until this existed that was indistinguishable from the middle of
    /// the desktop.
    pub const CLEARANCE_SCALES: [f64; 3] = [1.0, 1.5, 2.0];

    /// How many of `CLEARANCE_SCALES` the spot clears, starting from the
    /// smallest. Zero means it is on content; None means the field cannot
    /// judge even the pet's own frame there.
    pub fn clearance(
        point: WorldPoint,
        object_size: WorldSize,
        field: &LuminanceField,
        threshold: f64,
    ) -> Option<usize> {
        Self::clearance_and_score(point, object_size, field, threshold).map(|(tier, _)| tier)
    }

    /// The tier plus the score at the pet's own frame, so a caller ranking the
    /// spots that failed does not pay for the same score twice.
    fn clearance_and_score(
        point: WorldPoint,
        object_size: WorldSize,
        field: &LuminanceField,
        threshold: f64,
    ) -> Option<(usize, f64)> {
        let mut passed = 0usize;
        let mut base = 0.0;
        for (index, scale) in Self::CLEARANCE_SCALES.iter().enumerate() {
            let frame = WorldRect::new(
                point.x - object_size.width * scale / 2.0,
                point.y - object_size.height * scale / 2.0,
                object_size.width * scale,
                object_size.height * scale,
            );
            let Some(score) = Self::score(frame, field) else {
                if index == 0 {
                    return None;
                }
                break;
            };
            if index == 0 {
                base = score;
            }
            if !(score >= threshold) {
                break;
            }
            passed += 1;
        }
        Some((passed, base))
    }

    /// Picks the spot from `points` with the most room around it, without
    /// making roaming look calculated.
    ///
    /// The first candidate that clears every scale wins outright -- any spot
    /// with a pet's width of nothing on every side is as good as any other, and
    /// taking the first keeps the walk aimless. Only when none does is the
    /// best tier taken, and only when nothing passes at all does the least bad
    /// one come back, marked as such so the caller can decline the walk.
    pub fn most_comfortable(
        points: &[WorldPoint],
        object_size: WorldSize,
        field: &LuminanceField,
        threshold: f64,
    ) -> ComfortPick {
        let mut best_clear: Option<(WorldPoint, usize)> = None;
        let mut least_bad: Option<(WorldPoint, f64)> = None;
        for point in points {
            let Some((tier, base)) =
                Self::clearance_and_score(*point, object_size, field, threshold)
            else {
                continue;
            };
            if tier >= Self::CLEARANCE_SCALES.len() {
                return ComfortPick::Clear(*point);
            }
            if tier >= 1 {
                if tier > best_clear.map(|(_, value)| value).unwrap_or(0) {
                    best_clear = Some((*point, tier));
                }
            } else if base > least_bad.map(|(_, value)| value).unwrap_or(-1.0) {
                least_bad = Some((*point, base));
            }
        }
        if let Some((point, _)) = best_clear {
            return ComfortPick::Clear(point);
        }
        if let Some((point, _)) = least_bad {
            return ComfortPick::Marginal(point);
        }
        ComfortPick::Unjudged
    }
}

/// What `VisualEmptiness::most_comfortable` found.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ComfortPick {
    /// Off content, with the most room around it of the spots offered.
    Clear(WorldPoint),
    /// Every judgeable spot is on content; this one least so.
    Marginal(WorldPoint),
    /// The field could not judge any of them.
    Unjudged,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PositionCandidate {
    pub point: WorldPoint,
    pub visual_empty_score: f64,
    pub distance_from_caret: f64,
    pub distance_from_controls: f64,
    pub edge_preference: f64,
    pub stability_score: f64,
    pub context_preference: f64,
    pub pet_comfort: f64,
    pub pointer_proximity: f64,
    pub obstruction_penalty: f64,
}

impl PositionCandidate {
    pub fn at(point: WorldPoint) -> Self {
        Self { point, ..Default::default() }
    }

    pub fn score(&self) -> f64 {
        self.visual_empty_score
            + self.distance_from_caret
            + self.distance_from_controls
            + self.edge_preference
            + self.stability_score
            + self.context_preference
            + self.pet_comfort
            - self.pointer_proximity
            - self.obstruction_penalty
    }
}

pub struct CandidatePositionScorer;

impl CandidatePositionScorer {
    /// Swift's `max(by:)` keeps the last of equal elements, and the comparator
    /// falls through score, then stability, then x -- so a three-way tie still
    /// lands somewhere fixed rather than wherever the array happened to order.
    pub fn best(candidates: &[PositionCandidate]) -> Option<&PositionCandidate> {
        last_maximum(candidates, |lhs, rhs| {
            if lhs.score() == rhs.score() {
                if lhs.stability_score == rhs.stability_score {
                    lhs.point.x > rhs.point.x
                } else {
                    lhs.stability_score < rhs.stability_score
                }
            } else {
                lhs.score() < rhs.score()
            }
        })
    }
}
