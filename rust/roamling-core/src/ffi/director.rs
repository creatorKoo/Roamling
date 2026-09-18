// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

use super::*;
use crate::emptiness::LuminanceField;
use crate::geometry::WorldPoint;
use crate::geometry::WorldRect;
use crate::geometry::WorldSize;
use crate::placement::PetSituation;
use crate::placement::PlacementConfiguration as CorePlacementConfiguration;
use crate::placement::PlacementDirector;
use crate::placement::PlacementIntent;
use crate::placement::PlacementPolicy;
use crate::placement::PlacementTravelReason;
use crate::world::DesktopWorldSnapshot;
use crate::world::DisplaySnapshot;
use crate::world::FocusSnapshot;
use crate::world::LocationHint;
use std::sync::Mutex;

// -------------------------------------------------------------- the director

const TRAVEL_REASONS: [PlacementTravelReason; 6] = [
    PlacementTravelReason::NewActivity,
    PlacementTravelReason::CoveringCaret,
    PlacementTravelReason::CoveringWork,
    PlacementTravelReason::PlannedBlind,
    PlacementTravelReason::FollowedFocus,
    PlacementTravelReason::SeatUnderPointer,
];

#[derive(uniffi::Record)]
pub struct FfiHint {
    /// Absent for a hint that names a window but not where it is. The planner
    /// refuses to place against one, and so does this.
    pub region: Option<FfiRect>,
    pub confidence: f64,
}

/// One tick's worth of what placement is allowed to look at.
///
/// The displays and the luminance grid are deliberately *not* here. They change
/// a few times a second at most and the grid is 64 columns wide, so pushing
/// them separately keeps the per-tick payload to a few dozen numbers instead of
/// a few thousand.
#[derive(uniffi::Record)]
pub struct FfiSituation {
    pub timestamp: f64,
    pub x: f64,
    pub y: f64,
    pub object_width: f64,
    pub object_height: f64,
    pub pointer: Option<Vec<f64>>,
    pub pointer_clearance: f64,
    pub walking_speed: f64,
    pub is_pointer_owned: bool,
    pub is_pointer_watching: bool,
    pub is_evading: bool,
    pub is_walking: bool,
    pub is_resting: bool,
    pub activity_source_id: Option<String>,
    pub hint: Option<FfiHint>,
    pub focus: Option<FfiFocus>,
    pub user_idle_duration: f64,
    pub idle_before_rest: f64,
    pub is_roaming_enabled: bool,
    pub is_stroll_due: bool,
    pub stroll_candidates: Vec<FfiPoint>,
}

#[derive(uniffi::Record)]
pub struct FfiPlacementIntent {
    /// 0 none, 1 hold, 2 travel, 3 sleep in place, 4 stroll, 5 escape.
    pub kind: u8,
    pub x: f64,
    pub y: f64,
    pub score: f64,
    /// Meaningful only when `kind` is 2, along with `score` and `display_id`.
    pub reason: u8,
    pub display_id: String,
    pub is_seated: bool,
    pub is_travelling: bool,
}

/// The one place that answers where the pet should be, with its seat, its trip
/// and the time of its last review held on this side.
#[derive(uniffi::Object)]
pub struct Placement {
    director: Mutex<PlacementDirector>,
    displays: Mutex<Vec<DisplaySnapshot>>,
    field: Mutex<Option<LuminanceField>>,
}

#[uniffi::export]
impl Placement {
    #[uniffi::constructor]
    pub fn new() -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self {
            // The Swift side compares itself against this object, so it is the
            // ported rules and not the ones the shipping pet follows.
            director: Mutex::new(PlacementDirector::new(
                PlacementPolicy::PortedContract,
                CorePlacementConfiguration::default(),
            )),
            displays: Mutex::new(Vec::new()),
            field: Mutex::new(None),
        })
    }

    /// Pushed when the desk changes, not every tick.
    pub fn set_displays(&self, displays: Vec<FfiDisplay>) {
        *self.displays.lock().unwrap() = displays
            .iter()
            .map(|display| DisplaySnapshot {
                id: display.id.clone(),
                name: String::new(),
                frame: (&display.frame).into(),
                visible_frame: (&display.visible_frame).into(),
                scale: 1.0,
            })
            .collect();
    }

    /// Pushed when a capture lands, not every tick. The grid is the expensive
    /// part of the situation and it refreshes every three seconds at most.
    pub fn set_field(&self, field: Option<FfiLuminanceField>) {
        *self.field.lock().unwrap() = field.and_then(|value| {
            LuminanceField::new(
                (&value.bounds).into(),
                value.columns as usize,
                value.rows as usize,
                value.samples,
            )
        });
    }

    pub fn is_seated(&self) -> bool {
        self.director.lock().unwrap().is_seated()
    }

    pub fn is_travelling(&self) -> bool {
        self.director.lock().unwrap().is_travelling()
    }

    pub fn settle_in_place(&self, source_id: Option<String>, timestamp: f64) {
        self.director
            .lock()
            .unwrap()
            .settle_in_place(source_id.as_deref(), timestamp);
    }

    pub fn decide(&self, situation: FfiSituation) -> FfiPlacementIntent {
        let mut world =
            DesktopWorldSnapshot::new(self.displays.lock().unwrap().clone(), Vec::new());
        world.focus = situation.focus.as_ref().map(|focus| {
            FocusSnapshot::new(
                focus.window_frame.as_ref().map(WorldRect::from),
                focus.focused_element_frame.as_ref().map(WorldRect::from),
                focus.caret_frame.as_ref().map(WorldRect::from),
                focus.confidence,
            )
        });
        world.luminance = self.field.lock().unwrap().clone();

        let situation = PetSituation {
            timestamp: situation.timestamp,
            world,
            position: WorldPoint::new(situation.x, situation.y),
            object_size: WorldSize::new(situation.object_width, situation.object_height),
            pointer_position: situation.pointer.and_then(|values| {
                (values.len() == 2).then(|| WorldPoint::new(values[0], values[1]))
            }),
            pointer_clearance: situation.pointer_clearance,
            walking_speed: situation.walking_speed,
            is_pointer_owned: situation.is_pointer_owned,
            is_pointer_watching: situation.is_pointer_watching,
            is_evading: situation.is_evading,
            is_walking: situation.is_walking,
            is_resting: situation.is_resting,
            activity_source_id: situation.activity_source_id,
            activity_hint: situation.hint.as_ref().map(|hint| {
                LocationHint::new(hint.region.as_ref().map(WorldRect::from), hint.confidence)
            }),
            user_idle_duration: situation.user_idle_duration,
            idle_before_rest: situation.idle_before_rest,
            is_roaming_enabled: situation.is_roaming_enabled,
            is_stroll_due: situation.is_stroll_due,
            stroll_candidates: situation
                .stroll_candidates
                .iter()
                .map(|point| WorldPoint::new(point.x, point.y))
                .collect(),
        };

        let mut director = self.director.lock().unwrap();
        let intent = director.decide(&situation);
        let (kind, x, y, score, reason, display_id) = match &intent {
            PlacementIntent::None => (0u8, 0.0, 0.0, 0.0, 0u8, String::new()),
            PlacementIntent::Hold => (1, 0.0, 0.0, 0.0, 0, String::new()),
            PlacementIntent::Travel(destination, reason) => (
                2,
                destination.point.x,
                destination.point.y,
                destination.score,
                TRAVEL_REASONS
                    .iter()
                    .position(|value| value == reason)
                    .unwrap_or(0) as u8,
                destination.display_id.clone(),
            ),
            PlacementIntent::SleepInPlace => (3, 0.0, 0.0, 0.0, 0, String::new()),
            PlacementIntent::Stroll(point) => (4, point.x, point.y, 0.0, 0, String::new()),
            PlacementIntent::Escape(point) => (5, point.x, point.y, 0.0, 0, String::new()),
        };
        FfiPlacementIntent {
            kind,
            x,
            y,
            score,
            reason,
            display_id,
            is_seated: director.is_seated(),
            is_travelling: director.is_travelling(),
        }
    }
}
