// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Ported from `Sources/RoamlingPet/PetAnimation.swift` and
//! `PetCapabilityMapping.swift` -- only the naming and the mapping, not the
//! resolver or the atlas. What the runtime needs is the answer to "which
//! picture", and a shell needs that before it needs anything else in `Pet`.

use crate::behavior::BehaviorState;

/// Declaration order is the wire order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PetCapability {
    Idle,
    MoveLeft,
    MoveRight,
    Sit,
    Sleep,
    Work,
    Observe,
    Gaze,
    Paw,
    Spark,
    Celebrate,
    Fail,
    Stretch,
    Caught,
    Dragged,
    Landing,
}

pub const PET_CAPABILITIES: [PetCapability; 16] = [
    PetCapability::Idle,
    PetCapability::MoveLeft,
    PetCapability::MoveRight,
    PetCapability::Sit,
    PetCapability::Sleep,
    PetCapability::Work,
    PetCapability::Observe,
    PetCapability::Gaze,
    PetCapability::Paw,
    PetCapability::Spark,
    PetCapability::Celebrate,
    PetCapability::Fail,
    PetCapability::Stretch,
    PetCapability::Caught,
    PetCapability::Dragged,
    PetCapability::Landing,
];

/// Which picture a state wears.
///
/// Watching the cursor and reviewing a file are different pictures. They shared
/// `Observe` -- and so shared Petdex's `review` row, a one-second "about to read
/// a file" beat -- which is how a cursor drifting past the pet started a loop
/// that ran until something else interrupted it.
pub fn capability_for(
    state: BehaviorState,
    velocity_dx: f64,
    is_caught_transition_active: bool,
) -> PetCapability {
    match state {
        BehaviorState::Idle => PetCapability::Idle,
        BehaviorState::Wander
        | BehaviorState::EvadePointer
        | BehaviorState::FindSleepSpot
        | BehaviorState::TravelToInterest => {
            if velocity_dx < 0.0 {
                PetCapability::MoveLeft
            } else {
                PetCapability::MoveRight
            }
        }
        BehaviorState::Sit => PetCapability::Sit,
        BehaviorState::LookAtPointer => PetCapability::Gaze,
        BehaviorState::Observe => PetCapability::Observe,
        BehaviorState::Spark => PetCapability::Spark,
        BehaviorState::Caught => PetCapability::Caught,
        BehaviorState::Dragged => {
            if is_caught_transition_active {
                PetCapability::Caught
            } else {
                PetCapability::Dragged
            }
        }
        BehaviorState::Dropped => PetCapability::Landing,
        BehaviorState::Work => PetCapability::Work,
        BehaviorState::WaitingForUser => PetCapability::Paw,
        BehaviorState::Celebrate => PetCapability::Celebrate,
        BehaviorState::Sad => PetCapability::Fail,
        BehaviorState::Sleep => PetCapability::Sleep,
        BehaviorState::Stretch | BehaviorState::Wake => PetCapability::Stretch,
    }
}
