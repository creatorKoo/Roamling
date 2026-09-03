// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Roamling's platform-independent core, being ported from Swift one unit at a
//! time. See `docs/windows.md`, "조각내서 갈아탄다", for the order and for why
//! the Swift original stays beside each unit until their outputs agree.

uniffi::setup_scaffolding!();

pub mod activity;
pub mod activity_director;
pub mod animation;
pub mod attention;
pub mod behavior;
pub mod capability;
pub mod coordinate_space;
pub mod emptiness;
pub mod ffi;
pub mod interest;
pub mod geometry;
pub mod movement;
pub mod pet_runtime;
pub mod placement;
pub mod pointer;
pub mod safe_zone;
pub mod topology;
pub mod tuning;
pub mod world;

pub use coordinate_space::DesktopCoordinateSpace;
pub use geometry::{clamped, swift_max, swift_min, WorldPoint, WorldRect, WorldSize, WorldVector};
pub use activity::{
    ActivityLifetime, CompanionEvent, CompanionEventKind, CompanionReaction, ReactingBehavior,
    UserContext,
};
pub use activity_director::{
    wants_window_hint, ActivityDirector, ActivityEffect,
};
pub use animation::{
    authored_names, look_frame_index, borrows, capability_name, petdex_state, standard_tracks, AnimationResolver,
    Borrow, Coverage, PetAnimationFrame, PetAnimationPlayer, PetAnimationTrack, PetdexState,
    Provenance,
};
pub use attention::{
    AttentionConfiguration, AttentionModel, ReactionConfiguration, ReactionPolicy,
};
pub use capability::{capability_for, PetCapability, PET_CAPABILITIES};
pub use behavior::{
    timing, BehaviorController, BehaviorInput, BehaviorState, BehaviorTransition,
    BEHAVIOR_STATES,
};
pub use movement::{MovementConfiguration, MovementController, MovementUpdate};
pub use pet_runtime::{
    Aimlessness, InteractionOutput, LuminanceRequest, PetRuntime, TickInput, TickOutput,
};
pub use placement::{
    PetSituation, PlacementConfiguration, PlacementDirector, PlacementIntent,
    PlacementTravelReason,
};
pub use pointer::{
    look_direction_degrees, PointerDecision, PointerInteractionConfiguration,
    PointerInteractionModel, PointerKinematics, PointerProximity,
};
pub use emptiness::{
    CandidatePositionScorer, LuminanceField, PositionCandidate, VisualEmptiness,
};
pub use interest::{BasicInterestPositionPlanner, InterestDestination, SeatEvaluation};
pub use safe_zone::{BasicSafeZonePlanner, RestDestination};
pub use tuning::{RuntimeTuning, RuntimeTuningKey, TUNING_KEYS};
pub use topology::{DisplayPortal, DisplayRoute, DisplayTopology};
pub use world::{DesktopWorldSnapshot, DisplaySnapshot, FocusSnapshot, LocationHint, SafeZone};
