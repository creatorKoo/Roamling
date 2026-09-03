// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Ported from `Sources/RoamlingCore/PlacementDirector.swift`.
//!
//! The one place that answers where the pet should be. Placement used to be
//! decided in four unrelated code paths that shared mutable runtime state, so a
//! rule added to one of them silently did not apply to the other three.
//! `docs/placement.md` records the decision table this implements and why the
//! thresholds are asymmetric.
//!
//! It holds a seat, a trip and the time of the last review across ticks, so
//! Swift keeps a handle rather than shipping the state in and out every frame.

use crate::emptiness::VisualEmptiness;
use crate::geometry::{clamped, swift_max, swift_min, WorldPoint, WorldRect, WorldSize};
use crate::interest::{BasicInterestPositionPlanner, InterestDestination, SeatEvaluation};
use crate::world::{DesktopWorldSnapshot, LocationHint};

/// Why the director is sending the pet somewhere.
///
/// The reason is not decoration. A caller has to know whether it may interrupt
/// a nap to obey the move, and every placement bug so far was easier to read as
/// "it travelled for the wrong reason" than as a wrong coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementTravelReason {
    /// A source the pet was not already watching started working.
    NewActivity,
    CoveringCaret,
    CoveringWork,
    /// The seat was chosen before any capture existed and one has since
    /// arrived, so the decision gets re-made rather than defended.
    PlannedBlind,
    /// The seat no longer belongs to the window being watched.
    FollowedFocus,
}

/// The single answer to "where should the pet be right now".
#[derive(Debug, Clone, PartialEq)]
pub enum PlacementIntent {
    /// Something else owns the pet -- a catch, a drag, a pointer reaction, or an
    /// evade. The seat verdict behind this was still computed; only the move
    /// waits, so the tick the pointer lets go acts on a current answer.
    None,
    Hold,
    Travel(InterestDestination, PlacementTravelReason),
    SleepInPlace,
    Stroll(WorldPoint),
    /// The spot the pet is standing on turned out to be covered, so this is a
    /// walk it owes the user rather than one it fancied. Kept apart from
    /// `Stroll` because callers rank it differently: an aimless walk yields to
    /// the cursor, and getting off someone's paragraph does not.
    Escape(WorldPoint),
}

impl PlacementIntent {
    pub fn travel_reason(&self) -> Option<PlacementTravelReason> {
        match self {
            Self::Travel(_, reason) => Some(*reason),
            _ => Option::None,
        }
    }
}

/// Everything the placement decision is allowed to look at, gathered once per
/// tick by the platform adapter.
///
/// A value on purpose. The decision used to read seventeen mutable fields on
/// the runtime that four separate code paths wrote to, and every placement
/// defect in MVP 4 was one path failing to set what another read.
#[derive(Debug, Clone)]
pub struct PetSituation {
    pub timestamp: f64,
    /// Displays, plus focus and luminance when the user granted them.
    pub world: DesktopWorldSnapshot,
    pub position: WorldPoint,
    pub object_size: WorldSize,
    pub pointer_position: Option<WorldPoint>,
    pub walking_speed: f64,
    /// The pointer owns the pet outright: caught, dragged, evading, or close
    /// enough to be reaching for it. Nothing placement decides survives this.
    pub is_pointer_owned: bool,
    /// The weaker claim: the cursor is in the outer band and the pet has
    /// stopped to look at it. It is a moment, and it stops the pet exactly
    /// where it stands -- which may be the paragraph the user is reading.
    pub is_pointer_watching: bool,
    pub is_evading: bool,
    /// A walk is already under way, so nothing here should start another one.
    pub is_walking: bool,
    /// The pet is sitting, seeking a sleep spot, or asleep. Rest owns movement
    /// while that lasts, so planning a stroll it cannot take is wasted work.
    pub is_resting: bool,
    pub activity_source_id: Option<String>,
    pub activity_hint: Option<LocationHint>,
    pub user_idle_duration: f64,
    pub idle_before_rest: f64,
    pub is_roaming_enabled: bool,
    /// The roaming pause has run out. Pacing belongs to the caller because a
    /// catch, a drop and a display change all extend it for reasons that have
    /// nothing to do with placement.
    pub is_stroll_due: bool,
    /// Aimless destinations for the director to filter. Keeping the sampling
    /// outside means roaming stays random without the decision being random.
    pub stroll_candidates: Vec<WorldPoint>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlacementConfiguration {
    /// A seat has to look at least this empty to be worth taking or keeping.
    pub hold_emptiness: f64,
    /// The bar for walking away from a seat, deliberately the same as the bar
    /// for taking one. A lower bar was tried and measured: on a real desktop it
    /// turned 15% of the screen into seats that are on text and yet not bad
    /// enough to leave.
    pub abandon_emptiness: f64,
    /// How long a fresh seat is defended against `CoveringWork` alone, so that
    /// a screen changing under the pet cannot move it at frame rate.
    pub seat_dwell: f64,
    /// Scoring a seat is cheap, planning a new one is not, and neither is worth
    /// doing at frame rate.
    pub review_interval: f64,
    /// How much better a replacement has to be when it is not itself clear.
    /// This is the actual fix for the seat that would not settle.
    pub replacement_margin: f64,
    /// Below this a "new" seat is the seat the pet already has.
    pub reseat_distance: f64,
    /// Shorter than this is not a walk worth watching.
    pub minimum_travel_distance: f64,
    pub arrival_tolerance: f64,
}

impl PlacementConfiguration {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        hold_emptiness: f64,
        abandon_emptiness: f64,
        seat_dwell: f64,
        review_interval: f64,
        replacement_margin: f64,
        reseat_distance: f64,
        minimum_travel_distance: f64,
        arrival_tolerance: f64,
    ) -> Self {
        // `abandon` is clamped against the *assigned* hold, not the argument --
        // Swift writes `self.holdEmptiness` there explicitly, and the two differ
        // whenever the hold argument was out of range.
        let hold_emptiness = clamped(hold_emptiness, 0.0, 1.0);
        Self {
            hold_emptiness,
            abandon_emptiness: swift_min(clamped(abandon_emptiness, 0.0, 1.0), hold_emptiness),
            seat_dwell: swift_max(0.0, seat_dwell),
            review_interval: swift_max(0.0, review_interval),
            replacement_margin: swift_max(0.0, replacement_margin),
            reseat_distance: swift_max(0.0, reseat_distance),
            minimum_travel_distance: swift_max(0.0, minimum_travel_distance),
            arrival_tolerance: swift_max(0.5, arrival_tolerance),
        }
    }
}

impl Default for PlacementConfiguration {
    fn default() -> Self {
        Self::new(
            BasicInterestPositionPlanner::HOLD_EMPTINESS,
            BasicInterestPositionPlanner::HOLD_EMPTINESS,
            2.5,
            0.5,
            15.0,
            24.0,
            18.0,
            4.0,
        )
    }
}

/// A seat the pet is standing on. Its coordinate is deliberately absent: the
/// pet is the seat, so a drag or a display change cannot leave the director
/// judging a spot the pet no longer occupies.
#[derive(Debug, Clone)]
struct Seat {
    source_id: String,
    taken_at: f64,
    /// Whether a capture existed when this seat was chosen. Without one the
    /// planner never sweeps the window and only the seats beside it remain,
    /// which is how a pet ends up in a corner for a whole session.
    saw_capture: bool,
}

#[derive(Debug, Clone)]
struct Travel {
    destination: InterestDestination,
    reason: PlacementTravelReason,
    source_id: String,
    started_at: f64,
    saw_capture: bool,
}

#[derive(Debug, Clone)]
pub struct PlacementDirector {
    configuration: PlacementConfiguration,
    seat: Option<Seat>,
    travel: Option<Travel>,
    parked_since: Option<f64>,
    last_review_at: f64,
    /// The verdict from the last review, repeated between beats so a walk in
    /// progress keeps its destination instead of restarting every frame.
    carried: PlacementIntent,
}

impl Default for PlacementDirector {
    fn default() -> Self {
        Self::new(PlacementConfiguration::default())
    }
}

impl PlacementDirector {
    pub fn new(configuration: PlacementConfiguration) -> Self {
        Self {
            configuration,
            seat: Option::None,
            travel: Option::None,
            parked_since: Option::None,
            last_review_at: f64::NEG_INFINITY,
            carried: PlacementIntent::Hold,
        }
    }

    pub fn configuration(&self) -> PlacementConfiguration {
        self.configuration
    }

    /// True while the pet is parked on a seat it picked for the current source.
    pub fn is_seated(&self) -> bool {
        self.seat.is_some() && self.travel.is_none()
    }

    pub fn is_travelling(&self) -> bool {
        self.travel.is_some()
    }

    pub fn decide(&mut self, situation: &PetSituation) -> PlacementIntent {
        let verdict = self.verdict(situation);
        // Priorities 1 and 2. The table above them was still read, so the
        // verdict is current when the pointer lets go. Gating the reading as
        // well as the moving is what froze the seat watch next to the cursor.
        if situation.is_pointer_owned || situation.is_evading {
            return PlacementIntent::None;
        }
        // The glance is the one pointer claim that loses, and only to the one
        // answer that outranks it. Standing on the user's work is a condition;
        // looking up at the cursor is a moment, and the moment happens to stop
        // the pet right where the condition is. Everything else still waits.
        if situation.is_pointer_watching && !matches!(verdict, PlacementIntent::Escape(_)) {
            return PlacementIntent::None;
        }
        verdict
    }

    /// Drops the trip in progress and treats where the pet stands as its seat.
    /// A setback ends the walk without ending the watch, so the caller names the
    /// source the seat now belongs to rather than letting it keep the one the
    /// abandoned trip was for.
    pub fn settle_in_place(&mut self, owner: Option<&str>, timestamp: f64) {
        let source_id = owner
            .map(str::to_owned)
            .or_else(|| self.travel.as_ref().map(|travel| travel.source_id.clone()))
            .or_else(|| self.seat.as_ref().map(|seat| seat.source_id.clone()));
        let Some(source_id) = source_id else { return };
        // Read before the seat is replaced, which is what Swift's evaluation
        // order does: the old seat is still there while the new one is built.
        let saw_capture = self
            .travel
            .as_ref()
            .map(|travel| travel.saw_capture)
            .or_else(|| self.seat.as_ref().map(|seat| seat.saw_capture))
            .unwrap_or(false);
        self.seat = Some(Seat { source_id, taken_at: timestamp, saw_capture });
        self.travel = Option::None;
        self.carried = PlacementIntent::Hold;
    }

    fn verdict(&mut self, situation: &PetSituation) -> PlacementIntent {
        let (Some(source_id), Some(hint)) = (
            situation.activity_source_id.as_ref(),
            situation.activity_hint.as_ref(),
        ) else {
            self.seat = Option::None;
            self.travel = Option::None;
            return self.stroll_verdict(situation);
        };
        self.parked_since = Option::None;

        // A different agent is a different window. The pet walks over to it
        // rather than claiming wherever it happens to be standing.
        if self.seat.as_ref().map(|seat| seat.source_id.as_str()) != Some(source_id.as_str()) {
            self.seat = Option::None;
        }
        if self.travel.as_ref().map(|travel| travel.source_id.as_str()) != Some(source_id.as_str())
        {
            self.travel = Option::None;
        }

        // Arrival runs every tick, not on the review beat: a pet that reached
        // its seat has to react now, not up to half a second later.
        //
        // Whether the seat was chosen blind is a fact about the moment it was
        // chosen, and the walk to it is exactly when the capture tends to land.
        // Folding the arrival's capture in here marked every blind seat as
        // informed and switched `PlannedBlind` off in practice.
        if let Some(travel) = &self.travel {
            let arrived = situation.position.distance(travel.destination.point)
                <= self.configuration.arrival_tolerance;
            // A walk that cannot finish must not own the pet for the rest of
            // the session, however it got stuck.
            let expired = situation.timestamp - travel.started_at > self.timeout(travel, situation);
            if arrived || expired {
                let saw_capture = travel.saw_capture;
                return self.settle(source_id.clone(), saw_capture, situation.timestamp);
            }
        }

        let is_new = self.seat.is_none() && self.travel.is_none();
        if !(is_new
            || situation.timestamp - self.last_review_at >= self.configuration.review_interval)
        {
            return self.carried.clone();
        }
        self.last_review_at = situation.timestamp;

        let judged = self
            .travel
            .as_ref()
            .map(|travel| travel.destination.point)
            .unwrap_or(situation.position);
        let evaluation = BasicInterestPositionPlanner::evaluate_seat(
            judged,
            hint,
            &situation.world,
            situation.position,
            situation.pointer_position,
            situation.object_size,
        );
        let saw_capture = self
            .travel
            .as_ref()
            .map(|travel| travel.saw_capture)
            .or_else(|| self.seat.as_ref().map(|seat| seat.saw_capture))
            .unwrap_or(false);

        if let Some(reason) =
            self.departure_reason(evaluation.as_ref(), saw_capture, is_new, situation)
        {
            if let Some(destination) = BasicInterestPositionPlanner::destination(
                hint,
                &situation.world,
                situation.position,
                situation.pointer_position,
                situation.object_size,
            ) {
                if self.accepts(
                    &destination,
                    evaluation.as_ref(),
                    judged,
                    reason,
                    hint,
                    situation,
                ) {
                    self.travel = Some(Travel {
                        destination: destination.clone(),
                        reason,
                        source_id: source_id.clone(),
                        started_at: situation.timestamp,
                        saw_capture: situation.world.luminance.is_some(),
                    });
                    self.seat = Option::None;
                    self.carried = PlacementIntent::Travel(destination, reason);
                    return self.carried.clone();
                }
            }
        }

        // Nothing better exists, so the walk already under way continues rather
        // than being restarted from here.
        if let Some(carried) = self
            .travel
            .as_ref()
            .map(|travel| PlacementIntent::Travel(travel.destination.clone(), travel.reason))
        {
            self.carried = carried;
            return self.carried.clone();
        }

        if self.seat.is_none() {
            // No seat was worth walking to, which still means this window is
            // the one being watched. Leaving that unrecorded is what stranded
            // the old seat watch for a whole session.
            self.seat = Some(Seat {
                source_id: source_id.clone(),
                taken_at: situation.timestamp,
                saw_capture: situation.world.luminance.is_some(),
            });
        }

        // Priority 7. A pet dozing beside a working agent keeps the seat it
        // already vetted instead of walking to a display corner to sleep.
        // "Cannot tell" reads as fine here, the same way a missing capture does.
        if evaluation
            .as_ref()
            .map(SeatEvaluation::is_holdable)
            .unwrap_or(true)
            && situation.user_idle_duration >= situation.idle_before_rest
        {
            self.carried = PlacementIntent::SleepInPlace;
            return self.carried.clone();
        }
        self.carried = PlacementIntent::Hold;
        self.carried.clone()
    }

    /// Priorities 3 through 6, in order. Above them is only ownership, below
    /// them only staying put.
    fn departure_reason(
        &self,
        evaluation: Option<&SeatEvaluation>,
        saw_capture: bool,
        is_new: bool,
        situation: &PetSituation,
    ) -> Option<PlacementTravelReason> {
        if is_new {
            return Some(PlacementTravelReason::NewActivity);
        }
        // No answer is not a bad answer. Moving because the seat could not be
        // scored would walk the pet on exactly the screens it understands least.
        let evaluation = evaluation?;
        if evaluation.covers_caret {
            return Some(PlacementTravelReason::CoveringCaret);
        }
        if let Some(emptiness) = evaluation.emptiness {
            if emptiness < self.configuration.abandon_emptiness && self.dwell_elapsed(situation) {
                return Some(PlacementTravelReason::CoveringWork);
            }
        }
        if !saw_capture && situation.world.luminance.is_some() {
            return Some(PlacementTravelReason::PlannedBlind);
        }
        if !evaluation.watches_region {
            return Some(PlacementTravelReason::FollowedFocus);
        }
        Option::None
    }

    fn dwell_elapsed(&self, situation: &PetSituation) -> bool {
        match &self.seat {
            Option::None => true,
            Some(seat) => situation.timestamp - seat.taken_at >= self.configuration.seat_dwell,
        }
    }

    fn accepts(
        &self,
        destination: &InterestDestination,
        evaluation: Option<&SeatEvaluation>,
        judged: WorldPoint,
        reason: PlacementTravelReason,
        hint: &LocationHint,
        situation: &PetSituation,
    ) -> bool {
        if !(situation.position.distance(destination.point)
            > self.configuration.minimum_travel_distance
            && destination.point.distance(judged) > self.configuration.reseat_distance)
        {
            return false;
        }

        match reason {
            PlacementTravelReason::CoveringWork => {
                // A seat that is genuinely empty ends this in one move: measured
                // on a real desktop a clear seat scores around 0.97, nowhere near
                // the bar it would have to fall back under to move the pet again.
                let replacement = BasicInterestPositionPlanner::evaluate_seat(
                    destination.point,
                    hint,
                    &situation.world,
                    situation.position,
                    situation.pointer_position,
                    situation.object_size,
                );
                if replacement
                    .as_ref()
                    .map(SeatEvaluation::is_holdable)
                    .unwrap_or(false)
                {
                    return true;
                }
                // Otherwise the pet would be trading one marginal seat for
                // another, and that trade has to be worth watching.
                let Some(evaluation) = evaluation else { return true };
                destination.score > evaluation.score + self.configuration.replacement_margin
            }
            PlacementTravelReason::NewActivity => {
                // Walking over is the point of this priority, but only when
                // there is somewhere better to be. Being on another display is a
                // reason and the score does not say so -- measured across two
                // displays the corner seat beat a clear one on the wrong screen
                // by 6.9, well under the margin -- so it is asked directly.
                let Some(evaluation) = evaluation else { return true };
                if !evaluation.watches_region {
                    return true;
                }
                destination.score > evaluation.score + self.configuration.replacement_margin
            }
            PlacementTravelReason::CoveringCaret
            | PlacementTravelReason::PlannedBlind
            | PlacementTravelReason::FollowedFocus => true,
        }
    }

    fn settle(
        &mut self,
        source_id: String,
        saw_capture: bool,
        timestamp: f64,
    ) -> PlacementIntent {
        self.seat = Some(Seat { source_id, taken_at: timestamp, saw_capture });
        self.travel = Option::None;
        self.last_review_at = timestamp;
        self.carried = PlacementIntent::Hold;
        self.carried.clone()
    }

    /// Long enough for the walk plus the slowdown at every waypoint, and never
    /// so short that a legitimate cross-display trip counts as stuck.
    fn timeout(&self, travel: &Travel, situation: &PetSituation) -> f64 {
        let distance = situation.position.distance(travel.destination.point);
        8.0 + distance / swift_max(20.0, situation.walking_speed) * 2.0
    }

    /// Priorities 10 and 11. Wandering is where the pet spends most of its life,
    /// so it passes the same emptiness bar as an interest seat -- a rule that
    /// only applied to agent seats left most of the day unruled.
    fn stroll_verdict(&mut self, situation: &PetSituation) -> PlacementIntent {
        self.carried = PlacementIntent::Hold;
        let first = situation.stroll_candidates.first().copied();
        let (true, Some(first)) = (situation.is_roaming_enabled, first) else {
            self.parked_since = Option::None;
            return self.carried.clone();
        };
        if situation.is_walking || situation.is_resting {
            self.parked_since = Option::None;
            return self.carried.clone();
        }
        if self.parked_since.is_none() {
            self.parked_since = Some(situation.timestamp);
        }

        if situation.is_stroll_due {
            self.parked_since = Option::None;
            // Deliberately not stored in `carried`: a stroll is acted on once,
            // and repeating it between review beats would relay the same walk.
            return PlacementIntent::Stroll(self.comfortable(situation).unwrap_or(first));
        }

        // The pause between walks is the whole point of roaming, and it is also
        // long enough for the user to scroll a paragraph under a pet that is
        // just sitting there. Nothing else is watching during it, so this is.
        let Some(field) = situation.world.luminance.as_ref() else {
            return self.carried.clone();
        };
        let Some(parked_since) = self.parked_since else {
            return self.carried.clone();
        };
        if !(situation.timestamp - parked_since >= self.configuration.seat_dwell) {
            return self.carried.clone();
        }
        let Some(score) = VisualEmptiness::score(
            frame(situation.position, situation.object_size),
            field,
        ) else {
            return self.carried.clone();
        };
        if !(score < self.configuration.hold_emptiness) {
            return self.carried.clone();
        }
        let Some(escape) = self.comfortable(situation) else {
            return self.carried.clone();
        };
        // Only somewhere genuinely clear, for the same reason a seat is:
        // trading one covered spot for another just paces the pet.
        let Some(escape_score) =
            VisualEmptiness::score(frame(escape, situation.object_size), field)
        else {
            return self.carried.clone();
        };
        if !(escape_score >= self.configuration.hold_emptiness) {
            return self.carried.clone();
        }
        if !(situation.position.distance(escape) > self.configuration.minimum_travel_distance) {
            return self.carried.clone();
        }

        // `parked_since` deliberately survives this. The decision is only worth
        // making if it is still here when it can be acted on, and `decide`
        // throws away every answer while the pointer owns the pet -- so
        // clearing the dwell here meant a cursor drifting past at the wrong
        // moment restarted the wait, over and over, and the pet kept the
        // paragraph it was supposed to be leaving. Walking clears it instead,
        // at the top of this function, which is the point at which the answer
        // has actually been used.
        PlacementIntent::Escape(escape)
    }

    fn comfortable(&self, situation: &PetSituation) -> Option<WorldPoint> {
        situation.world.luminance.as_ref().and_then(|field| {
            VisualEmptiness::first_comfortable(
                &situation.stroll_candidates,
                situation.object_size,
                field,
                self.configuration.hold_emptiness,
            )
        })
    }
}

fn frame(point: WorldPoint, size: WorldSize) -> WorldRect {
    WorldRect::new(
        point.x - size.width / 2.0,
        point.y - size.height / 2.0,
        size.width,
        size.height,
    )
}
