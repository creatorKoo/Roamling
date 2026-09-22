// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

use super::*;
use crate::activity::CompanionEvent;
use crate::behavior::BEHAVIOR_STATES;
use crate::capability::PET_CAPABILITIES;
use crate::emptiness::LuminanceField;
use crate::geometry::WorldPoint;
use crate::geometry::WorldRect;
use crate::geometry::WorldSize;
use crate::pet_runtime::PetRuntime;
use crate::pet_runtime::TickInput;
use crate::source_state::StateDeclaration;
use crate::tuning::RuntimeTuning as CoreTuning;
use crate::world::DisplaySnapshot;
use crate::world::FocusSnapshot;
use std::sync::Mutex;

// ------------------------------------------------------------- the tick loop

#[derive(uniffi::Record)]
pub struct FfiDiagnostic {
    pub category: String,
    pub message: String,
}

/// A capture the runtime would like, near a region it cares about. The caller
/// owns the permission, the task and the throttle.
#[derive(uniffi::Record)]
pub struct FfiLuminanceRequest {
    pub region: FfiRect,
    pub interval: f64,
}

#[derive(uniffi::Record)]
pub struct FfiTickInput {
    pub now: f64,
    pub pointer_x: f64,
    pub pointer_y: f64,
    pub primary_button_down: bool,
    pub user_idle_duration: f64,
    pub capture_authorized: bool,
    pub focus_authorized: bool,
    pub did_query_focus: bool,
    pub queried_focus: Option<FfiFocus>,
    pub pointer_is_over_pet: bool,
    pub affection_held: bool,
}

#[derive(uniffi::Record)]
pub struct FfiTickOutput {
    pub delta_time: f64,
    pub x: f64,
    pub y: f64,
    pub state: u8,
    pub capability: u8,
    pub look_direction_degrees: Option<f64>,
    pub locomotion_rate: f64,
    pub interaction_enabled: bool,
    pub luminance_requests: Vec<FfiLuminanceRequest>,
    pub diagnostics: Vec<FfiDiagnostic>,
    pub persist_position: bool,
}

/// Polygon points use pet-width units, relative to its centre (y down).
#[derive(uniffi::Record)]
pub struct FfiEffectFrame {
    pub points: Vec<FfiPoint>,
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub opacity: f64,
}

#[derive(uniffi::Record)]
pub struct FfiInteractionOutput {
    pub x: f64,
    pub y: f64,
    pub capability: u8,
    pub look_direction_degrees: Option<f64>,
    pub set_interaction_enabled: Option<bool>,
    pub render: bool,
    pub reschedule_after: Option<f64>,
    pub persist_position: bool,
}

fn tuning_from(value: &FfiTuning) -> CoreTuning {
    CoreTuning::new(
        value.walking_speed,
        value.wander_pause,
        value.cross_display_wander_chance,
        value.pointer_awareness_distance,
        value.catch_arm_distance,
        value.catch_approach_speed,
        value.catch_window,
        value.hit_region_scale,
        value.gait_cadence,
        value.evade_speed_scale,
        value.idle_before_rest,
    )
}

fn displays_from(displays: &[FfiDisplay]) -> Vec<DisplaySnapshot> {
    displays
        .iter()
        .map(|display| DisplaySnapshot {
            id: display.id.clone(),
            name: String::new(),
            frame: (&display.frame).into(),
            visible_frame: (&display.visible_frame).into(),
            scale: 1.0,
        })
        .collect()
}

fn capability_index(capability: crate::capability::PetCapability) -> u8 {
    PET_CAPABILITIES
        .iter()
        .position(|candidate| *candidate == capability)
        .unwrap_or(0) as u8
}

fn state_index(state: crate::behavior::BehaviorState) -> u8 {
    BEHAVIOR_STATES
        .iter()
        .position(|candidate| *candidate == state)
        .unwrap_or(0) as u8
}

fn interaction_out(answer: crate::pet_runtime::InteractionOutput) -> FfiInteractionOutput {
    FfiInteractionOutput {
        x: answer.position.x,
        y: answer.position.y,
        capability: capability_index(answer.capability),
        look_direction_degrees: answer.look_direction_degrees,
        set_interaction_enabled: answer.set_interaction_enabled,
        render: answer.render,
        reschedule_after: answer.reschedule_after,
        persist_position: answer.persist_position,
    }
}

/// Everything the pet decides, in one object. The shell around it keeps only
/// the parts that are not decisions: the timer, the defaults, the diagnostics
/// file, the agent subscriptions and the sprite sheet.
#[derive(uniffi::Object)]
pub struct PetLoop {
    inner: Mutex<PetRuntime>,
}

#[uniffi::export]
impl PetLoop {
    #[uniffi::constructor]
    pub fn new(x: f64, y: f64, tuning: FfiTuning, seed: u64) -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self {
            inner: Mutex::new(PetRuntime::new(
                WorldPoint::new(x, y),
                tuning_from(&tuning),
                seed,
            )),
        })
    }

    pub fn set_displays(&self, displays: Vec<FfiDisplay>) {
        self.inner
            .lock()
            .unwrap()
            .set_displays(displays_from(&displays));
    }

    pub fn set_luminance(&self, field: Option<FfiLuminanceField>) {
        let field = field.and_then(|value| {
            LuminanceField::new(
                (&value.bounds).into(),
                value.columns as usize,
                value.rows as usize,
                value.samples,
            )
        });
        self.inner.lock().unwrap().set_luminance(field);
    }

    pub fn effect_frames(&self) -> Vec<FfiEffectFrame> {
        self.inner.lock().unwrap().effect_frames().into_iter().map(|frame| FfiEffectFrame {
            points: frame.points.into_iter().map(|point| FfiPoint { x: point.x, y: point.y }).collect(),
            red: frame.red, green: frame.green, blue: frame.blue, opacity: frame.opacity,
        }).collect()
    }

    pub fn set_object_size(&self, width: f64, height: f64) {
        self.inner
            .lock()
            .unwrap()
            .set_object_size(WorldSize::new(width, height));
    }

    pub fn set_flags(&self, roaming: bool, avoidance: bool, interactions: bool) {
        self.inner
            .lock()
            .unwrap()
            .set_flags(roaming, avoidance, interactions);
    }

    pub fn set_roaming_enabled(&self, enabled: bool, now: f64) {
        self.inner.lock().unwrap().set_roaming_enabled(enabled, now);
    }

    pub fn set_pointer_avoidance_enabled(&self, enabled: bool) {
        self.inner
            .lock()
            .unwrap()
            .set_pointer_avoidance_enabled(enabled);
    }

    pub fn set_interactions_enabled(&self, enabled: bool) -> bool {
        self.inner.lock().unwrap().set_interactions_enabled(enabled)
    }

    pub fn set_hidden(&self, hidden: bool) {
        self.inner.lock().unwrap().set_hidden(hidden);
    }

    pub fn set_animation_durations(&self, caught: f64, dragged: f64) {
        self.inner
            .lock()
            .unwrap()
            .set_animation_durations(caught, dragged);
    }

    pub fn set_position(&self, x: f64, y: f64) {
        self.inner
            .lock()
            .unwrap()
            .set_position(WorldPoint::new(x, y));
    }

    pub fn clear_click_reaction(&self, clear_caught_transition: bool) {
        self.inner
            .lock()
            .unwrap()
            .clear_click_reaction(clear_caught_transition);
    }

    pub fn set_next_wander_at(&self, timestamp: f64) {
        self.inner.lock().unwrap().set_next_wander_at(timestamp);
    }

    pub fn apply_tuning(&self, tuning: FfiTuning, now: f64) {
        self.inner
            .lock()
            .unwrap()
            .apply_tuning(tuning_from(&tuning), now);
    }

    pub fn handle_display_change(
        &self,
        displays: Vec<FfiDisplay>,
        carried_x: f64,
        carried_y: f64,
        now: f64,
    ) -> FfiPoint {
        let point = self.inner.lock().unwrap().handle_display_change(
            displays_from(&displays),
            WorldPoint::new(carried_x, carried_y),
            now,
        );
        FfiPoint {
            x: point.x,
            y: point.y,
        }
    }

    pub fn set_scale(&self, width: f64, height: f64) -> FfiPoint {
        let point = self
            .inner
            .lock()
            .unwrap()
            .set_scale(WorldSize::new(width, height));
        FfiPoint {
            x: point.x,
            y: point.y,
        }
    }

    /// Everything before the platform is asked anything. True when an
    /// accessibility query is worth paying for this tick.
    pub fn begin_tick(&self, now: f64) -> bool {
        self.inner.lock().unwrap().begin_tick(now)
    }

    pub fn finish_tick(&self, input: FfiTickInput) -> FfiTickOutput {
        let focus = input.queried_focus.as_ref().map(|focus| {
            FocusSnapshot::new(
                focus.window_frame.as_ref().map(WorldRect::from),
                focus.focused_element_frame.as_ref().map(WorldRect::from),
                focus.caret_frame.as_ref().map(WorldRect::from),
                focus.confidence,
            )
        });
        let answer = self.inner.lock().unwrap().finish_tick(&TickInput {
            now: input.now,
            pointer: WorldPoint::new(input.pointer_x, input.pointer_y),
            primary_button_down: input.primary_button_down,
            user_idle_duration: input.user_idle_duration,
            capture_authorized: input.capture_authorized,
            focus_authorized: input.focus_authorized,
            did_query_focus: input.did_query_focus,
            queried_focus: focus,
            pointer_is_over_pet: input.pointer_is_over_pet,
            affection_held: input.affection_held,
        });
        FfiTickOutput {
            delta_time: answer.delta_time,
            x: answer.position.x,
            y: answer.position.y,
            state: state_index(answer.state),
            capability: capability_index(answer.capability),
            look_direction_degrees: answer.look_direction_degrees,
            locomotion_rate: answer.locomotion_rate,
            interaction_enabled: answer.interaction_enabled,
            luminance_requests: answer
                .luminance_requests
                .into_iter()
                .map(|request| FfiLuminanceRequest {
                    region: request.region.into(),
                    interval: request.interval,
                })
                .collect(),
            diagnostics: answer
                .diagnostics
                .into_iter()
                .map(|(category, message)| FfiDiagnostic { category, message })
                .collect(),
            persist_position: answer.persist_position,
        }
    }

    pub fn pointer_down(&self, x: f64, y: f64, now: f64) -> FfiInteractionOutput {
        interaction_out(
            self.inner
                .lock()
                .unwrap()
                .pointer_down(WorldPoint::new(x, y), now),
        )
    }

    pub fn touch_down(&self, x: f64, y: f64, now: f64) -> FfiInteractionOutput {
        interaction_out(self.inner.lock().unwrap().touch_down(WorldPoint::new(x, y), now))
    }

    pub fn pointer_dragged(&self, x: f64, y: f64, distance: f64, now: f64) -> FfiInteractionOutput {
        interaction_out(self.inner.lock().unwrap().pointer_dragged(
            WorldPoint::new(x, y),
            distance,
            now,
        ))
    }

    pub fn pointer_up(&self, x: f64, y: f64, was_dragged: bool, now: f64) -> FfiInteractionOutput {
        interaction_out(self.inner.lock().unwrap().pointer_up(
            WorldPoint::new(x, y),
            was_dragged,
            now,
        ))
    }

    pub fn handle_activity_event(
        &self,
        event: FfiActivityEvent,
        now: f64,
    ) -> Vec<FfiLuminanceRequest> {
        self.inner
            .lock()
            .unwrap()
            .handle_activity_event(CompanionEvent::from(&event), now)
            .into_iter()
            .map(|request| FfiLuminanceRequest {
                region: request.region.into(),
                interval: request.interval,
            })
            .collect()
    }

    pub fn declare_state(
        &self,
        declaration: FfiStateDeclaration,
        now: f64,
    ) -> Vec<FfiLuminanceRequest> {
        self.inner
            .lock()
            .unwrap()
            .declare_state(StateDeclaration::from(&declaration), now)
            .into_iter()
            .map(|request| FfiLuminanceRequest {
                region: request.region.into(),
                interval: request.interval,
            })
            .collect()
    }

    pub fn position(&self) -> FfiPoint {
        let point = self.inner.lock().unwrap().position();
        FfiPoint {
            x: point.x,
            y: point.y,
        }
    }

    pub fn state(&self) -> u8 {
        state_index(self.inner.lock().unwrap().state())
    }

    pub fn is_placement_travelling(&self) -> bool {
        self.inner.lock().unwrap().is_placement_travelling()
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

    /// How many random numbers the pet has spent. Only the recorded-session
    /// test reads it, and it is the fastest way to see two runs part company.
    pub fn draws(&self) -> u64 {
        self.inner.lock().unwrap().draws()
    }

    pub fn preferred_tick_interval(&self, now: f64) -> f64 {
        self.inner.lock().unwrap().preferred_tick_interval(now)
    }
}
