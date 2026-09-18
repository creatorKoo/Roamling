// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

use super::*;
use crate::activity::CompanionEvent;
use crate::activity_director::ActivityDirector;
use crate::activity_director::ActivityEffect;
use std::sync::Mutex;

// ----------------------------------------------------- the activity director

/// One thing the runtime must do, in the order the decision made them. The
/// order is part of the answer: a setback settles the seat, cancels the route
/// and *then* reacts.
#[derive(uniffi::Record)]
pub struct FfiActivityEffect {
    /// 0 cancel rest, 1 settle in place, 2 cancel route, 3 next wander at,
    /// 4 apply reaction, 5 request luminance.
    pub kind: u8,
    pub source_id: Option<String>,
    pub timestamp: f64,
    pub reaction: u8,
    pub region: Option<FfiRect>,
}

fn effect(effect: &ActivityEffect) -> FfiActivityEffect {
    let mut answer = FfiActivityEffect {
        kind: 0,
        source_id: None,
        timestamp: 0.0,
        reaction: 0,
        region: None,
    };
    match effect {
        ActivityEffect::CancelRest => {}
        ActivityEffect::SettleInPlace { source_id } => {
            answer.kind = 1;
            answer.source_id = Some(source_id.clone());
        }
        ActivityEffect::CancelRoute => answer.kind = 2,
        ActivityEffect::SetNextWanderAt { timestamp } => {
            answer.kind = 3;
            answer.timestamp = *timestamp;
        }
        ActivityEffect::ApplyReaction { reaction } => {
            answer.kind = 4;
            answer.reaction = REACTIONS
                .iter()
                .position(|candidate| candidate == reaction)
                .unwrap_or(0) as u8;
        }
        ActivityEffect::RequestLuminance { region } => {
            answer.kind = 5;
            answer.region = Some((*region).into());
        }
    }
    answer
}

/// Which agent the pet is watching, what it wears, and what it still owes the
/// user when it arrives. Seven fields that survived between ticks on the
/// runtime, each written by more than one code path.
#[derive(uniffi::Object)]
pub struct ActivityWatch {
    inner: Mutex<ActivityDirector>,
}

#[uniffi::export]
impl ActivityWatch {
    #[uniffi::constructor]
    pub fn new() -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self {
            inner: Mutex::new(ActivityDirector::default()),
        })
    }

    pub fn handle_event(
        &self,
        event: FfiActivityEvent,
        is_held_by_pointer: bool,
        is_resting: bool,
        random_unit: f64,
        timestamp: f64,
    ) -> Vec<FfiActivityEffect> {
        self.inner
            .lock()
            .unwrap()
            .handle_event(
                CompanionEvent::from(&event),
                is_held_by_pointer,
                is_resting,
                random_unit,
                timestamp,
            )
            .iter()
            .map(effect)
            .collect()
    }

    pub fn expire_silent(&self, is_resting: bool, timestamp: f64) -> Vec<FfiActivityEffect> {
        self.inner
            .lock()
            .unwrap()
            .expire_silent(is_resting, timestamp)
            .iter()
            .map(effect)
            .collect()
    }

    pub fn resume_pending_if_ready(
        &self,
        is_idle: bool,
        is_held_by_pointer: bool,
        is_resting: bool,
        random_unit: f64,
        timestamp: f64,
    ) -> Vec<FfiActivityEffect> {
        self.inner
            .lock()
            .unwrap()
            .resume_pending_if_ready(
                is_idle,
                is_held_by_pointer,
                is_resting,
                random_unit,
                timestamp,
            )
            .iter()
            .map(effect)
            .collect()
    }

    pub fn deliver_arrival_reaction(
        &self,
        is_resting: bool,
        timestamp: f64,
    ) -> Vec<FfiActivityEffect> {
        self.inner
            .lock()
            .unwrap()
            .deliver_arrival_reaction(is_resting, timestamp)
            .iter()
            .map(effect)
            .collect()
    }

    pub fn sustain_on_seat(&self, is_resting: bool, timestamp: f64) -> Vec<FfiActivityEffect> {
        self.inner
            .lock()
            .unwrap()
            .sustain_on_seat(is_resting, timestamp)
            .iter()
            .map(effect)
            .collect()
    }

    pub fn is_watching_window(&self) -> bool {
        self.inner.lock().unwrap().is_watching_window()
    }

    pub fn active_source_id(&self) -> Option<String> {
        self.inner
            .lock()
            .unwrap()
            .active_source_id()
            .map(str::to_owned)
    }

    pub fn hint(&self) -> Option<FfiHint> {
        self.inner.lock().unwrap().hint().map(|hint| FfiHint {
            region: hint.approximate_region.map(FfiRect::from),
            confidence: hint.confidence,
        })
    }

    pub fn has_arrival_reaction(&self) -> bool {
        self.inner.lock().unwrap().has_arrival_reaction()
    }

    pub fn sustained_reaction(&self) -> Option<u8> {
        self.inner
            .lock()
            .unwrap()
            .sustained_reaction()
            .map(|reaction| {
                REACTIONS
                    .iter()
                    .position(|candidate| *candidate == reaction)
                    .unwrap_or(0) as u8
            })
    }

    /// The event the director last acted on. Read by the switch-over test.
    pub fn last_dispatched_id(&self) -> Option<String> {
        self.inner
            .lock()
            .unwrap()
            .last_dispatched_id()
            .map(str::to_owned)
    }

    /// Whether nothing the desk says may reach the pet now. Read by the
    /// switch-over test, which holds both directors to that by outcome.
    pub fn agent_on_duty(&self, timestamp: f64) -> bool {
        self.inner.lock().unwrap().agent_on_duty(timestamp)
    }

    /// The event queued for the pet once it is free. Read by the switch-over
    /// test.
    pub fn pending_event_id(&self) -> Option<String> {
        self.inner
            .lock()
            .unwrap()
            .pending_event_id()
            .map(str::to_owned)
    }
}

/// Whether an event without a window of its own is worth asking the platform
/// where its window is. The query costs a synchronous round trip, so the rule
/// lives here and the call stays with the caller.
#[uniffi::export]
pub fn activity_wants_window_hint(kind: u8) -> bool {
    crate::activity_director::wants_window_hint(KINDS[kind as usize])
}
