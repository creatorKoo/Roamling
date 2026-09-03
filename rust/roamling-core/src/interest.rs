// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Ported from `Sources/RoamlingCore/InterestPlacement.swift`.

use crate::emptiness::{LuminanceField, VisualEmptiness};
use crate::geometry::{swift_max, swift_min, WorldPoint, WorldRect, WorldSize};
use crate::world::{last_maximum, DesktopWorldSnapshot, DisplaySnapshot, FocusSnapshot, LocationHint};

#[derive(Debug, Clone, PartialEq)]
pub struct InterestDestination {
    pub point: WorldPoint,
    pub display_id: String,
    pub score: f64,
}

/// How a seat looks right now, so a caller can decide to stay put.
///
/// Re-planning on every agent event makes the pet twitch across the screen
/// while nothing about the seat got worse. `is_holdable` is the question that
/// deserves asking first: is the pet covering the user's work, and is it still
/// watching the window it was sent to.
#[derive(Debug, Clone, PartialEq)]
pub struct SeatEvaluation {
    pub point: WorldPoint,
    pub display_id: String,
    pub score: f64,
    /// None when no capture was available or the seat fell outside it, which
    /// reads as "cannot tell" rather than "busy".
    pub emptiness: Option<f64>,
    pub covers_caret: bool,
    /// False once the seat no longer belongs to the window it was planned for,
    /// which is how a focus change unsticks a held seat.
    pub watches_region: bool,
}

impl SeatEvaluation {
    pub fn is_holdable(&self) -> bool {
        self.watches_region
            && !self.covers_caret
            && self.emptiness.unwrap_or(1.0) >= BasicInterestPositionPlanner::HOLD_EMPTINESS
    }
}

struct Plan<'a> {
    region: WorldRect,
    display: &'a DisplaySnapshot,
    safe: WorldRect,
    focus: Option<&'a FocusSnapshot>,
    field: Option<&'a LuminanceField>,
    confidence: f64,
    bottom_y: f64,
    object_size: WorldSize,
    current_position: WorldPoint,
    pointer_position: Option<WorldPoint>,
}

impl Plan<'_> {
    fn half_width(&self) -> f64 { self.object_size.width / 2.0 }
    fn half_height(&self) -> f64 { self.object_size.height / 2.0 }

    fn pet_frame(&self, point: WorldPoint) -> WorldRect {
        WorldRect::new(
            point.x - self.half_width(),
            point.y - self.half_height(),
            self.object_size.width,
            self.object_size.height,
        )
    }
}

/// Placement favours the bottom edge and sits just outside the window when
/// space exists. Without accessibility it sees only a coarse window region.
/// With focus it prefers the focused window's frame, leans toward the caret and
/// refuses to sit on top of it. With a capture it also sweeps the lower window
/// for a gap that is actually empty.
pub struct BasicInterestPositionPlanner;

impl BasicInterestPositionPlanner {
    /// Extra margin kept between the pet and the insertion point.
    const CARET_CLEARANCE: f64 = 12.0;
    /// A held seat may not look busier than this. The field is downsampled, so
    /// it answers "is the pet parked on content", never "is a glyph under a paw".
    pub const HOLD_EMPTINESS: f64 = 0.55;
    /// A seat only counts as verified empty above this. Overlapping the focused
    /// control is forgiven here and nowhere else, so it sits well clear of the
    /// hold threshold rather than next to it.
    const CLEAR_EMPTINESS: f64 = 0.85;
    /// How far a seat may sit outside its window and still count as watching it.
    /// It has to reach at least as far as the seats placed beside the window, or
    /// a seat just chosen reads as no longer watching and the caller moves the
    /// pet again on the very next review, forever.
    const HOLD_REGION_MARGIN: f64 = 48.0;
    /// The caret marches right as the user types, so a seat on its line and
    /// ahead of it is buried within seconds. It outweighs the proximity bonus on
    /// purpose and stays below the penalty for covering the caret outright.
    const CARET_ADVANCE_PENALTY: f64 = 60.0;

    pub fn destination(
        hint: &LocationHint,
        world: &DesktopWorldSnapshot,
        current_position: WorldPoint,
        pointer_position: Option<WorldPoint>,
        object_size: WorldSize,
    ) -> Option<InterestDestination> {
        let plan = Self::make_plan(hint, world, current_position, pointer_position, object_size)?;
        let evaluations: Vec<SeatEvaluation> = Self::candidates(&plan)
            .into_iter()
            .map(|(point, outside)| Self::evaluate(point, outside, &plan))
            .collect();

        // Swift's `max(by:)` keeps the last of equal elements, and on a tied
        // score the comparator prefers the nearer seat.
        let best = last_maximum(&evaluations, |lhs, rhs| {
            if lhs.score == rhs.score {
                current_position.distance(lhs.point) > current_position.distance(rhs.point)
            } else {
                lhs.score < rhs.score
            }
        })?;
        Some(InterestDestination {
            point: best.point,
            display_id: best.display_id.clone(),
            score: best.score,
        })
    }

    /// Scores an arbitrary point with the formula the planner uses, so the seat
    /// the pet already occupies can be compared against a fresh candidate
    /// without either side drifting to its own scale.
    pub fn evaluate_seat(
        point: WorldPoint,
        hint: &LocationHint,
        world: &DesktopWorldSnapshot,
        current_position: WorldPoint,
        pointer_position: Option<WorldPoint>,
        object_size: WorldSize,
    ) -> Option<SeatEvaluation> {
        let plan = Self::make_plan(hint, world, current_position, pointer_position, object_size)?;
        Some(Self::evaluate(point, false, &plan))
    }

    fn make_plan<'a>(
        hint: &LocationHint,
        world: &'a DesktopWorldSnapshot,
        current_position: WorldPoint,
        pointer_position: Option<WorldPoint>,
        object_size: WorldSize,
    ) -> Option<Plan<'a>> {
        let focus = world.focus.as_ref().filter(|value| value.confidence > 0.0);
        // The focused window frame is exact where the coarse hint only knows the
        // frontmost process, so it wins when accessibility supplied one.
        let focused_window_frame = focus.and_then(|value| value.window_frame);
        let confidence = swift_max(hint.confidence, focus.map_or(0.0, |v| v.confidence));
        if !(confidence > 0.0) {
            return None;
        }
        let region = focused_window_frame.or(hint.approximate_region)?;
        let display = world
            .display_containing(region.center())
            .or_else(|| world.nearest_display(region.center()))?;

        let safe = display
            .visible_frame
            .inset_by(object_size.width / 2.0 + 10.0, object_size.height / 2.0 + 10.0);
        if safe.is_empty() {
            return None;
        }

        Some(Plan {
            region,
            display,
            safe,
            focus,
            field: world.luminance.as_ref(),
            confidence,
            bottom_y: region.max_y() - object_size.height / 2.0 - 14.0,
            object_size,
            current_position,
            pointer_position,
        })
    }

    /// Candidate seats in the order they were proposed, each carrying whether it
    /// was meant to land outside the window.
    ///
    /// Ordered, not a map. Two mirrored seats either side of a window score
    /// identically and sit the same distance away, and the winner of a dead heat
    /// is whichever came first -- which for a hash map is whatever the seed
    /// decided that launch.
    fn candidates(plan: &Plan<'_>) -> Vec<(WorldPoint, bool)> {
        let region = plan.region;
        let mut raw: Vec<(WorldPoint, bool)> = vec![
            (WorldPoint::new(region.min_x() - plan.half_width() - 14.0, plan.bottom_y), true),
            (WorldPoint::new(region.max_x() + plan.half_width() + 14.0, plan.bottom_y), true),
            (WorldPoint::new(region.min_x() + plan.half_width() + 18.0, plan.bottom_y), false),
            (WorldPoint::new(region.max_x() - plan.half_width() - 18.0, plan.bottom_y), false),
        ];

        // Four seats at the two edges give emptiness nothing to choose between,
        // and a single bottom line only ever finds the busiest part of a
        // terminal -- the prompt. With a capture, sweep the window so the score
        // can find the gap that is actually there, wherever it is.
        if plan.field.is_some() {
            let inner_left = region.min_x() + plan.half_width() + 18.0;
            let inner_right = region.max_x() - plan.half_width() - 18.0;
            let top_limit = region.min_y() + plan.half_height() + 18.0;
            if inner_right > inner_left {
                let row_step = plan.object_size.height * 1.15;
                for row in 0..6 {
                    let y = plan.bottom_y - row_step * f64::from(row);
                    if !(y >= top_limit) {
                        break;
                    }
                    let columns = 6;
                    for step in 1..columns {
                        let ratio = f64::from(step) / f64::from(columns);
                        let x = inner_left + (inner_right - inner_left) * ratio;
                        raw.push((WorldPoint::new(x, y), false));
                    }
                    if row > 0 {
                        raw.push((WorldPoint::new(inner_left, y), false));
                        raw.push((WorldPoint::new(inner_right, y), false));
                    }
                }
            }
        }

        // Clamping collapses candidates onto each other near a screen edge, so
        // duplicates merge -- first proposal keeps its place, and being wanted
        // outside by any of them wins.
        let mut order: Vec<WorldPoint> = Vec::new();
        let mut outside: Vec<bool> = Vec::new();
        for (point, wants_outside) in raw {
            let clamped = plan.safe.closest_point(point);
            match order.iter().position(|existing| *existing == clamped) {
                Some(index) => outside[index] = outside[index] || wants_outside,
                None => {
                    order.push(clamped);
                    outside.push(wants_outside);
                }
            }
        }
        order.into_iter().zip(outside).collect()
    }

    fn evaluate(point: WorldPoint, intended_outside: bool, plan: &Plan<'_>) -> SeatEvaluation {
        let frame = plan.pet_frame(point);
        let is_outside = !plan.region.contains(point);
        // Sitting beside the window is safe but it is also how the pet ends up
        // parked at a screen edge, far from the work, whenever the window is
        // large. A tiebreaker, not an argument that outranks a seat the capture
        // confirmed is empty.
        let outside_bonus = if intended_outside && is_outside { 12.0 } else { 0.0 };
        let pointer_penalty = plan
            .pointer_position
            .map_or(0.0, |pointer| swift_max(0.0, 220.0 - pointer.distance(point)) / 8.0);
        let travel_penalty = swift_min(18.0, plan.current_position.distance(point) / 220.0);
        let bottom_distance = (point.y - plan.bottom_y).abs();
        let bottom_edge_score = swift_max(0.0, 12.0 - bottom_distance / 24.0);

        let emptiness = plan.field.and_then(|field| VisualEmptiness::score(frame, field));

        // Sitting near the caret is the point of this gate. Sitting on top of it
        // is the one thing it must never do, so the occlusion penalty outweighs
        // every bonus a candidate can earn.
        let mut caret_affinity = 0.0;
        let mut occlusion_penalty = 0.0;
        let mut advance_penalty = 0.0;
        let mut covers_caret = false;
        if let Some(focus) = plan.focus {
            if let Some(caret) = focus.caret_frame {
                // Falling off over a hundred points rather than a thousand is
                // what makes this a reason to pick a seat.
                caret_affinity = swift_max(0.0, 40.0 - caret.distance(point) / 12.0);
                if frame.intersects(&caret, Self::CARET_CLEARANCE) {
                    covers_caret = true;
                    occlusion_penalty += 120.0;
                }
                // The caret marches right through everything on its line, so a
                // seat ahead of it is empty now and buried in a sentence.
                let shares_line = frame.max_y() >= caret.min_y() && caret.max_y() >= frame.min_y();
                if shares_line && frame.max_x() > caret.min_x() {
                    advance_penalty = Self::CARET_ADVANCE_PENALTY;
                }
            }
            // Overlapping the focused control is a guess about content, and a
            // capture answers it directly. All-or-nothing on purpose: a partial
            // discount let the pet inch onto body text that merely scored
            // middling.
            if let Some(element) = focus.focused_element_frame {
                if frame.intersects(&element, 0.0) {
                    occlusion_penalty +=
                        if emptiness.unwrap_or(0.0) >= Self::CLEAR_EMPTINESS { 0.0 } else { 40.0 };
                }
            }
        }

        // Emptiness only ranks seats that already passed the caret and pointer
        // checks, so it can move the pet along the sweep but never onto
        // something it must avoid.
        let visual_empty_score = emptiness.unwrap_or(0.0) * 34.0;

        let watch_margin = swift_max(Self::HOLD_REGION_MARGIN, plan.half_width() + 24.0);
        let watched = WorldRect::new(
            plan.region.min_x() - watch_margin,
            plan.region.min_y() - watch_margin,
            plan.region.size.width + watch_margin * 2.0,
            plan.region.size.height + watch_margin * 2.0,
        );

        SeatEvaluation {
            point,
            display_id: plan.display.id.clone(),
            score: plan.confidence * 30.0 + outside_bonus + bottom_edge_score + caret_affinity
                + visual_empty_score
                - pointer_penalty
                - travel_penalty
                - occlusion_penalty
                - advance_penalty,
            emptiness,
            covers_caret,
            watches_region: watched.contains(point),
        }
    }
}
