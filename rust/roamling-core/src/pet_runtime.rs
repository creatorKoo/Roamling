// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! The tick body, ported from `Sources/RoamlingEngine/RoamlingRuntime.swift`.
//!
//! Everything the pet decides, in one object: where it walks, when it evades,
//! when it sits down, what it wears. The runtime around it keeps only the parts
//! that are not decisions -- the timer, the defaults, the diagnostics file, the
//! agent subscriptions and the sprite sheet.
//!
//! This is the unit a second shell needs. `docs/history/windows.md` unit 6c: a Rust
//! Windows shell has nothing to call until the orchestrator is here, because
//! Rust cannot call Swift.
//!
//! It answers in values rather than reaching for anything. Two questions have
//! to go back out mid-tick -- whether to pay for an accessibility round trip,
//! and whether to ask for a screen capture -- so the tick is two calls with the
//! platform's answer in between.

use crate::activity::{CompanionEvent, CompanionReaction};
use crate::activity_director::{ActivityDirector, ActivityEffect};
use crate::behavior::BehaviorController;
use crate::behavior::{BehaviorInput, BehaviorState};
use crate::capability::{capability_for, PetCapability};
use crate::display_policy::{crossing, placement_frame, placement_world};
use crate::emptiness::LuminanceField;
use crate::geometry::{swift_max, swift_min, WorldPoint, WorldRect, WorldSize, WorldVector};
use crate::movement::{MovementConfiguration, MovementController};
use crate::placement::{PetSituation, PlacementDirector, PlacementIntent, PlacementTravelReason};
use crate::pointer::{PointerDecision, PointerInteractionModel, PointerProximity};
use crate::safe_zone::{BasicSafeZonePlanner, RestDestination};
use crate::source_state::StateDeclaration;
use crate::topology::DisplayTopology;
use crate::tuning::RuntimeTuning;
use crate::world::{DesktopWorldSnapshot, DisplaySnapshot, FocusSnapshot, LocationHint};

/// How long the pet dozes before the capture is asked for again.
const LUMINANCE_REFRESH_INTERVAL: f64 = 3.0;
const RESTING_LUMINANCE_REFRESH_INTERVAL: f64 = 6.0;
const ROAMING_LUMINANCE_REFRESH_INTERVAL: f64 = 6.0;
const FOCUS_REFRESH_INTERVAL: f64 = 0.5;
const WANDER_CANDIDATE_COUNT: usize = 6;

/// Sitting and waking are authored pacing rather than a preference, so only the
/// idle threshold is tunable. Ported from `RestConfiguration`.
const SITTING_DURATION: f64 = 2.4;
const WAKE_WANDER_DELAY: f64 = 2.5;

/// The pet's aimlessness, made repeatable.
///
/// Deliberately the same xorshift the recorded session was captured with, down
/// to the modulus: the port is proved by replaying that session, and a
/// different sequence would only prove that both implementations are random.
#[derive(Debug, Clone, Copy)]
pub struct Aimlessness {
    seed: u64,
    draws: u64,
}

impl Aimlessness {
    /// A zero seed makes xorshift produce nothing but zeros, so it is refused
    /// rather than silently turning the pet metronomic.
    pub fn new(seed: u64) -> Self {
        Self {
            seed: if seed == 0 {
                0x2545_F491_4F6C_DD1D
            } else {
                seed
            },
            draws: 0,
        }
    }

    pub fn draws(&self) -> u64 {
        self.draws
    }

    pub fn unit(&mut self) -> f64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.draws += 1;
        (self.seed % 1_000_000) as f64 / 1_000_000.0
    }

    /// A draw turned into an index. The clamp is spelled out because a 1.0
    /// would step off the end of the array.
    fn index(&mut self, count: usize) -> usize {
        if count == 0 {
            return 0;
        }
        let raw = (self.unit() * count as f64) as i64;
        raw.clamp(0, count as i64 - 1) as usize
    }
}

/// What the platform gathered for this tick.
#[derive(Debug, Clone)]
pub struct TickInput {
    pub now: f64,
    pub pointer: WorldPoint,
    pub primary_button_down: bool,
    pub user_idle_duration: f64,
    /// True when the capture permission is granted, which is a different thing
    /// from having a capture: a seat chosen blind is re-decided when one lands.
    pub capture_authorized: bool,
    pub focus_authorized: bool,
    /// Whether the caller actually paid for the query this tick. A query that
    /// came back empty is not the same as no query: it still resets the clock
    /// on the next one.
    pub did_query_focus: bool,
    pub queried_focus: Option<FocusSnapshot>,
    /// Whether the cursor is inside the pet's hit region. Only the platform
    /// knows how big that is once the panel has scaled it.
    pub pointer_is_over_pet: bool,
    /// The user is holding the affection key -- left Command on macOS, left
    /// Control on Windows. A cursor that comes close while it is held is a
    /// hand, not a threat: the pet watches it instead of stepping away, and
    /// once the hand is on it, sits up for it. Catching still works, since a
    /// hand can also pick the pet up.
    pub affection_held: bool,
}

/// A capture the runtime would like, near a region it cares about. The caller
/// owns the permission, the task and the throttle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LuminanceRequest {
    pub region: WorldRect,
    pub interval: f64,
}

/// What the platform must do about this tick.
#[derive(Debug, Clone)]
pub struct TickOutput {
    /// The clamped step this tick advanced by. The caller drives the animation
    /// player with it, scaled by `locomotion_rate`.
    pub delta_time: f64,
    pub position: WorldPoint,
    pub state: BehaviorState,
    pub capability: PetCapability,
    /// Only while the pet is looking at the cursor; nil turns the head back.
    pub look_direction_degrees: Option<f64>,
    /// A multiple of the authored cadence, for the animation player.
    pub locomotion_rate: f64,
    pub interaction_enabled: bool,
    pub luminance_requests: Vec<LuminanceRequest>,
    pub diagnostics: Vec<(String, String)>,
    /// The pet came to rest somewhere worth remembering across launches.
    pub persist_position: bool,
}

/// What a click or a drag did.
#[derive(Debug, Clone)]
pub struct InteractionOutput {
    pub position: WorldPoint,
    pub capability: PetCapability,
    pub look_direction_degrees: Option<f64>,
    /// Absent leaves the panel's ownership alone.
    pub set_interaction_enabled: Option<bool>,
    pub render: bool,
    /// The click reaction needs a fast beat to play out; the caller reschedules.
    pub reschedule_after: Option<f64>,
    pub persist_position: bool,
}

pub struct PetRuntime {
    movement: MovementController,
    behavior: BehaviorController,
    pointer_model: PointerInteractionModel,
    placement: PlacementDirector,
    activity: ActivityDirector,
    tuning: RuntimeTuning,
    rng: Aimlessness,

    displays: Vec<DisplaySnapshot>,
    world: DesktopWorldSnapshot,
    luminance: Option<LuminanceField>,
    cached_focus: Option<FocusSnapshot>,
    focus_queried_at: f64,
    object_size: WorldSize,

    is_roaming_enabled: bool,
    is_pointer_avoidance_enabled: bool,
    are_interactions_enabled: bool,
    is_hidden: bool,

    last_tick_at: Option<f64>,
    next_wander_at: f64,
    approach_hold_until: f64,
    caught_animation_until: f64,
    click_reaction_until: f64,
    /// How long the pet is allowed to land for before it notices the cursor.
    ///
    /// The cursor is on top of the pet the instant it is dropped, because that
    /// is where it was put down -- so proximity there is a by-product of the
    /// gesture, not a signal. Without this the pointer input arrives on the
    /// very next tick and `Dropped` never survives one, which made the landing
    /// animation unreachable by construction: a 0.84 s state and an authored
    /// 0.50 s track that nobody had ever seen. `docs/behavior-flow.md` 7.
    landing_until: f64,
    is_click_reaction_pending: bool,
    is_dragging: bool,
    drag_offset: WorldVector,
    last_pointer_decision: Option<PointerDecision>,
    is_evade_transitioning: bool,
    /// The current walk is one the pet owes the user: it is standing on their
    /// work and leaving. Cleared as soon as the route is gone, however it went.
    escape_route_active: bool,
    /// Once at a shared edge, keep walking until the whole pet is inside.
    crossing_clear: Option<WorldPoint>,
    /// The affection key is held and the cursor is on the pet: see `TickInput`.
    is_petted: bool,
    rest_destination: Option<RestDestination>,
    rest_retry_at: f64,
    rest_content_check: Option<(WorldPoint, WorldSize, bool)>,
    caught_transition_duration: f64,
    dragged_cycle_duration: f64,

    diagnostics: Vec<(String, String)>,
    luminance_requests: Vec<LuminanceRequest>,
    persist_position: bool,
}

impl PetRuntime {
    pub fn new(position: WorldPoint, tuning: RuntimeTuning, seed: u64) -> Self {
        Self {
            movement: MovementController::new(
                position,
                WorldVector::ZERO,
                MovementConfiguration::new(tuning.walking_speed, 90.0, 115.0, 1.5),
            ),
            behavior: BehaviorController::default(),
            pointer_model: PointerInteractionModel::new(tuning.pointer_configuration()),
            placement: PlacementDirector::for_runtime(),
            activity: ActivityDirector::default(),
            tuning,
            rng: Aimlessness::new(seed),
            displays: Vec::new(),
            world: DesktopWorldSnapshot::new(Vec::new(), Vec::new()),
            luminance: None,
            cached_focus: None,
            focus_queried_at: f64::NEG_INFINITY,
            object_size: WorldSize::new(96.0, 104.0),
            is_roaming_enabled: true,
            is_pointer_avoidance_enabled: true,
            are_interactions_enabled: true,
            is_hidden: false,
            last_tick_at: None,
            next_wander_at: 0.0,
            approach_hold_until: 0.0,
            caught_animation_until: 0.0,
            click_reaction_until: 0.0,
            landing_until: 0.0,
            is_click_reaction_pending: false,
            is_dragging: false,
            drag_offset: WorldVector::ZERO,
            last_pointer_decision: None,
            is_evade_transitioning: false,
            escape_route_active: false,
            crossing_clear: None,
            is_petted: false,
            rest_destination: None,
            rest_retry_at: 0.0,
            rest_content_check: None,
            caught_transition_duration: 0.0,
            dragged_cycle_duration: 0.4,
            diagnostics: Vec::new(),
            luminance_requests: Vec::new(),
            persist_position: false,
        }
    }

    // ------------------------------------------------------------- accessors

    pub fn position(&self) -> WorldPoint {
        self.movement.position()
    }

    pub fn state(&self) -> BehaviorState {
        self.behavior.state()
    }

    pub fn is_placement_travelling(&self) -> bool {
        self.placement.is_travelling()
    }

    pub fn is_watching_window(&self) -> bool {
        self.activity.is_watching_window()
    }

    pub fn active_source_id(&self) -> Option<&str> {
        self.activity.active_source_id()
    }

    pub fn draws(&self) -> u64 {
        self.rng.draws()
    }

    pub fn tuning(&self) -> RuntimeTuning {
        self.tuning
    }

    /// How soon the caller should come back. A pet asleep is worth a beat every
    /// half second; one being reached for is worth every frame.
    pub fn preferred_tick_interval(&self, now: f64) -> f64 {
        if now <= self.approach_hold_until {
            return 1.0 / 60.0;
        }
        match self.behavior.state() {
            BehaviorState::Wander
            | BehaviorState::EvadePointer
            | BehaviorState::FindSleepSpot
            | BehaviorState::TravelToInterest => 1.0 / 60.0,
            BehaviorState::Caught
            | BehaviorState::Dragged
            | BehaviorState::Dropped
            | BehaviorState::Wake
            | BehaviorState::Stretch => 1.0 / 30.0,
            BehaviorState::LookAtPointer => 1.0 / 16.0,
            BehaviorState::Sleep => 1.0 / 2.0,
            _ => 1.0 / 12.0,
        }
    }

    // --------------------------------------------------------------- setters

    pub fn set_displays(&mut self, displays: Vec<DisplaySnapshot>) {
        if self.displays != displays {
            self.crossing_clear = None;
        }
        self.displays = displays.clone();
        self.world = DesktopWorldSnapshot::new(displays, Vec::new());
    }

    pub fn set_luminance(&mut self, field: Option<LuminanceField>) {
        self.luminance = field;
        self.rest_content_check = None;
    }

    pub fn set_object_size(&mut self, size: WorldSize) {
        self.object_size = size;
    }

    pub fn set_flags(&mut self, roaming: bool, avoidance: bool, interactions: bool) {
        self.is_roaming_enabled = roaming;
        self.is_pointer_avoidance_enabled = avoidance;
        self.are_interactions_enabled = interactions;
    }

    /// Turning roaming off stops the walk in progress; turning it back on
    /// gives the pet a moment before it sets off, so the menu item does not
    /// read as a launch button.
    pub fn set_roaming_enabled(&mut self, enabled: bool, now: f64) {
        let changed = self.is_roaming_enabled != enabled;
        self.is_roaming_enabled = enabled;
        if !enabled {
            self.movement.cancel_route(false);
            self.next_wander_at = f64::INFINITY;
        } else if changed {
            self.next_wander_at = now + 0.8;
        }
    }

    /// A pet mid-way through an escape across a display seam has to be let go
    /// of, or it finishes a journey the user just turned off.
    pub fn set_pointer_avoidance_enabled(&mut self, enabled: bool) {
        self.is_pointer_avoidance_enabled = enabled;
        if !enabled && self.is_evade_transitioning {
            self.is_evade_transitioning = false;
            self.movement.cancel_route(false);
        }
    }

    /// Returns whether the caller should release the panel: an approach
    /// reaction already under way must not survive the switch being turned off.
    pub fn set_interactions_enabled(&mut self, enabled: bool) -> bool {
        self.are_interactions_enabled = enabled;
        if !enabled {
            self.approach_hold_until = 0.0;
        }
        !enabled
    }

    /// Hiding is only a presentation switch: ticks, movement and activity
    /// sources stay alive. A hidden pet cannot keep or start an approach reaction, and it
    /// has no visible seat worth paying for a luminance capture to choose.
    pub fn set_hidden(&mut self, hidden: bool) {
        self.is_hidden = hidden;
        if hidden {
            self.approach_hold_until = 0.0;
            self.luminance_requests.clear();
        }
    }

    /// Frame timings the sprite sheet decides. The caught transition is capped
    /// because a pet held for a whole second reads as stuck.
    pub fn set_animation_durations(&mut self, caught: f64, dragged: f64) {
        self.caught_transition_duration = caught;
        self.dragged_cycle_duration = dragged;
    }

    pub fn set_position(&mut self, position: WorldPoint) {
        self.movement.teleport(position, false);
    }

    /// Drops a click reaction that is still playing. Shutting down and
    /// swapping the sprite sheet both do this; only the swap also drops the
    /// caught pose, because its length came from the sheet being replaced.
    pub fn clear_click_reaction(&mut self, clear_caught_transition: bool) {
        self.click_reaction_until = 0.0;
        self.is_click_reaction_pending = false;
        self.landing_until = 0.0;
        if clear_caught_transition {
            self.caught_animation_until = 0.0;
        }
    }

    pub fn set_next_wander_at(&mut self, timestamp: f64) {
        self.next_wander_at = timestamp;
    }

    /// The panel moved a slider. The pointer model forgets its previous sample,
    /// because a changed radius would otherwise be measured against a distance
    /// taken under the old one.
    pub fn apply_tuning(&mut self, proposed: RuntimeTuning, now: f64) {
        let pause_changed = (proposed.wander_pause - self.tuning.wander_pause).abs() > 0.001;
        self.tuning = proposed;
        self.pointer_model
            .set_configuration(proposed.pointer_configuration());
        self.pointer_model.reset();
        self.movement.set_maximum_speed(proposed.walking_speed);
        if pause_changed && !self.movement.has_route() && !self.behavior.state().is_held() {
            self.next_wander_at = now + proposed.wander_delay(0.5);
        }
    }

    /// The desk changed shape. The caller has already converted the pet's old
    /// position into the new coordinate space, because only it knows what the
    /// window server did.
    pub fn handle_display_change(
        &mut self,
        displays: Vec<DisplaySnapshot>,
        carried_position: WorldPoint,
        now: f64,
    ) -> WorldPoint {
        self.set_displays(displays);
        let clamped = self.world.clamp(carried_position, self.object_size);
        self.is_evade_transitioning = false;
        self.movement.cancel_route(true);
        self.movement.teleport(clamped, true);
        self.next_wander_at = now + 1.0;
        clamped
    }

    /// The panel resized the pet. Its footprint changed, so where it may stand
    /// changed with it.
    pub fn set_scale(&mut self, size: WorldSize) -> WorldPoint {
        self.object_size = size;
        let clamped = self.world.clamp(self.movement.position(), size);
        self.movement.teleport(clamped, false);
        clamped
    }

    // --------------------------------------------------------------- the tick

    /// Everything before the platform is asked anything. Returns whether an
    /// accessibility query is worth paying for: it is a synchronous round trip,
    /// so it only runs while there is a window whose caret would move the pet.
    pub fn begin_tick(&mut self, now: f64) -> bool {
        self.diagnostics.clear();
        self.luminance_requests.clear();
        self.persist_position = false;

        self.behavior.handle(BehaviorInput::Tick, now);
        let effects = self
            .activity
            .expire_states(self.behavior.state().is_resting(), now);
        self.apply_activity(effects, now);
        let effects = self
            .activity
            .expire_silent(self.behavior.state().is_resting(), now);
        self.apply_activity(effects, now);
        // The draw happens whether or not anything is pending, because the
        // Swift original evaluated it as an argument. Moving it inside the
        // branch would shift every draw after it.
        let roll = self.rng.unit();
        let effects = self.activity.resume_pending_if_ready(
            self.behavior.state() == BehaviorState::Idle,
            self.behavior.state().is_held(),
            self.behavior.state().is_resting(),
            roll,
            now,
        );
        self.apply_activity(effects, now);

        self.activity.is_watching_window() && now - self.focus_queried_at >= FOCUS_REFRESH_INTERVAL
    }

    pub fn finish_tick(&mut self, input: &TickInput) -> TickOutput {
        let now = input.now;
        let delta_time = swift_min(swift_max(now - self.last_tick_at.unwrap_or(now), 0.0), 0.1);
        self.last_tick_at = Some(now);

        if input.user_idle_duration < 0.8 && self.behavior.state().is_resting() {
            self.cancel_rest_for_activity(now);
        }
        if self.behavior.state().is_held() && !input.primary_button_down {
            if !self.is_click_reaction_pending || now >= self.click_reaction_until {
                self.finish_drop(now);
            }
        }

        if !self.movement.has_route() {
            self.escape_route_active = false;
            self.crossing_clear = None;
        }
        if self.crossing_clear.is_none() && !self.behavior.state().is_held() {
            if let Some((clear, route)) = crossing(
                &self.displays, self.movement.position(),
                self.movement.remaining_waypoints(), self.object_size,
            ) {
                self.crossing_clear = Some(clear);
                self.movement.set_route(route);
            }
        }

        let decision = self
            .pointer_model
            .evaluate(input.pointer, self.movement.position(), now);
        self.last_pointer_decision = Some(decision);
        // Under the affection key every close approach reads as the glance:
        // the pet stops and looks, and never evades. The catch below still
        // reads the real decision, so the hand can still pick it up.
        let is_adored = input.affection_held && decision.proximity != PointerProximity::Far;
        // Only a hand actually on the pet is petting it. Beside it, the pet
        // just watches as it would any cursor -- minus the stepping away.
        self.is_petted = is_adored && input.pointer_is_over_pet;
        let allows_pointer_glance = is_adored
            || (self.activity.sustained_reaction() != Some(CompanionReaction::Work)
                && matches!(self.behavior.state(),
                    BehaviorState::Idle | BehaviorState::Sit | BehaviorState::LookAtPointer)
                && !self.movement.has_route()
                && self.movement.velocity().length() <= 1.0);
        let proximity = if is_adored {
            PointerProximity::Watching
        } else if !allows_pointer_glance
            && matches!(decision.proximity, PointerProximity::Watching | PointerProximity::Catchable)
        {
            // A passing cursor gets a tail wag only from a seated pet. Work,
            // walks and one-shot reactions keep their motion; close evasion
            // and an actual click still use the unmodified pointer decision.
            PointerProximity::Far
        } else {
            decision.proximity
        };

        if !self.is_click_reaction_pending
            && decision.is_fast_approach()
            && self.are_interactions_enabled
            && !self.is_hidden
        {
            self.approach_hold_until =
                swift_max(self.approach_hold_until, now + self.tuning.approach_hold);
        }
        let approach_held = !self.is_click_reaction_pending
            && self.are_interactions_enabled
            && !self.is_hidden
            && now <= self.approach_hold_until;

        if !self.behavior.state().is_held() {
            // Gather, decide, apply. The decision runs every tick even when
            // something else owns the pet, so the seat verdict is never stale
            // by the time placement is allowed to act on it.
            let was_travelling = self.placement.is_travelling();
            let situation = self.make_situation(
                now, input, proximity, is_adored, approach_held && allows_pointer_glance,
            );
            let intent = if self.crossing_clear.is_some() {
                PlacementIntent::None
            } else {
                self.placement.decide(&situation)
            };
            // Arriving is an event, and this is where it happens: the director
            // stops travelling on the tick it decides the walk is over, whether
            // the pet reached the seat or the trip timed out. Guarded on still
            // watching something, because the director also drops a walk when
            // the agent goes quiet -- that is the pet being let go.
            let did_arrive = was_travelling
                && !self.placement.is_travelling()
                && self.activity.is_watching_window();
            let mut wore_arrival_reaction = false;

            self.record(
                "capture",
                if input.capture_authorized {
                    if self.luminance.is_none() {
                        "authorised, none yet"
                    } else {
                        "available"
                    }
                } else {
                    "not authorised"
                },
            );
            let state_name = state_name(self.behavior.state());
            self.record("pet", state_name);
            let described = describe(&intent);
            self.record("place", &described);
            let agent = match self.activity.active_source_id() {
                Some(id) => format!(
                    "{id} window={}",
                    if self.activity.hint().is_none() {
                        "none"
                    } else {
                        "found"
                    }
                ),
                None => "none".to_string(),
            };
            self.record("agent", &agent);

            if approach_held && self.crossing_clear.is_none() && allows_pointer_glance {
                self.is_evade_transitioning = false;
                self.movement.cancel_route(false);
                self.behavior
                    .handle(BehaviorInput::Pointer(PointerProximity::Catchable), now);
                self.movement.set_maximum_speed(self.tuning.walking_speed);
                self.movement.update_route(delta_time);
                self.next_wander_at = swift_max(self.next_wander_at, now + 1.0);
            } else if let Some(clear) = self.crossing_clear {
                // The approach reaction stays held, but a passing cursor
                // cannot park the panel astride two macOS displays.
                self.movement.update_route(delta_time);
                if !self.movement.remaining_waypoints().contains(&clear) {
                    self.crossing_clear = None;
                }
            } else if self.is_evade_transitioning {
                self.update_evade_transition(now, delta_time);
            } else if intent.travel_reason().is_some() && self.behavior.state().is_resting() {
                // Stepping out from under the user's text is the one thing that
                // outranks a nap, and the only reason placement may end one.
                self.cancel_rest_for_activity(now);
                wore_arrival_reaction = self.apply_intent(&intent, now, delta_time);
            } else if self.update_rest_lifecycle(
                input.user_idle_duration,
                // Suppressing a tail wag does not make the nearby hand absent
                // for waking up or deciding whether it is time to sleep.
                if is_adored { PointerProximity::Watching } else { decision.proximity },
                input.pointer,
                intent == PlacementIntent::SleepInPlace,
                now,
                delta_time,
            ) {
                // Rest owns movement until input wakes the creature.
            } else if now < self.landing_until {
                // Landing. The cursor is only where it is because the user put
                // the pet there, so the pet finishes the animation first.
                wore_arrival_reaction = self.apply_intent(&intent, now, delta_time);
            } else if is_adored
                || (self.is_pointer_avoidance_enabled
                    && !self.walk_outranks_glance(&intent, proximity))
            {
                self.behavior.handle(BehaviorInput::Pointer(proximity), now);
                match proximity {
                    PointerProximity::SlowEvade | PointerProximity::FastEvade => {
                        self.apply_evade(decision.escape_velocity, delta_time, now);
                    }
                    PointerProximity::Watching | PointerProximity::Catchable => {
                        self.movement.cancel_route(false);
                        self.movement.set_maximum_speed(self.tuning.walking_speed);
                        self.movement.update_route(delta_time);
                        self.next_wander_at = swift_max(self.next_wander_at, now + 0.8);
                    }
                    PointerProximity::Far => {
                        wore_arrival_reaction = self.apply_intent(&intent, now, delta_time);
                    }
                }
            } else {
                wore_arrival_reaction = self.apply_intent(&intent, now, delta_time);
            }

            // After the move, so a seat taken this tick is where the reaction is
            // worn. The arrival reaction falls back to `observe` when nothing is
            // owed, which is what ends the walk: a pet that has arrived is
            // watching, not still walking.
            //
            // Skipped when holding the seat already wore it this tick. Both run
            // on the tick a walk ends, and the second call finds nothing left
            // owed, so it fell back to whatever lasting condition the seat
            // carries: `observe` after `ActivityStarted`, which replaced the
            // greeting hop in the same tick it was played, and a silent
            // re-apply of `work` or `paw` after the other two. The visible loss
            // was the hop -- nobody ever saw a pet arrive and greet.
            if did_arrive {
                if !wore_arrival_reaction {
                    let effects = self
                        .activity
                        .deliver_arrival_reaction(self.behavior.state().is_resting(), now);
                    self.apply_activity(effects, now);
                }
                // Outside the guard: the pet has stopped somewhere new either
                // way, and that is what is worth writing down.
                self.persist_position = true;
            }
        }

        // A direct click is intent on its own. The fast-approach window still
        // drives anticipation, but must not gate the shell's mouse events.
        let catch_is_live = self.are_interactions_enabled
            && !self.is_click_reaction_pending && input.pointer_is_over_pet;
        let owns_pointer = self.behavior.state().is_held() && !self.is_click_reaction_pending;

        TickOutput {
            delta_time,
            position: self.movement.position(),
            state: self.behavior.state(),
            capability: self.current_capability(now),
            look_direction_degrees: if self.behavior.state() == BehaviorState::LookAtPointer {
                decision.look_direction_degrees
            } else {
                None
            },
            locomotion_rate: self.locomotion_rate(),
            interaction_enabled: !self.is_hidden && (owns_pointer || catch_is_live),
            luminance_requests: std::mem::take(&mut self.luminance_requests),
            diagnostics: std::mem::take(&mut self.diagnostics),
            persist_position: self.persist_position,
        }
    }

    // ------------------------------------------------------------ the pointer

    pub fn pointer_down(&mut self, pointer: WorldPoint, now: f64) -> InteractionOutput {
        self.touch_down(pointer, now)
    }

    /// Mouse clicks and touch both express direct intent without a preceding
    /// fast approach. The shell handles artwork hit testing; core also rejects
    /// stale or out-of-bounds contacts and duplicate presses while held.
    pub fn touch_down(&mut self, pointer: WorldPoint, now: f64) -> InteractionOutput {
        let position = self.movement.position();
        let inside = pointer.x >= position.x - self.object_size.width / 2.0
            && pointer.y >= position.y - self.object_size.height / 2.0
            && pointer.x <= position.x + self.object_size.width / 2.0
            && pointer.y <= position.y + self.object_size.height / 2.0;
        if self.is_hidden
            || !self.are_interactions_enabled
            || self.behavior.state().is_held()
            || !inside
        {
            return self.interaction(now, Some(false), false);
        }
        self.begin_catch(pointer, now)
    }

    fn begin_catch(&mut self, pointer: WorldPoint, now: f64) -> InteractionOutput {
        self.crossing_clear = None;
        self.drag_offset = self.movement.position().vector_from(pointer);
        self.click_reaction_until = 0.0;
        self.is_click_reaction_pending = false;
        self.is_dragging = false;
        self.is_evade_transitioning = false;
        self.movement.cancel_route(true);
        self.behavior.handle(BehaviorInput::CatchBegan, now);
        self.caught_animation_until = now + self.caught_transition_duration;
        let mut output = self.interaction(now, None, false);
        output.look_direction_degrees = self
            .last_pointer_decision
            .and_then(|decision| decision.look_direction_degrees);
        output
    }

    pub fn pointer_dragged(
        &mut self,
        pointer: WorldPoint,
        distance: f64,
        now: f64,
    ) -> InteractionOutput {
        if !self.behavior.state().is_held() {
            return self.interaction(now, None, false);
        }
        if distance > 4.0 {
            self.is_dragging = true;
            self.behavior.handle(BehaviorInput::DragMoved, now);
        }
        self.movement
            .teleport(pointer.offset(self.drag_offset), true);
        self.interaction(now, None, true)
    }

    pub fn pointer_up(
        &mut self,
        pointer: WorldPoint,
        was_dragged: bool,
        now: f64,
    ) -> InteractionOutput {
        if !self.behavior.state().is_held() {
            return self.interaction(now, Some(false), false);
        }
        if was_dragged || self.is_dragging {
            self.movement
                .teleport(pointer.offset(self.drag_offset), true);
            self.finish_drop(now);
            let mut output = self.interaction(now, Some(false), true);
            output.persist_position = true;
            return output;
        }

        // A click has the same caught -> four-paw scramble response as a drag.
        // Release panel ownership immediately so the reaction never blocks the
        // underlying app, then finish with the normal landing after one loop.
        self.behavior.handle(BehaviorInput::DragMoved, now);
        self.is_click_reaction_pending = true;
        self.click_reaction_until =
            swift_max(now, self.caught_animation_until) + self.dragged_cycle_duration;
        let clamped = self.world.clamp(self.movement.position(), self.object_size);
        self.movement.teleport(clamped, true);
        self.approach_hold_until = 0.0;
        let mut output = self.interaction(now, Some(false), true);
        output.reschedule_after = Some(1.0 / 30.0);
        output
    }

    /// The agent adapters deliver on their own schedule, so this is its own
    /// entry point rather than part of the tick.
    pub fn handle_activity_event(
        &mut self,
        event: CompanionEvent,
        now: f64,
    ) -> Vec<LuminanceRequest> {
        self.luminance_requests.clear();
        let roll = self.rng.unit();
        let effects = self.activity.handle_event(
            event,
            self.behavior.state().is_held(),
            self.behavior.state().is_resting(),
            roll,
            now,
        );
        self.apply_activity(effects, now);
        std::mem::take(&mut self.luminance_requests)
    }

    /// A state source declares its current fact independently of the event
    /// path. The director owns rest deferral, source precedence and placement.
    pub fn declare_state(
        &mut self,
        declaration: StateDeclaration,
        now: f64,
    ) -> Vec<LuminanceRequest> {
        self.luminance_requests.clear();
        let effects =
            self.activity
                .declare_state(declaration, self.behavior.state().is_resting(), now);
        self.apply_activity(effects, now);
        std::mem::take(&mut self.luminance_requests)
    }

    // -------------------------------------------------------------- internals

    fn interaction(
        &mut self,
        now: f64,
        set_interaction_enabled: Option<bool>,
        render: bool,
    ) -> InteractionOutput {
        InteractionOutput {
            position: self.movement.position(),
            capability: self.current_capability(now),
            look_direction_degrees: None,
            set_interaction_enabled,
            render,
            reschedule_after: None,
            persist_position: false,
        }
    }

    fn current_capability(&self, now: f64) -> PetCapability {
        capability_for(
            self.behavior.state(),
            self.movement.velocity().dx,
            now < self.caught_animation_until,
            self.is_petted,
        )
    }

    /// Only the states that actually travel at the tuned walking speed scale
    /// their cadence. Watching the pointer scales too, but on distance rather
    /// than on tuning: the closer the pointer, the faster the pet's tail goes.
    fn locomotion_rate(&self) -> f64 {
        match self.behavior.state() {
            BehaviorState::Wander
            | BehaviorState::FindSleepSpot
            | BehaviorState::TravelToInterest => self.tuning.locomotion_animation_rate(),
            BehaviorState::LookAtPointer => self
                .last_pointer_decision
                .map_or(1.0, |decision| decision.attention_rate),
            _ => 1.0,
        }
    }

    fn record(&mut self, category: &str, message: &str) {
        self.diagnostics
            .push((category.to_string(), message.to_string()));
    }

    fn apply_activity(&mut self, effects: Vec<ActivityEffect>, now: f64) {
        for effect in effects {
            match effect {
                ActivityEffect::CancelRest => self.cancel_rest_for_activity(now),
                ActivityEffect::SettleInPlace { source_id } => {
                    self.placement.settle_in_place(Some(&source_id), now)
                }
                ActivityEffect::CancelRoute => {
                    if self.crossing_clear.is_none() {
                        self.movement.cancel_route(false);
                    }
                }
                ActivityEffect::SetNextWanderAt { timestamp } => self.next_wander_at = timestamp,
                ActivityEffect::ApplyReaction { reaction } => {
                    self.behavior.handle(BehaviorInput::Reaction(reaction), now);
                }
                ActivityEffect::RequestLuminance { region } => {
                    self.request_luminance(region, LUMINANCE_REFRESH_INTERVAL)
                }
            }
        }
    }

    fn request_luminance(&mut self, region: WorldRect, requested: f64) {
        if self.is_hidden {
            return;
        }
        let interval = if self.behavior.state().is_resting() {
            swift_max(requested, RESTING_LUMINANCE_REFRESH_INTERVAL)
        } else {
            requested
        };
        let centre = region.center();
        if self.world.display_containing(centre).is_none()
            && self.world.nearest_display(centre).is_none()
        {
            return;
        }
        self.luminance_requests
            .push(LuminanceRequest { region, interval });
    }

    /// Whether the walk under way outranks the cursor on this tick. Stopping
    /// to look at the cursor is a moment; standing on the user's work is a
    /// condition, and a moment must not cancel the remedy for a condition.
    /// Which walks qualify is `PlacementIntent::outranks_glance`; the director
    /// carries a travel intent every tick, so only roaming's one-shot escape
    /// needs the flag below to survive past the tick it was issued.
    fn walk_outranks_glance(&self, intent: &PlacementIntent, proximity: PointerProximity) -> bool {
        if proximity != PointerProximity::Watching {
            return false;
        }
        if intent.outranks_glance() {
            return true;
        }
        self.escape_route_active && self.movement.has_route()
    }

    /// Collects everything the placement decision is allowed to look at.
    /// Nothing here decides anything.
    fn make_situation(
        &mut self,
        now: f64,
        input: &TickInput,
        proximity: PointerProximity,
        is_adored: bool,
        approach_held: bool,
    ) -> PetSituation {
        let pointer = input.pointer;
        let is_watching = self.activity.is_watching_window();
        let is_roaming = self.is_roaming_enabled && !is_watching
            && self.activity.sustained_reaction() != Some(CompanionReaction::Work);
        let is_stroll_due = is_roaming && !self.movement.has_route() && now >= self.next_wander_at;

        if is_watching {
            if let Some(region) = self
                .activity
                .hint()
                .and_then(|hint| hint.approximate_region)
            {
                self.request_luminance(region, LUMINANCE_REFRESH_INTERVAL);
            }
        } else if !self.movement.has_route() {
            self.request_luminance_for_roaming();
        }

        let candidates = if is_roaming && !self.movement.has_route() {
            self.stroll_candidates()
        } else {
            Vec::new()
        };

        // The accessibility query costs a synchronous round trip, so it only
        // runs while there is a window whose caret the answer would move the
        // pet away from. Revoking the permission has to take effect on the next
        // event rather than leaving a stale caret behind, and a query that came
        // back empty still resets the clock.
        let focus = if !is_watching {
            None
        } else if !input.focus_authorized {
            self.cached_focus = None;
            None
        } else if input.did_query_focus {
            self.focus_queried_at = now;
            self.cached_focus = input.queried_focus.clone();
            self.cached_focus.clone()
        } else {
            self.cached_focus.clone()
        };

        let mut world = placement_world(&self.world);
        world.focus = focus;
        world.luminance = self.luminance.clone();

        PetSituation {
            timestamp: now,
            world,
            position: self.movement.position(),
            object_size: self.object_size,
            pointer_position: Some(pointer),
            pointer_clearance: if self.is_pointer_avoidance_enabled {
                self.tuning.pointer_awareness_distance
            } else {
                0.0
            },
            walking_speed: self.tuning.walking_speed,
            // Being petted owns the pet outright, and is not a glance: a
            // glance is something the pet tires of, and this is not.
            is_pointer_owned: approach_held
                || is_adored
                || matches!(
                    self.behavior.state(),
                    BehaviorState::Caught | BehaviorState::Dragged | BehaviorState::EvadePointer
                )
                || (self.is_pointer_avoidance_enabled
                    && proximity != PointerProximity::Far
                    && proximity != PointerProximity::Watching),
            is_pointer_watching: !is_adored
                && self.is_pointer_avoidance_enabled
                && (proximity == PointerProximity::Watching
                    || self.behavior.state() == BehaviorState::LookAtPointer),
            is_evading: self.is_evade_transitioning,
            is_walking: self.movement.has_route(),
            is_resting: self.behavior.state().is_resting(),
            activity_source_id: self.activity.active_source_id().map(str::to_owned),
            activity_hint: self.activity.hint().cloned(),
            user_idle_duration: input.user_idle_duration,
            idle_before_rest: self.tuning.idle_before_rest,
            is_roaming_enabled: self.is_roaming_enabled,
            is_stroll_due,
            stroll_candidates: candidates,
        }
    }

    fn request_luminance_for_roaming(&mut self) {
        let position = self.movement.position();
        let frame = self
            .world
            .display_containing(position)
            .or_else(|| self.world.nearest_display(position))
            .map(|display| display.visible_frame);
        if let Some(frame) = frame {
            self.request_luminance(frame, ROAMING_LUMINANCE_REFRESH_INTERVAL);
        }
    }

    /// Carries out the director's decision. Nothing here re-decides.
    ///
    /// Returns whether it spent the arrival reaction, because the caller has a
    /// second chance to deliver one and delivering twice is how a greeting hop
    /// gets replaced by the `observe` fallback on the tick it was played.
    fn apply_intent(&mut self, intent: &PlacementIntent, now: f64, delta_time: f64) -> bool {
        match intent {
            PlacementIntent::Travel(destination, _) => {
                let point = destination.point;
                self.travel_to_seat(point, now, delta_time)
            }
            PlacementIntent::Stroll(point) => {
                self.begin_stroll(*point, now, delta_time);
                false
            }
            PlacementIntent::Escape(point) => {
                self.begin_stroll(*point, now, delta_time);
                self.escape_route_active = self.movement.has_route();
                false
            }
            // `SleepInPlace` lands here when rest declined to start -- the
            // pointer came close, or the state machine was mid-transition. The
            // seat is kept either way.
            PlacementIntent::Hold | PlacementIntent::SleepInPlace | PlacementIntent::None => {
                if self.activity.is_watching_window()
                    || self.activity.sustained_reaction() == Some(CompanionReaction::Work)
                {
                    self.hold_seat(now, delta_time)
                } else {
                    self.update_roaming(now, delta_time);
                    false
                }
            }
        }
    }

    fn travel_to_seat(&mut self, destination: WorldPoint, now: f64, delta_time: f64) -> bool {
        if !self.movement.has_route() || self.movement.destination() != Some(destination) {
            let route = DisplayTopology::new(self.displays.clone())
                .route(self.movement.position(), destination);
            if route.waypoints.is_empty() {
                // Nowhere to walk is not a reason to keep trying. The pet
                // watches from where it stands and the seat is judged there.
                let owner = self.activity.active_source_id().map(str::to_owned);
                self.placement.settle_in_place(owner.as_deref(), now);
                return self.hold_seat(now, delta_time);
            }
            self.movement.set_route(route.waypoints);
        }
        self.is_evade_transitioning = false;
        self.rest_destination = None;
        self.behavior
            .handle(BehaviorInput::BeginInterestTravel, now);
        self.movement.set_maximum_speed(self.tuning.walking_speed);
        self.next_wander_at = f64::INFINITY;
        self.movement.update_route(delta_time);
        false
    }

    /// A parked pet keeps its seat. All that is left is wearing the reaction
    /// the current event asked for. Returns whether it wore one.
    fn hold_seat(&mut self, now: f64, delta_time: f64) -> bool {
        self.movement.cancel_route(false);
        self.movement.set_maximum_speed(self.tuning.walking_speed);
        self.movement.update_route(delta_time);
        if self.activity.has_arrival_reaction() {
            let effects = self
                .activity
                .deliver_arrival_reaction(self.behavior.state().is_resting(), now);
            self.apply_activity(effects, now);
            return true;
        }
        match self.behavior.state() {
            BehaviorState::Observe
            | BehaviorState::Work
            | BehaviorState::WaitingForUser
            | BehaviorState::Celebrate
            | BehaviorState::Sad
            | BehaviorState::Spark
            | BehaviorState::Wake
            | BehaviorState::Stretch
            | BehaviorState::Caught
            | BehaviorState::Dragged => {}
            _ => {
                // Only a lasting condition is worn continuously. A moment is
                // delivered once and then the pet is simply present.
                let effects = self
                    .activity
                    .sustain_on_seat(self.behavior.state().is_resting(), now);
                self.apply_activity(effects, now);
            }
        }
        false
    }

    fn update_rest_lifecycle(
        &mut self,
        user_idle_duration: f64,
        proximity: PointerProximity,
        pointer: WorldPoint,
        may_nap_on_seat: bool,
        now: f64,
        delta_time: f64,
    ) -> bool {
        if self.behavior.state().is_resting() && proximity != PointerProximity::Far {
            self.cancel_rest_for_activity(now);
            return false;
        }

        match self.behavior.state() {
            BehaviorState::Sit => {
                self.movement.cancel_route(false);
                self.movement.update_route(delta_time);
                if now - self.behavior.entered_at() >= SITTING_DURATION {
                    self.behavior.handle(BehaviorInput::SeekSleepSpot, now);
                    self.begin_rest_travel(pointer, may_nap_on_seat, now);
                }
                return true;
            }
            BehaviorState::FindSleepSpot => {
                self.movement
                    .set_maximum_speed(swift_max(24.0, self.tuning.walking_speed * 0.75));
                if self.movement.has_route() {
                    if self.movement.update_route(delta_time).reached_destination {
                        self.enter_sleep(now);
                    }
                } else {
                    self.enter_sleep(now);
                }
                return true;
            }
            BehaviorState::Sleep => {
                if self.rest_spot_is_busy() {
                    self.defer_rest(now);
                    return true;
                }
                self.movement.cancel_route(false);
                self.movement.update_route(delta_time);
                return true;
            }
            _ => {}
        }

        // Watching an agent used to block rest outright, which meant the pet
        // could never sleep during the long unattended run that is exactly when
        // nobody is looking at it.
        let blocked = if user_idle_duration < self.tuning.idle_before_rest {
            "waiting for user idle".to_string()
        } else if self.placement.is_travelling() {
            "travelling".to_string()
        } else if self.activity.is_watching_window() && !may_nap_on_seat {
            "on duty, seat not nappable".to_string()
        } else if proximity != PointerProximity::Far {
            format!("pointer {}", proximity_name(proximity))
        } else if !self.behavior.state().allows_rest_entry() {
            format!("state {}", state_name(self.behavior.state()))
        } else {
            "clear to rest".to_string()
        };
        self.record("rest", &blocked);

        if !(user_idle_duration >= self.tuning.idle_before_rest
            && now >= self.rest_retry_at
            && !self.placement.is_travelling()
            && (!self.activity.is_watching_window() || may_nap_on_seat)
            && proximity == PointerProximity::Far
            && self.behavior.state().allows_rest_entry())
        {
            return false;
        }
        self.is_evade_transitioning = false;
        self.rest_destination = None;
        self.movement.cancel_route(false);
        self.behavior.handle(BehaviorInput::BeginRest, now);
        self.next_wander_at = f64::INFINITY;
        self.movement.update_route(delta_time);
        true
    }

    fn begin_rest_travel(&mut self, pointer: WorldPoint, nap_in_place: bool, now: f64) {
        let away_from_seam = self.world.display_containing(self.movement.position())
            .is_some_and(|display| placement_frame(display, &self.displays)
                .inset_by(self.object_size.width / 2.0, self.object_size.height / 2.0)
                .contains(self.movement.position()));
        // Without a field, retain the permission-free behaviour. A measured
        // field, however, judges every sleep spot including an agent's seat.
        if nap_in_place && away_from_seam && self.luminance.is_none() {
            self.record("rest", "sleeping in place, on a vetted seat");
            self.enter_sleep(now);
            return;
        }
        if self.luminance.is_none() {
            self.record("rest", "tucking into a safe zone, spot unvetted");
        }

        let mut rest_world = placement_world(&self.world);
        rest_world.safe_zones = BasicSafeZonePlanner::safe_zones(&rest_world);
        rest_world.focus = self.world.focus.clone();
        rest_world.luminance = self.luminance.clone();
        self.rest_destination = BasicSafeZonePlanner::clear_destination(
            &rest_world,
            self.movement.position(),
            Some(pointer),
            self.object_size,
        );

        if away_from_seam {
            if let Some(field) = self.luminance.as_ref() {
                let map = crate::clearance::ClearanceMap::new(field);
                let position = self.movement.position();
                if map.distance(position, self.object_size).is_some_and(|distance| distance >= 0.0)
                    && !self.rest_destination.as_ref().is_some_and(|destination| {
                        map.improves(position, destination.point, self.object_size)
                    })
                {
                    self.enter_sleep(now);
                    return;
                }
            }
        }

        let Some(destination) = self.rest_destination.clone() else {
            self.defer_rest(now);
            return;
        };
        if !self.is_roaming_enabled {
            self.enter_sleep(now);
            return;
        }
        let route = DisplayTopology::new(self.displays.clone())
            .route(self.movement.position(), destination.point);
        self.movement
            .set_maximum_speed(swift_max(24.0, self.tuning.walking_speed * 0.75));
        self.movement.set_route(route.waypoints);
        if !self.movement.has_route() {
            self.enter_sleep(now);
        }
    }

    fn enter_sleep(&mut self, now: f64) {
        if self.rest_spot_is_busy() {
            self.defer_rest(now);
            return;
        }
        self.movement.cancel_route(true);
        self.behavior.handle(BehaviorInput::SleepSpotReached, now);
        self.next_wander_at = f64::INFINITY;
        self.persist_position = true;
    }

    fn rest_spot_is_busy(&mut self) -> bool {
        let position = self.movement.position();
        if let Some((point, size, busy)) = self.rest_content_check {
            if point == position && size == self.object_size {
                return busy;
            }
        }
        // Sleeping has no movement: only a new field, position or size needs
        // another content check, not every animation tick.
        let busy = self.luminance.as_ref().is_some_and(|field| {
            crate::clearance::ClearanceMap::new(field)
                .distance(position, self.object_size)
                .is_some_and(|distance| distance < 0.0)
        });
        self.rest_content_check = Some((position, self.object_size, busy));
        busy
    }

    fn defer_rest(&mut self, now: f64) {
        self.movement.cancel_route(true);
        self.rest_destination = None;
        self.behavior.handle(BehaviorInput::Reaction(CompanionReaction::Calm), now);
        self.rest_retry_at = now + 30.0;
        self.next_wander_at = now + WAKE_WANDER_DELAY;
        self.record("rest", "no clear sleep spot, staying awake");
    }

    fn cancel_rest_for_activity(&mut self, now: f64) {
        if !self.behavior.state().is_resting() {
            return;
        }
        self.rest_destination = None;
        self.movement.cancel_route(false);
        self.behavior.handle(BehaviorInput::MeaningfulActivity, now);
        self.next_wander_at = now + WAKE_WANDER_DELAY;
    }

    /// Walks whatever route roaming already has and paces the next pause.
    /// Choosing where to stroll is the director's job, not this one.
    fn update_roaming(&mut self, now: f64, delta_time: f64) {
        self.movement.set_maximum_speed(self.tuning.walking_speed);
        if !self.is_roaming_enabled {
            self.movement.cancel_route(false);
            self.movement.update_route(delta_time);
            return;
        }
        if !self.movement.has_route() {
            self.movement.update_route(delta_time);
            return;
        }
        if self.movement.update_route(delta_time).reached_destination {
            self.behavior.handle(BehaviorInput::Arrived, now);
            let roll = self.rng.unit();
            self.next_wander_at = now + self.tuning.wander_delay(roll);
            self.persist_position = true;
        }
    }

    fn begin_stroll(&mut self, point: WorldPoint, now: f64, delta_time: f64) {
        self.is_evade_transitioning = false;
        let route =
            DisplayTopology::new(self.displays.clone()).route(self.movement.position(), point);
        self.movement.set_maximum_speed(self.tuning.walking_speed);
        self.movement.set_route(route.waypoints);

        // A destination the pet is already standing on leaves nothing to walk:
        // `set_route` hands the single waypoint straight to its arrival check
        // and it is consumed. Claiming the walking state before finding that
        // out is what put the pet in the walk frames going nowhere, once every
        // two seconds until a different point came up -- roughly six seconds of
        // running on the spot in the session this was found in.
        if !self.movement.has_route() {
            self.next_wander_at = now + 2.0;
            self.movement.update_route(delta_time);
            return;
        }

        // Only now is the state claimed. The route is laid first because the
        // question "is there anywhere to go" cannot be answered without it, and
        // it is cancelled again if the state machine refuses -- otherwise the
        // pet walks the whole leg wearing the idle frames, which is the fault
        // this ordering was reversed to fix in the first place.
        if self.behavior.handle(BehaviorInput::BeginWander, now).to != BehaviorState::Wander {
            self.movement.cancel_route(false);
            self.next_wander_at = now + 2.0;
            self.movement.update_route(delta_time);
            return;
        }
        self.movement.update_route(delta_time);
    }

    /// Offering the director a handful of destinations to reject is the
    /// cheapest way to keep an aimless walk off the user's text without making
    /// roaming look calculated.
    fn stroll_candidates(&mut self) -> Vec<WorldPoint> {
        let position = self.movement.position();
        let radius = self.movement.configuration().arrival_radius;
        // Somewhere the pet already stands is not a destination. It comes up
        // because `random_wander_point` ends in a clamp: every target past the
        // safe area folds onto the same edge, so a pet parked on that edge is
        // handed its own position back. Filtered after the draw, not instead of
        // it -- the sampler's random draws stay in the same order.
        (0..WANDER_CANDIDATE_COUNT)
            .filter_map(|_| self.random_wander_point())
            .filter(|candidate| position.distance(*candidate) > radius)
            .collect()
    }

    fn random_wander_point(&mut self) -> Option<WorldPoint> {
        if self.displays.is_empty() {
            return None;
        }
        let position = self.movement.position();
        let current_id = self
            .world
            .display_containing(position)
            .or_else(|| self.world.nearest_display(position))
            .map(|display| display.id.clone());
        // Short-circuits exactly as Swift's `&&` does: a single display draws
        // nothing at all, and a draw taken here would shift every draw after it.
        let should_explore =
            self.displays.len() > 1 && self.rng.unit() < self.tuning.cross_display_wander_chance;
        let target = if should_explore {
            let alternatives: Vec<DisplaySnapshot> = self
                .displays
                .iter()
                .filter(|display| Some(&display.id) != current_id.as_ref())
                .cloned()
                .collect();
            if alternatives.is_empty() {
                self.displays[0].clone()
            } else {
                let index = self.rng.index(alternatives.len());
                alternatives[index].clone()
            }
        } else {
            match current_id
                .as_ref()
                .and_then(|id| self.displays.iter().find(|display| &display.id == id))
            {
                Some(display) => display.clone(),
                None => {
                    let index = self.rng.index(self.displays.len());
                    self.displays[index].clone()
                }
            }
        };

        let safe = placement_frame(&target, &self.displays).inset_by(
            self.object_size.width / 2.0 + 18.0,
            self.object_size.height / 2.0 + 12.0,
        );
        if safe.is_empty() {
            return Some(target.visible_frame.center());
        }

        // A cross-display trip ends shortly inside the destination display.
        // Crossing the seam reads clearly, while avoiding another full-screen
        // trek before Roamling finally pauses.
        if Some(&target.id) != current_id.as_ref() {
            let boundary = target.visible_frame.closest_point(position);
            let inward = target
                .visible_frame
                .center()
                .vector_from(boundary)
                .normalized();
            let depth = 140.0 + self.rng.unit() * 220.0;
            return Some(safe.closest_point(boundary.offset(inward.scaled(depth))));
        }

        let x = safe.min_x() + self.rng.unit() * safe.size.width;
        let y = if self.rng.unit() < 0.72 {
            let upper = swift_max(
                safe.min_y(),
                safe.max_y() - swift_min(170.0, safe.size.height * 0.32),
            );
            upper + self.rng.unit() * (safe.max_y() - upper)
        } else {
            safe.min_y() + self.rng.unit() * safe.size.height
        };
        let sampled = WorldPoint::new(x, y);
        let offset = sampled.vector_from(position);
        if !(offset.length() > 520.0) {
            return Some(sampled);
        }
        let leg = 280.0 + self.rng.unit() * 240.0;
        Some(safe.closest_point(position.offset(offset.normalized().scaled(leg))))
    }

    fn apply_evade(&mut self, desired_velocity: WorldVector, delta_time: f64, now: f64) {
        let topology = DisplayTopology::new(self.displays.clone());
        if let Some(mut transition) = topology.evade_transition(
            self.movement.position(),
            desired_velocity,
            self.object_size,
            320.0,
            1.0,
        ) {
            if let Some(target) = transition.display_ids.last()
                .and_then(|id| self.displays.iter().find(|display| &display.id == id))
            {
                if let Some(destination) = transition.waypoints.last_mut() {
                    *destination = placement_frame(target, &self.displays)
                        .clamped_center(*destination, self.object_size);
                }
            }
            self.is_evade_transitioning = true;
            self.movement.set_maximum_speed(swift_max(
                self.tuning.walking_speed,
                desired_velocity.length(),
            ));
            self.movement.set_route(transition.waypoints);
            self.movement.update_route(delta_time);
            self.next_wander_at = now + 1.5;
            return;
        }

        self.movement.cancel_route(false);
        let mut velocity = desired_velocity;
        let position = self.movement.position();
        let current = self
            .world
            .display_containing(position)
            .or_else(|| self.world.nearest_display(position))
            .map(|display| display.visible_frame);
        if let Some(frame) = current {
            let safe = frame.inset_by(self.object_size.width / 2.0, self.object_size.height / 2.0);
            let proposed = position.offset(velocity.scaled(delta_time));
            if proposed.x < safe.min_x() || proposed.x > safe.max_x() {
                velocity.dx = 0.0;
            }
            if proposed.y < safe.min_y() || proposed.y > safe.max_y() {
                velocity.dy = 0.0;
            }
            if velocity.length() < 1.0 {
                // Pinned against an edge: go along it, away from where the
                // cursor is heading rather than into it.
                let pointer_y = self
                    .last_pointer_decision
                    .map_or(0.0, |decision| decision.kinematics.velocity.dy);
                velocity = WorldVector::new(
                    0.0,
                    if pointer_y >= 0.0 {
                        -desired_velocity.length()
                    } else {
                        desired_velocity.length()
                    },
                );
            }
            let constrained = safe.closest_point(position.offset(velocity.scaled(delta_time)));
            self.movement.teleport(constrained, false);
        } else {
            self.movement
                .teleport(position.offset(velocity.scaled(delta_time)), false);
        }
        self.movement.set_maximum_speed(swift_max(
            self.tuning.walking_speed,
            desired_velocity.length(),
        ));
        self.movement.set_velocity(velocity);
        self.next_wander_at = now + 1.0;
    }

    fn update_evade_transition(&mut self, now: f64, delta_time: f64) {
        if !self.movement.has_route() {
            self.is_evade_transitioning = false;
            self.behavior
                .handle(BehaviorInput::Pointer(PointerProximity::Far), now);
            return;
        }
        self.movement.set_maximum_speed(swift_max(
            self.tuning.walking_speed,
            self.tuning.pointer_configuration().fast_evade_speed,
        ));
        if !self.movement.update_route(delta_time).reached_destination {
            return;
        }
        self.is_evade_transitioning = false;
        self.behavior
            .handle(BehaviorInput::Pointer(PointerProximity::Far), now);
        let roll = self.rng.unit();
        self.next_wander_at = now + swift_max(1.5, self.tuning.wander_delay(roll) * 0.35);
        self.persist_position = true;
    }

    fn finish_drop(&mut self, now: f64) {
        self.is_dragging = false;
        self.is_click_reaction_pending = false;
        self.is_evade_transitioning = false;
        self.caught_animation_until = 0.0;
        self.click_reaction_until = 0.0;
        self.behavior.handle(BehaviorInput::MouseReleased, now);
        let clamped = self.world.clamp(self.movement.position(), self.object_size);
        self.movement.teleport(clamped, true);
        self.approach_hold_until = 0.0;
        // The whole of `Dropped`, so the pet lands and then notices the user a
        // beat later. Reusing the state's own length keeps this from becoming
        // a second number that has to be kept in step with it.
        self.landing_until = now + crate::behavior::timing::DROPPED;
        self.next_wander_at = now + 1.4;
        self.persist_position = true;
    }
}

/// The diagnostics vocabulary, which is the `rawValue` of the Swift enums. It
/// is user-visible through Copy Diagnostics, so it is spelled rather than
/// derived.
fn state_name(state: BehaviorState) -> &'static str {
    match state {
        BehaviorState::Idle => "idle",
        BehaviorState::Wander => "wander",
        BehaviorState::LookAtPointer => "lookAtPointer",
        BehaviorState::EvadePointer => "evadePointer",
        BehaviorState::Caught => "caught",
        BehaviorState::Dragged => "dragged",
        BehaviorState::Dropped => "dropped",
        BehaviorState::Sit => "sit",
        BehaviorState::FindSleepSpot => "findSleepSpot",
        BehaviorState::Sleep => "sleep",
        BehaviorState::Wake => "wake",
        BehaviorState::Stretch => "stretch",
        BehaviorState::TravelToInterest => "travelToInterest",
        BehaviorState::Observe => "observe",
        BehaviorState::Spark => "spark",
        BehaviorState::Work => "work",
        BehaviorState::WaitingForUser => "waitingForUser",
        BehaviorState::Celebrate => "celebrate",
        BehaviorState::Sad => "sad",
    }
}

fn proximity_name(proximity: PointerProximity) -> &'static str {
    match proximity {
        PointerProximity::Far => "far",
        PointerProximity::Watching => "watching",
        PointerProximity::SlowEvade => "slowEvade",
        PointerProximity::FastEvade => "fastEvade",
        PointerProximity::Catchable => "catchable",
    }
}

fn reason_name(reason: PlacementTravelReason) -> &'static str {
    match reason {
        PlacementTravelReason::NewActivity => "newActivity",
        PlacementTravelReason::CoveringCaret => "coveringCaret",
        PlacementTravelReason::SeatUnderPointer => "seatUnderPointer",
        PlacementTravelReason::CoveringWork => "coveringWork",
        PlacementTravelReason::PlannedBlind => "plannedBlind",
        PlacementTravelReason::FollowedFocus => "followedFocus",
    }
}

fn describe(intent: &PlacementIntent) -> String {
    match intent {
        PlacementIntent::None => "none, something else owns the pet".to_string(),
        PlacementIntent::Hold => "hold".to_string(),
        PlacementIntent::SleepInPlace => "sleep in place".to_string(),
        PlacementIntent::Stroll(point) => {
            format!("stroll to {:.0},{:.0}", point.x, point.y)
        }
        PlacementIntent::Escape(point) => {
            format!("escape to {:.0},{:.0}", point.x, point.y)
        }
        PlacementIntent::Travel(destination, reason) => format!(
            "travel {} to {:.0},{:.0}",
            reason_name(*reason),
            destination.point.x,
            destination.point.y
        ),
    }
}

/// Unused today: reactions cross as indices and the runtime never names one.
/// Kept so the vocabulary lives in one place when the shell starts showing it.
#[allow(dead_code)]
fn reaction_name(reaction: CompanionReaction) -> &'static str {
    match reaction {
        CompanionReaction::Glance => "glance",
        CompanionReaction::Observe => "observe",
        CompanionReaction::Spark => "spark",
        CompanionReaction::Work => "work",
        CompanionReaction::Paw => "paw",
        CompanionReaction::SmallCelebrate => "smallCelebrate",
        CompanionReaction::LargeCelebrate => "largeCelebrate",
        CompanionReaction::Sad => "sad",
        CompanionReaction::Calm => "calm",
    }
}

/// `LocationHint` is carried by value into the situation, so this is only here
/// to keep the import honest when the hint is absent.
#[allow(dead_code)]
fn empty_hint() -> LocationHint {
    LocationHint::new(None, 0.0)
}

#[cfg(test)]
mod movement_policy_tests {
    use super::*;
    use crate::activity::CompanionEventKind;

    fn display(id: &str, x: f64, y: f64) -> DisplaySnapshot {
        DisplaySnapshot {
            id: id.into(),
            name: id.into(),
            frame: WorldRect::new(x, y, 1000.0, 800.0),
            visible_frame: WorldRect::new(x, y, 1000.0, 800.0),
            scale: 1.0,
        }
    }

    fn input(now: f64, pointer: WorldPoint) -> TickInput {
        TickInput {
            now,
            pointer,
            primary_button_down: false,
            user_idle_duration: 0.0,
            capture_authorized: false,
            focus_authorized: false,
            did_query_focus: false,
            queried_focus: None,
            pointer_is_over_pet: false,
            affection_held: false,
        }
    }

    #[test]
    fn a_pointer_kept_on_the_pet_prevents_rest_despite_idle_desktop() {
        let mut pet = PetRuntime::new(WorldPoint::new(500.0, 400.0), RuntimeTuning::default(), 7);
        pet.set_displays(vec![display("main", 0.0, 0.0)]);
        for tick in 1..=600 {
            let now = 1000.0 + f64::from(tick) / 30.0;
            pet.begin_tick(now);
            let mut sample = input(now, pet.position());
            sample.pointer_is_over_pet = true;
            sample.user_idle_duration = 600.0;
            let output = pet.finish_tick(&sample);
            assert!(!output.state.is_resting(), "resting with a pointer on the pet: {:?}", output.state);
        }
    }

    #[test]
    fn body_click_catches_walk_and_work_without_an_approach_then_drags_and_drops() {
        for working in [false, true] {
            let mut pet = PetRuntime::new(WorldPoint::new(500.0, 400.0), RuntimeTuning::default(), 7);
            pet.set_displays(vec![display("main", 0.0, 0.0)]);
            // Avoidance is independently configurable; direct clicks must not
            // depend on either avoidance or the fast-approach reaction.
            pet.set_flags(true, false, true);
            if working {
                pet.handle_activity_event(CompanionEvent::new(
                    "work", "codex:turn", 10.0, CompanionEventKind::HighIntensity, 1.0, None,
                ), 10.0);
            } else {
                pet.begin_stroll(WorldPoint::new(750.0, 400.0), 10.0, 0.0);
            }
            let contact = pet.position();
            let mut sample = input(10.1, contact);
            sample.pointer_is_over_pet = true;
            let out = pet.finish_tick(&sample);
            assert!(out.interaction_enabled, "the shell must receive the click without a fast approach");
            assert_eq!(pet.approach_hold_until, 0.0);
            assert_eq!(out.state, if working { BehaviorState::Work } else { BehaviorState::Wander });
            let origin = pet.position();
            pet.pointer_down(contact, 10.11);
            assert_eq!(pet.state(), BehaviorState::Caught);
            assert!(!pet.movement.has_route());
            let dragged_pointer = contact.offset(WorldVector::new(100.0, 50.0));
            pet.pointer_dragged(dragged_pointer, 112.0, 10.2);
            assert_eq!(pet.state(), BehaviorState::Dragged);
            assert_eq!(pet.position(), origin.offset(WorldVector::new(100.0, 50.0)));
            let dropped = pet.pointer_up(dragged_pointer, true, 10.3);
            assert!(dropped.persist_position);
            assert!(!pet.state().is_held());
        }
    }

    #[test]
    fn a_stationary_cursor_can_click_an_evading_pet() {
        let mut pet = PetRuntime::new(WorldPoint::new(500.0, 400.0), RuntimeTuning::default(), 7);
        pet.set_displays(vec![display("main", 0.0, 0.0)]);
        let contact = pet.position();
        let mut sample = input(10.0, contact);
        sample.pointer_is_over_pet = true;
        let out = pet.finish_tick(&sample);
        assert_eq!(pet.approach_hold_until, 0.0);
        assert_eq!(out.state, BehaviorState::EvadePointer);
        assert!(out.interaction_enabled);
        pet.pointer_down(contact, 10.01);
        assert_eq!(pet.state(), BehaviorState::Caught);
    }

    #[test]
    fn direct_mouse_click_respects_body_visibility_and_interaction_gates() {
        let mut pet = PetRuntime::new(WorldPoint::new(500.0, 400.0), RuntimeTuning::default(), 7);
        pet.set_displays(vec![display("main", 0.0, 0.0)]);
        let contact = pet.position();
        assert!(!pet.finish_tick(&input(10.0, WorldPoint::new(800.0, 700.0))).interaction_enabled);
        pet.pointer_down(WorldPoint::new(800.0, 700.0), 10.01);
        assert!(!pet.state().is_held());
        let mut sample = input(10.1, contact);
        sample.pointer_is_over_pet = true;
        pet.set_hidden(true);
        assert!(!pet.finish_tick(&sample).interaction_enabled);
        pet.pointer_down(contact, 10.11);
        assert!(!pet.state().is_held());
        pet.set_hidden(false);
        pet.set_flags(true, false, false);
        sample.now = 10.2;
        assert!(!pet.finish_tick(&sample).interaction_enabled);
        pet.pointer_down(contact, 10.21);
        assert!(!pet.state().is_held());
        pet.set_flags(true, false, true);
        pet.pointer_down(pet.position(), 10.3);
        assert_eq!(pet.state(), BehaviorState::Caught);
        let origin = pet.position();
        pet.pointer_down(origin.offset(WorldVector::new(20.0, 0.0)), 10.31);
        pet.pointer_dragged(origin.offset(WorldVector::new(100.0, 0.0)), 100.0, 10.4);
        assert_eq!(pet.position(), origin.offset(WorldVector::new(100.0, 0.0)),
            "duplicate down must not reset the grab offset");
    }

    #[test]
    fn codex_work_ignores_glance_but_not_affection_or_close_evasion() {
        let mut pet = PetRuntime::new(WorldPoint::new(500.0, 400.0), RuntimeTuning::default(), 7);
        pet.set_displays(vec![display("main", 0.0, 0.0)]);
        pet.handle_activity_event(
            CompanionEvent::new(
                "work",
                "codex:turn",
                10.0,
                CompanionEventKind::HighIntensity,
                1.0,
                None,
            ),
            10.0,
        );
        assert_eq!(pet.state(), BehaviorState::Work);
        for i in 0..90 {
            let now = 10.0 + f64::from(i) / 30.0;
            pet.begin_tick(now);
            let out = pet.finish_tick(&input(now, WorldPoint::new(635.0, 400.0)));
            assert_eq!(out.state, BehaviorState::Work);
            assert_eq!(out.capability, PetCapability::Work);
        }
        let mut affection = input(13.1, WorldPoint::new(635.0, 400.0));
        affection.affection_held = true;
        assert_eq!(
            pet.finish_tick(&affection).state,
            BehaviorState::LookAtPointer
        );
        // Work without a window hint must also recover after explicit affection.
        assert_eq!(
            pet.finish_tick(&input(13.2, WorldPoint::new(635.0, 400.0)))
                .state,
            BehaviorState::Work
        );
        // A held approach reaction exposes the hit target without starting a tail wag.
        pet.approach_hold_until = 14.0;
        assert_eq!(
            pet.finish_tick(&input(13.3, WorldPoint::new(635.0, 400.0)))
                .state,
            BehaviorState::Work
        );
        pet.pointer_down(pet.position(), 13.4);
        assert_eq!(pet.state(), BehaviorState::Caught);

        let mut evading =
            PetRuntime::new(WorldPoint::new(500.0, 400.0), RuntimeTuning::default(), 7);
        evading.set_displays(vec![display("main", 0.0, 0.0)]);
        evading.handle_activity_event(
            CompanionEvent::new(
                "work",
                "codex:turn",
                10.0,
                CompanionEventKind::HighIntensity,
                1.0,
                None,
            ),
            10.0,
        );
        assert_eq!(
            evading
                .finish_tick(&input(10.0, WorldPoint::new(580.0, 400.0)))
                .state,
            BehaviorState::EvadePointer
        );
    }

    #[test]
    fn a_wandering_pet_only_wags_after_arriving_and_sitting_still() {
        let mut pet = PetRuntime::new(WorldPoint::new(500.0, 400.0), RuntimeTuning::default(), 7);
        pet.set_displays(vec![display("main", 0.0, 0.0)]);
        pet.begin_stroll(WorldPoint::new(750.0, 400.0), 10.0, 0.0);
        pet.approach_hold_until = 100.0;
        let mut arrived = false;
        for i in 1..600 {
            let now = 10.0 + f64::from(i) / 30.0;
            pet.begin_tick(now);
            let pointer = pet.position().offset(WorldVector::new(135.0, 0.0));
            let out = pet.finish_tick(&input(now, pointer));
            assert_ne!(out.state, BehaviorState::LookAtPointer);
            if !pet.movement.has_route() {
                assert_eq!(out.state, BehaviorState::Idle);
                assert_eq!(pet.position(), WorldPoint::new(750.0, 400.0));
                let seated = pet.finish_tick(&input(now + 0.1, pointer));
                assert_eq!(seated.state, BehaviorState::LookAtPointer);
                arrived = true;
                break;
            }
        }
        assert!(arrived);
    }

    #[test]
    fn shared_edge_crossings_finish_in_one_direction_despite_a_nearby_cursor() {
        for (dx, dy) in [(1000.0, 0.0), (-1000.0, 0.0), (0.0, 800.0), (0.0, -800.0)] {
            let centre = WorldPoint::new(500.0, 400.0);
            let direction = WorldVector::new(dx, dy).normalized();
            let start = centre.offset(direction.scaled(if dx != 0.0 { 400.0 } else { 300.0 }));
            let destination = centre.offset(WorldVector::new(dx, dy));
            let mut pet = PetRuntime::new(start, RuntimeTuning::default(), 7);
            pet.set_displays(vec![display("a", 0.0, 0.0), display("b", dx, dy)]);
            pet.begin_stroll(destination, 0.0, 0.0);
            let mut previous = start;
            let mut committed = false;
            let mut finished = false;
            for i in 1..900 {
                let now = f64::from(i) / 30.0;
                pet.begin_tick(now);
                let previous_speed = pet.movement.velocity().length();
                let out =
                    pet.finish_tick(&input(now, pet.position().offset(direction.scaled(135.0))));
                committed |= pet.crossing_clear.is_some();
                assert!(out.position.vector_from(previous).dot(direction) >= -0.001);
                assert_ne!(out.state, BehaviorState::LookAtPointer);
                let seam = centre.offset(direction.scaled(if dx != 0.0 { 500.0 } else { 400.0 }));
                if out.position.distance(seam) < 20.0 {
                    assert!(
                        pet.movement.velocity().length() + 0.001 >= previous_speed,
                        "slowed down on the display seam"
                    );
                }
                if committed && pet.crossing_clear.is_none() {
                    let target = &pet.displays[1];
                    assert!(placement_frame(target, &pet.displays)
                        .inset_by(48.0, 52.0)
                        .contains(out.position));
                    finished = true;
                    break;
                }
                previous = out.position;
            }
            assert!(finished, "crossing did not finish: {dx}, {dy}");
        }
    }

    #[test]
    fn wander_and_sleep_destinations_leave_shared_edges_clear() {
        let mut pet = PetRuntime::new(WorldPoint::new(990.0, 400.0), RuntimeTuning::default(), 7);
        pet.set_displays(vec![
            display("left", 0.0, 0.0),
            display("right", 1000.0, 0.0),
        ]);
        for _ in 0..500 {
            let point = pet.random_wander_point().unwrap();
            assert!((point.x - 1000.0).abs() >= 168.0);
        }
        pet.begin_rest_travel(WorldPoint::new(0.0, 0.0), false, 10.0);
        assert!((pet.rest_destination.as_ref().unwrap().point.x - 1000.0).abs() >= 168.0);
        let world = placement_world(&pet.world);
        let seat = crate::interest::BasicInterestPositionPlanner::destination(
            &LocationHint::new(Some(WorldRect::new(650.0, 0.0, 350.0, 800.0)), 1.0),
            &world,
            pet.position(),
            None,
            0.0,
            pet.object_size,
        )
        .unwrap();
        assert!((seat.point.x - 1000.0).abs() >= 168.0);
    }

    #[test]
    fn a_direct_catch_can_interrupt_a_committed_crossing() {
        let mut pet = PetRuntime::new(WorldPoint::new(900.0, 400.0), RuntimeTuning::default(), 7);
        pet.set_displays(vec![
            display("left", 0.0, 0.0),
            display("right", 1000.0, 0.0),
        ]);
        pet.begin_stroll(WorldPoint::new(1300.0, 400.0), 0.0, 0.0);
        pet.finish_tick(&input(0.1, WorldPoint::new(500.0, 400.0)));
        assert!(pet.crossing_clear.is_some());
        assert_eq!(pet.approach_hold_until, 0.0);
        pet.pointer_down(pet.position(), 0.2);
        assert_eq!(pet.state(), BehaviorState::Caught);
        assert!(pet.crossing_clear.is_none());
        assert!(!pet.movement.has_route());
    }
}

#[cfg(test)]
mod hidden_tests {
    use super::*;
    use crate::activity::CompanionEventKind;

    #[test]
    fn direct_touch_uses_the_shared_catch_drag_and_drop_without_arming_a_mouse() {
        let mut pet = PetRuntime::new(WorldPoint::new(100.0, 100.0), RuntimeTuning::default(), 7);
        pet.set_object_size(WorldSize::new(96.0, 104.0));
        pet.set_displays(vec![DisplaySnapshot {
            id: "phone".into(), name: "Test".into(),
            frame: WorldRect::new(0.0, 20.0, 400.0, 750.0),
            visible_frame: WorldRect::new(0.0, 20.0, 400.0, 750.0), scale: 1.0,
        }]);
        let contact = WorldPoint::new(140.0, 150.0);
        pet.touch_down(WorldPoint::new(51.0, 150.0), 10.0);
        assert!(!pet.state().is_held());
        pet.set_hidden(true);
        pet.touch_down(contact, 10.0);
        assert!(!pet.state().is_held());
        pet.set_hidden(false);
        pet.set_flags(true, true, false);
        pet.touch_down(contact, 10.0);
        assert!(!pet.state().is_held());
        pet.set_flags(true, true, true);
        pet.touch_down(contact, 10.0);
        assert_eq!(pet.state(), BehaviorState::Caught);
        // A second contact must not replace the original grab offset.
        pet.touch_down(WorldPoint::new(120.0, 130.0), 10.01);
        pet.pointer_dragged(WorldPoint::new(240.0, 250.0), 140.0, 10.1);
        assert_eq!(pet.position(), WorldPoint::new(200.0, 200.0));
        assert_eq!(pet.state(), BehaviorState::Dragged);
        let dropped = pet.pointer_up(WorldPoint::new(500.0, 900.0), false, 10.2);
        assert!(dropped.persist_position);
        assert!(!pet.state().is_held());
        assert!(pet.position().x <= 352.0 && pet.position().y <= 718.0);
    }

    #[test]
    fn hiding_releases_the_catch_and_suppresses_only_luminance() {
        let mut pet = PetRuntime::new(WorldPoint::new(300.0, 300.0), RuntimeTuning::default(), 7);
        let display = DisplaySnapshot {
            id: "display".into(),
            name: "Test".into(),
            frame: WorldRect::new(0.0, 0.0, 1_000.0, 800.0),
            visible_frame: WorldRect::new(0.0, 0.0, 1_000.0, 760.0),
            scale: 1.0,
        };
        pet.set_displays(vec![display]);
        let region = WorldRect::new(100.0, 100.0, 600.0, 400.0);

        pet.approach_hold_until = 42.0;
        pet.request_luminance(region, LUMINANCE_REFRESH_INTERVAL);
        assert_eq!(pet.luminance_requests.len(), 1);

        pet.set_hidden(true);
        assert_eq!(pet.approach_hold_until, 0.0);
        assert!(pet.luminance_requests.is_empty());

        let requests = pet.handle_activity_event(
            CompanionEvent::new(
                "event",
                "agent:hidden-turn",
                10.0,
                CompanionEventKind::ActivityStarted,
                0.5,
                Some(LocationHint::new(Some(region), 0.8)),
            ),
            10.0,
        );
        assert!(requests.is_empty(), "a hidden event requested luminance");
        assert_eq!(pet.active_source_id(), Some("agent:hidden-turn"));

        pet.set_hidden(false);
        assert_eq!(pet.active_source_id(), Some("agent:hidden-turn"));
        pet.request_luminance(region, LUMINANCE_REFRESH_INTERVAL);
        assert_eq!(pet.luminance_requests.len(), 1);
    }
}
