// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! The words the diagnostics log uses. Strings only; nothing here decides.

use super::*;

/// The diagnostics vocabulary, which is the `rawValue` of the Swift enums. It
/// is user-visible through Copy Diagnostics, so it is spelled rather than
/// derived.
pub(super) fn state_name(state: BehaviorState) -> &'static str {
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

pub(super) fn proximity_name(proximity: PointerProximity) -> &'static str {
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

pub(super) fn describe(intent: &PlacementIntent) -> String {
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
