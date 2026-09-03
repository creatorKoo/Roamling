// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Ported from `Sources/RoamlingCore/BehaviorController.swift` and
//! `BehaviorTiming.swift`.
//!
//! The timings are not ours. Petdex classes `waving`, `jumping`, `failed` and
//! `review` as duration states with a published length each, and a row drawn to
//! that contract is paced for them: hold one longer and it loops, shorter and
//! it is cut mid-gesture. `BehaviorTiming matches the Petdex contract` in the
//! Swift logic tests pins the numbers; this file must not drift from them.

use crate::activity::CompanionReaction;
use crate::pointer::PointerProximity;

/// Declaration order is the wire order. Swift sends the index, so inserting a
/// state anywhere but the end renames every state above it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BehaviorState {
    Idle,
    Wander,
    LookAtPointer,
    EvadePointer,
    Caught,
    Dragged,
    Dropped,
    Sit,
    FindSleepSpot,
    Sleep,
    Wake,
    Stretch,
    TravelToInterest,
    Observe,
    Spark,
    Work,
    WaitingForUser,
    Celebrate,
    Sad,
}

pub const BEHAVIOR_STATES: [BehaviorState; 19] = [
    BehaviorState::Idle,
    BehaviorState::Wander,
    BehaviorState::LookAtPointer,
    BehaviorState::EvadePointer,
    BehaviorState::Caught,
    BehaviorState::Dragged,
    BehaviorState::Dropped,
    BehaviorState::Sit,
    BehaviorState::FindSleepSpot,
    BehaviorState::Sleep,
    BehaviorState::Wake,
    BehaviorState::Stretch,
    BehaviorState::TravelToInterest,
    BehaviorState::Observe,
    BehaviorState::Spark,
    BehaviorState::Work,
    BehaviorState::WaitingForUser,
    BehaviorState::Celebrate,
    BehaviorState::Sad,
];

impl BehaviorState {
    pub fn is_resting(self) -> bool {
        matches!(self, Self::Sit | Self::FindSleepSpot | Self::Sleep)
    }

    /// The pointer owns the pet outright.
    pub fn is_held(self) -> bool {
        matches!(self, Self::Caught | Self::Dragged)
    }

    /// States that `BeginWander` may start roaming from.
    ///
    /// `Observe` and `Work` belong here because the activity that parked the
    /// pet in them is already cleared before roaming runs, and `Wander` because
    /// a route can be cancelled without ever arriving. Omitting any of them
    /// strands the pet: the caller lays a route the state machine refuses, and
    /// the pet walks it wearing the wrong animation.
    ///
    /// `LookAtPointer` is here for one caller only -- the walk that takes the
    /// pet off the user's work, which must not be refused just because the
    /// cursor happens to be sitting beside it.
    pub fn allows_wander_entry(self) -> bool {
        matches!(
            self,
            Self::Idle | Self::Dropped | Self::Wander | Self::Observe | Self::Work
                | Self::LookAtPointer
        )
    }

    /// States that `BeginRest` may start sitting from. `Observe` and `Work` are
    /// in because a pet beside a working agent is standing on a seat it already
    /// vetted; refusing rest there means it can only doze between sessions.
    pub fn allows_rest_entry(self) -> bool {
        matches!(
            self,
            Self::Idle | Self::Wander | Self::Dropped | Self::Observe | Self::Work
        )
    }
}

/// How long a transient behavior holds before it hands back. Only `wake` and
/// `stretch` are Roamling's own numbers; the rest are Petdex's.
pub mod timing {
    /// Petdex `jumping`: the turn just started.
    pub const SPARK: f64 = 0.840;
    /// Petdex `review`: reading or searching. Plays once, then stillness.
    pub const OBSERVE: f64 = 1.030;
    /// Petdex `waving`: the turn finished.
    pub const CELEBRATE: f64 = 0.700;
    /// Petdex `failed`: a tool call failed.
    pub const SAD: f64 = 1.220;
    /// Petdex `jumping` again, because a drop borrows the hop.
    pub const DROPPED: f64 = 0.840;
    /// Roamling-only, the two lengths Petdex has no word for.
    pub const WAKE: f64 = 0.7;
    pub const STRETCH: f64 = 1.0;
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BehaviorInput {
    BeginWander,
    Arrived,
    BeginRest,
    SeekSleepSpot,
    SleepSpotReached,
    BeginStretch,
    BeginInterestTravel,
    Pointer(PointerProximity),
    CatchBegan,
    DragMoved,
    MouseReleased,
    Reaction(CompanionReaction),
    MeaningfulActivity,
    Tick,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BehaviorTransition {
    pub from: BehaviorState,
    pub to: BehaviorState,
    pub changed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BehaviorController {
    state: BehaviorState,
    entered_at: f64,
}

impl Default for BehaviorController {
    fn default() -> Self {
        Self::new(BehaviorState::Idle, 0.0)
    }
}

impl BehaviorController {
    pub fn new(state: BehaviorState, entered_at: f64) -> Self {
        Self { state, entered_at }
    }

    pub fn state(&self) -> BehaviorState {
        self.state
    }

    pub fn entered_at(&self) -> f64 {
        self.entered_at
    }

    pub fn handle(&mut self, input: BehaviorInput, timestamp: f64) -> BehaviorTransition {
        let old = self.state;

        match input {
            BehaviorInput::BeginWander => {
                if self.state.allows_wander_entry() {
                    self.transition(BehaviorState::Wander, timestamp);
                }
            }
            BehaviorInput::Arrived => {
                if matches!(
                    self.state,
                    BehaviorState::Wander | BehaviorState::TravelToInterest
                ) {
                    self.transition(BehaviorState::Idle, timestamp);
                }
            }
            BehaviorInput::BeginRest => {
                if self.state.allows_rest_entry() {
                    self.transition(BehaviorState::Sit, timestamp);
                }
            }
            BehaviorInput::SeekSleepSpot => {
                if self.state == BehaviorState::Sit {
                    self.transition(BehaviorState::FindSleepSpot, timestamp);
                }
            }
            BehaviorInput::SleepSpotReached => {
                if self.state == BehaviorState::FindSleepSpot {
                    self.transition(BehaviorState::Sleep, timestamp);
                }
            }
            BehaviorInput::BeginStretch => {
                if !self.state.is_held() {
                    self.transition(BehaviorState::Stretch, timestamp);
                }
            }
            BehaviorInput::BeginInterestTravel => {
                if !self.state.is_held() {
                    self.transition(BehaviorState::TravelToInterest, timestamp);
                }
            }
            BehaviorInput::Pointer(proximity) => self.handle_pointer(proximity, timestamp),
            BehaviorInput::CatchBegan => {
                if self.state != BehaviorState::Dragged {
                    self.transition(BehaviorState::Caught, timestamp);
                }
            }
            BehaviorInput::DragMoved => {
                if self.state.is_held() {
                    self.transition(BehaviorState::Dragged, timestamp);
                }
            }
            BehaviorInput::MouseReleased => {
                if self.state.is_held() {
                    self.transition(BehaviorState::Dropped, timestamp);
                }
            }
            BehaviorInput::Reaction(reaction) => {
                if !self.state.is_held() {
                    let next = match reaction {
                        CompanionReaction::Glance | CompanionReaction::Observe => {
                            BehaviorState::Observe
                        }
                        CompanionReaction::Spark => BehaviorState::Spark,
                        CompanionReaction::Work => BehaviorState::Work,
                        CompanionReaction::Paw => BehaviorState::WaitingForUser,
                        CompanionReaction::SmallCelebrate
                        | CompanionReaction::LargeCelebrate => BehaviorState::Celebrate,
                        CompanionReaction::Sad => BehaviorState::Sad,
                        CompanionReaction::Calm => BehaviorState::Idle,
                    };
                    self.transition(next, timestamp);
                }
            }
            BehaviorInput::MeaningfulActivity => {
                if self.state.is_resting() {
                    self.transition(BehaviorState::Wake, timestamp);
                }
            }
            BehaviorInput::Tick => self.settle_transient_state(timestamp),
        }

        BehaviorTransition {
            from: old,
            to: self.state,
            changed: old != self.state,
        }
    }

    fn handle_pointer(&mut self, proximity: PointerProximity, timestamp: f64) {
        if self.state.is_held() {
            return;
        }
        if self.state.is_resting() && proximity != PointerProximity::Far {
            self.transition(BehaviorState::Wake, timestamp);
        }
        match proximity {
            PointerProximity::Far => {
                if matches!(
                    self.state,
                    BehaviorState::LookAtPointer | BehaviorState::EvadePointer
                ) {
                    self.transition(BehaviorState::Idle, timestamp);
                }
            }
            PointerProximity::Watching | PointerProximity::Catchable => {
                self.transition(BehaviorState::LookAtPointer, timestamp);
            }
            PointerProximity::SlowEvade | PointerProximity::FastEvade => {
                self.transition(BehaviorState::EvadePointer, timestamp);
            }
        }
    }

    /// `WaitingForUser` deliberately has no timer: Petdex classes `waiting` as
    /// a steady state, so the pet keeps asking until the user answers.
    fn settle_transient_state(&mut self, timestamp: f64) {
        let age = timestamp - self.entered_at;
        let next = match self.state {
            BehaviorState::Dropped if age >= timing::DROPPED => BehaviorState::Idle,
            BehaviorState::Wake if age >= timing::WAKE => BehaviorState::Stretch,
            BehaviorState::Stretch if age >= timing::STRETCH => BehaviorState::Idle,
            BehaviorState::Celebrate if age >= timing::CELEBRATE => BehaviorState::Idle,
            BehaviorState::Sad if age >= timing::SAD => BehaviorState::Idle,
            BehaviorState::Spark if age >= timing::SPARK => BehaviorState::Idle,
            BehaviorState::Observe if age >= timing::OBSERVE => BehaviorState::Idle,
            _ => return,
        };
        self.transition(next, timestamp);
    }

    fn transition(&mut self, new_state: BehaviorState, timestamp: f64) {
        if new_state == self.state {
            return;
        }
        self.state = new_state;
        self.entered_at = timestamp;
    }
}
