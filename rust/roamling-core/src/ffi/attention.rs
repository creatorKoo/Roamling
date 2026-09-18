// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

use super::*;
use crate::activity::ActivitySourceType;
use crate::activity::CompanionEvent;
use crate::activity::CompanionEventKind;
use crate::activity::CompanionReaction;
use crate::activity::ReactingBehavior;
use crate::activity::UserContext;
use crate::attention::AttentionModel;
use crate::attention::ReactionPolicy;
use crate::geometry::WorldRect;
use crate::source_state::Milestone;
use crate::source_state::SourceLevel;
use crate::source_state::StateDeclaration;
use crate::world::LocationHint;
use std::sync::Mutex;

// ------------------------------------------------------- attention and reactions

/// Enums cross as indices rather than as uniffi enums: the Swift side already
/// has its own spellings of these, and mapping two names is cheaper than
/// keeping three in step. New kinds go on the end: an index is a contract.
pub(super) const KINDS: [CompanionEventKind; 12] = [
    CompanionEventKind::ActivityStarted,
    CompanionEventKind::ActivityEnded,
    CompanionEventKind::Positive,
    CompanionEventKind::Negative,
    CompanionEventKind::Achievement,
    CompanionEventKind::Setback,
    CompanionEventKind::AttentionRequired,
    CompanionEventKind::Inspecting,
    CompanionEventKind::HighIntensity,
    CompanionEventKind::Calm,
    CompanionEventKind::Idle,
    CompanionEventKind::Present,
];

const CONTEXTS: [UserContext; 5] = [
    UserContext::Working,
    UserContext::Gaming,
    UserContext::WatchingMedia,
    UserContext::Browsing,
    UserContext::Idle,
];

/// Swift's `ActivitySourceType` in declaration order. `custom` crosses without
/// its name, which no rule reads.
const SOURCE_TYPES: [ActivitySourceType; 5] = [
    ActivitySourceType::Agent,
    ActivitySourceType::Game,
    ActivitySourceType::Media,
    ActivitySourceType::System,
    ActivitySourceType::Custom,
];

const STATE_LEVELS: [SourceLevel; 4] = [
    SourceLevel::Away,
    SourceLevel::Beside,
    SourceLevel::Active,
    SourceLevel::Paused,
];

const STATE_MILESTONES: [Milestone; 2] = [Milestone::SittingStarted, Milestone::SittingEnded];

#[derive(uniffi::Record)]
pub struct FfiStateDeclaration {
    pub source_id: String,
    /// An index into `SOURCE_TYPES`.
    pub source_type: u8,
    /// An index into `STATE_LEVELS`.
    pub level: u8,
    pub focused: bool,
    pub hint_confidence: Option<f64>,
    pub hint_region: Option<FfiRect>,
    /// An index into `STATE_MILESTONES`.
    pub milestone: Option<u8>,
}

impl From<&FfiStateDeclaration> for StateDeclaration {
    fn from(value: &FfiStateDeclaration) -> Self {
        StateDeclaration {
            source_id: value.source_id.clone(),
            source_type: SOURCE_TYPES[value.source_type as usize],
            level: STATE_LEVELS[value.level as usize],
            focused: value.focused,
            hint: value.hint_confidence.map(|confidence| {
                LocationHint::new(value.hint_region.as_ref().map(WorldRect::from), confidence)
            }),
            milestone: value
                .milestone
                .map(|index| STATE_MILESTONES[index as usize]),
        }
    }
}

impl From<StateDeclaration> for FfiStateDeclaration {
    fn from(value: StateDeclaration) -> Self {
        Self {
            source_id: value.source_id,
            source_type: SOURCE_TYPES
                .iter()
                .position(|candidate| *candidate == value.source_type)
                .unwrap_or(0) as u8,
            level: STATE_LEVELS
                .iter()
                .position(|candidate| *candidate == value.level)
                .unwrap_or(0) as u8,
            focused: value.focused,
            hint_confidence: value.hint.as_ref().map(|hint| hint.confidence),
            hint_region: value
                .hint
                .and_then(|hint| hint.approximate_region)
                .map(FfiRect::from),
            milestone: value.milestone.map(|milestone| {
                STATE_MILESTONES
                    .iter()
                    .position(|candidate| *candidate == milestone)
                    .unwrap_or(0) as u8
            }),
        }
    }
}

#[derive(uniffi::Record)]
pub struct FfiActivityEvent {
    pub id: String,
    pub source_id: String,
    /// An index into `SOURCE_TYPES`.
    pub source_type: u8,
    pub timestamp: f64,
    pub kind: u8,
    pub intensity: f64,
    pub hint_confidence: Option<f64>,
    /// Where the window is, when the adapter knew. Attention never reads this
    /// -- only the confidence -- but the activity director walks the pet to it.
    pub hint_region: Option<FfiRect>,
    pub context: Option<u8>,
}

impl From<&FfiActivityEvent> for CompanionEvent {
    fn from(value: &FfiActivityEvent) -> Self {
        CompanionEvent::new(
            value.id.clone(),
            value.source_id.clone(),
            value.timestamp,
            KINDS[value.kind as usize],
            value.intensity,
            value.hint_confidence.map(|confidence| {
                LocationHint::new(value.hint_region.as_ref().map(WorldRect::from), confidence)
            }),
        )
        .with_context(value.context.map(|index| CONTEXTS[index as usize]))
        .with_source_type(SOURCE_TYPES[value.source_type as usize])
    }
}

/// Which source the pet is watching. Held across calls, so Swift keeps a handle
/// rather than shipping the state back and forth.
#[derive(uniffi::Object)]
pub struct Attention {
    model: Mutex<AttentionModel>,
}

#[uniffi::export]
impl Attention {
    #[uniffi::constructor]
    pub fn new() -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self {
            model: Mutex::new(AttentionModel::default()),
        })
    }

    /// The id of the event to act on, or nil. Swift looks the event back up in
    /// its own table rather than having it marshalled home.
    pub fn select(&self, events: Vec<FfiActivityEvent>, timestamp: f64) -> Option<String> {
        let events: Vec<CompanionEvent> = events.iter().map(CompanionEvent::from).collect();
        self.model
            .lock()
            .unwrap()
            .select(&events, timestamp)
            .map(|event| event.id)
    }

    pub fn clear(&self, timestamp: f64) {
        self.model.lock().unwrap().clear(timestamp);
    }

    pub fn current_source_id(&self) -> Option<String> {
        self.model
            .lock()
            .unwrap()
            .current_source_id()
            .map(str::to_owned)
    }
}

/// How often the pet is allowed to react, and with what.
#[derive(uniffi::Object)]
pub struct Reactions {
    policy: Mutex<ReactionPolicy>,
}

#[uniffi::export]
impl Reactions {
    #[uniffi::constructor]
    pub fn new() -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self {
            policy: Mutex::new(ReactionPolicy::default()),
        })
    }

    /// The reaction's index, or nil for none.
    pub fn reaction(
        &self,
        event: FfiActivityEvent,
        context: u8,
        is_held_by_pointer: bool,
        random_unit: f64,
        timestamp: f64,
    ) -> Option<u8> {
        let behavior = if is_held_by_pointer {
            ReactingBehavior::Caught
        } else {
            ReactingBehavior::Other
        };
        self.policy
            .lock()
            .unwrap()
            .reaction(
                &CompanionEvent::from(&event),
                CONTEXTS[context as usize],
                behavior,
                random_unit,
                timestamp,
            )
            .map(|reaction| match reaction {
                CompanionReaction::Glance => 0,
                CompanionReaction::Observe => 1,
                CompanionReaction::Spark => 2,
                CompanionReaction::Work => 3,
                CompanionReaction::Paw => 4,
                CompanionReaction::SmallCelebrate => 5,
                CompanionReaction::LargeCelebrate => 6,
                CompanionReaction::Sad => 7,
                CompanionReaction::Calm => 8,
            })
    }
}
