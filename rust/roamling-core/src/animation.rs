// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Ported from `Sources/RoamlingPet/PetAnimation.swift`, `PetdexState.swift`
//! and `PetAnimationPlayer.swift`.
//!
//! Turning "which picture" into "which frame of which row, right now". The
//! tick answers with a capability; this answers with an index into the atlas,
//! which is the last thing a shell needs before it can draw.
//!
//! The vocabulary is borrowed, not invented. Row order and standard durations
//! come from petdex `src/lib/pet-states.ts`; the agent meanings from
//! `petdex-desktop-native/src/hook_runner.zig`. When upstream changes, re-read
//! those files rather than adjusting a caller to match.

use std::collections::BTreeMap;

use crate::capability::{PetCapability, PET_CAPABILITIES};

/// The nine Petdex sprite rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PetdexState {
    Idle,
    RunningRight,
    RunningLeft,
    Waving,
    Jumping,
    Failed,
    Waiting,
    Running,
    Review,
}

impl PetdexState {
    pub fn name(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::RunningRight => "running-right",
            Self::RunningLeft => "running-left",
            Self::Waving => "waving",
            Self::Jumping => "jumping",
            Self::Failed => "failed",
            Self::Waiting => "waiting",
            Self::Running => "running",
            Self::Review => "review",
        }
    }

    /// Row index in the 8-column atlas.
    pub fn row(self) -> usize {
        match self {
            Self::Idle => 0,
            Self::RunningRight => 1,
            Self::RunningLeft => 2,
            Self::Waving => 3,
            Self::Jumping => 4,
            Self::Failed => 5,
            Self::Waiting => 6,
            Self::Running => 7,
            Self::Review => 8,
        }
    }

    /// Alternative track names Codex's `model.rs` accepts for the same row.
    pub fn aliases(self) -> &'static [&'static str] {
        match self {
            Self::RunningRight => &["move-right", "move_right"],
            Self::RunningLeft => &["move-left", "move_left"],
            Self::Waving => &["wave"],
            Self::Jumping => &["jump", "bounce"],
            Self::Failed => &["failure", "sad"],
            _ => &[],
        }
    }

    /// Every name that answers for this row, most specific first.
    pub fn track_names(self) -> Vec<&'static str> {
        let mut names = vec![self.name()];
        names.extend_from_slice(self.aliases());
        names
    }
}

/// How a capability finds artwork when it has none of its own.
///
/// The two kinds read the same to the resolver and differently to a person,
/// which is the point: `Landing` wants the *hop*, `Celebrate` wants the
/// *sentiment*. Recording only "falls back to" is how landing ended up chained
/// behind celebrate, so that fixing celebrate's meaning would have turned every
/// landing into a farewell wave.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Borrow {
    /// Borrow what it says (a finished turn is greeted, so celebrate waves).
    Meaning(PetCapability),
    /// Borrow what it does (a landing really is a hop).
    Motion(PetCapability),
}

impl Borrow {
    pub fn capability(self) -> PetCapability {
        match self {
            Self::Meaning(value) | Self::Motion(value) => value,
        }
    }
}

/// The Petdex row this capability *is*, or none when Petdex has no such idea.
pub fn petdex_state(capability: PetCapability) -> Option<PetdexState> {
    match capability {
        PetCapability::Idle => Some(PetdexState::Idle),
        PetCapability::MoveLeft => Some(PetdexState::RunningLeft),
        PetCapability::MoveRight => Some(PetdexState::RunningRight),
        PetCapability::Work => Some(PetdexState::Running),
        PetCapability::Observe => Some(PetdexState::Review),
        PetCapability::Paw => Some(PetdexState::Waiting),
        PetCapability::Spark => Some(PetdexState::Jumping),
        PetCapability::Celebrate => Some(PetdexState::Waving),
        PetCapability::Fail => Some(PetdexState::Failed),
        _ => None,
    }
}

/// Names a package may use under its own vocabulary. Tried before the Petdex
/// row name, so a package that draws the real thing always wins over a
/// borrowed row.
pub fn authored_names(capability: PetCapability) -> &'static [&'static str] {
    match capability {
        PetCapability::Idle => &["idle"],
        PetCapability::Sit => &["sitting", "sit"],
        PetCapability::Sleep => &["sleeping", "sleep", "napping"],
        PetCapability::Work => &["working", "typing"],
        PetCapability::Observe => &["observe"],
        // Watching the cursor is not reviewing a file. `watching` is the name a
        // Roamling-aware package uses for it; `review` deliberately is not.
        PetCapability::Gaze => &["gaze", "watching", "looking"],
        PetCapability::Paw => &["pawing", "paw"],
        PetCapability::Spark => &["spark"],
        PetCapability::Celebrate => &["celebrate"],
        PetCapability::Stretch => &["stretching", "stretch"],
        PetCapability::Caught => &["caught"],
        PetCapability::Dragged => &["dragged"],
        PetCapability::Landing => &["landing"],
        PetCapability::MoveLeft | PetCapability::MoveRight | PetCapability::Fail => &[],
    }
}

/// What to fall back to, and in which sense.
pub fn borrows(capability: PetCapability) -> Option<Borrow> {
    match capability {
        PetCapability::Idle => None,
        PetCapability::MoveLeft | PetCapability::MoveRight => {
            Some(Borrow::Motion(PetCapability::Idle))
        }
        // Settling down goes to `idle` -- "between events" -- rather than to
        // `waiting`, whose contract meaning is "blocked on the user". A drowsy
        // pet has not asked for anything, and borrowing `waiting` put the
        // picture for needing approval on screen every time one got sleepy.
        PetCapability::Sit => Some(Borrow::Meaning(PetCapability::Idle)),
        PetCapability::Sleep => Some(Borrow::Meaning(PetCapability::Sit)),
        PetCapability::Work => Some(Borrow::Motion(PetCapability::MoveRight)),
        PetCapability::Observe => Some(Borrow::Meaning(PetCapability::Idle)),
        // Petdex has no picture for watching the cursor for as long as it is
        // near, so this degrades to stillness rather than to `review`, which is
        // a one-second "reading a file" beat.
        PetCapability::Gaze => Some(Borrow::Meaning(PetCapability::Idle)),
        PetCapability::Paw => Some(Borrow::Meaning(PetCapability::Idle)),
        PetCapability::Spark => Some(Borrow::Meaning(PetCapability::Celebrate)),
        PetCapability::Celebrate => Some(Borrow::Meaning(PetCapability::Idle)),
        PetCapability::Fail => Some(Borrow::Meaning(PetCapability::Idle)),
        PetCapability::Stretch => Some(Borrow::Motion(PetCapability::Idle)),
        // A held pet looking up at the cursor reads as held. A jump does not:
        // it loops, so while the cursor carries the pet it looks like the pet
        // is bouncing under its own power, which is the opposite of caught.
        PetCapability::Caught => Some(Borrow::Meaning(PetCapability::Paw)),
        PetCapability::Dragged => Some(Borrow::Meaning(PetCapability::Caught)),
        // Landing really is a hop, so it takes the jump row directly instead of
        // routing through celebrate, which now waves.
        PetCapability::Landing => Some(Borrow::Motion(PetCapability::Spark)),
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PetAnimationFrame {
    pub index: usize,
    pub duration: f64,
}

impl PetAnimationFrame {
    /// A zero-length frame would spin the player, so the floor is not optional.
    pub fn new(index: usize, duration: f64) -> Self {
        Self {
            index,
            duration: crate::geometry::swift_max(0.001, duration),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PetAnimationTrack {
    pub name: String,
    pub frames: Vec<PetAnimationFrame>,
    pub loops: bool,
    pub fallback: Option<String>,
}

impl PetAnimationTrack {
    pub fn new(name: impl Into<String>, frames: Vec<PetAnimationFrame>, loops: bool) -> Self {
        Self {
            name: name.into(),
            frames,
            loops,
            fallback: None,
        }
    }
}

/// The nine rows as Petdex paces them, for a package that declares none of its
/// own timings.
pub fn standard_tracks(columns: usize) -> BTreeMap<String, PetAnimationTrack> {
    fn track(
        name: &str,
        row: usize,
        columns: usize,
        durations_ms: &[u32],
    ) -> PetAnimationTrack {
        let frames = durations_ms
            .iter()
            .enumerate()
            .map(|(offset, milliseconds)| {
                PetAnimationFrame::new(row * columns + offset, *milliseconds as f64 / 1_000.0)
            })
            .collect();
        PetAnimationTrack::new(name, frames, true)
    }

    let rows: [(&str, usize, &[u32]); 9] = [
        ("idle", 0, &[280, 110, 110, 140, 140, 320]),
        ("running-right", 1, &[120, 120, 120, 120, 120, 120, 120, 220]),
        ("running-left", 2, &[120, 120, 120, 120, 120, 120, 120, 220]),
        ("waving", 3, &[140, 140, 140, 280]),
        ("jumping", 4, &[140, 140, 140, 140, 280]),
        ("failed", 5, &[140, 140, 140, 140, 140, 140, 140, 240]),
        ("waiting", 6, &[150, 150, 150, 150, 150, 260]),
        ("running", 7, &[120, 120, 120, 120, 120, 220]),
        ("review", 8, &[150, 150, 150, 150, 150, 280]),
    ];
    rows.into_iter()
        .map(|(name, row, durations)| {
            (name.to_string(), track(name, row, columns, durations))
        })
        .collect()
}

/// How a capability was answered, so a caller can tell a package's own artwork
/// from something borrowed to stand in for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provenance {
    /// The package authors this capability, under one of its own names.
    Authored,
    /// Answered by a related capability's artwork. Expected, not a defect: the
    /// Petdex contract has nine agent states and no notion of a pet that
    /// sleeps, sits, or gets picked up.
    Substituted(PetCapability),
    /// Nothing related existed, so the resting pose is standing in.
    Placeholder,
}

/// What a pet can and cannot show.
///
/// The counts exist because a package with one declared animation renders that
/// animation for every state and looks, from outside, like a pet whose
/// behaviour is broken.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Coverage {
    pub authored: Vec<PetCapability>,
    pub substituted: Vec<PetCapability>,
    pub placeholder: Vec<PetCapability>,
}

impl Coverage {
    pub fn total(&self) -> usize {
        PET_CAPABILITIES.len()
    }

    /// Substituted counts as covered: the Petdex contract has no sleeping or
    /// sitting pet, so borrowing for those is the design, not a fault.
    pub fn covered(&self) -> usize {
        self.authored.len() + self.substituted.len()
    }

    pub fn is_complete(&self) -> bool {
        self.placeholder.is_empty()
    }
}

#[derive(Debug, Clone, Default)]
pub struct AnimationResolver {
    /// Ordered, not hashed. The placeholder path falls back to "whatever track
    /// exists" when a package declares no `idle`, and picking that out of a
    /// Swift dictionary meant a package with two rows drew a different one on
    /// every launch.
    pub tracks: BTreeMap<String, PetAnimationTrack>,
    pub explicit_behaviors: BTreeMap<String, String>,
}

impl AnimationResolver {
    pub fn new(
        tracks: BTreeMap<String, PetAnimationTrack>,
        explicit_behaviors: BTreeMap<String, String>,
    ) -> Self {
        Self {
            tracks,
            explicit_behaviors,
        }
    }

    pub fn resolve(&self, capability: PetCapability) -> Option<&PetAnimationTrack> {
        self.resolution(capability).0
    }

    /// Resolves along meaning rather than down one flat list of names.
    ///
    /// Each capability names the one it degrades into, so improving a
    /// substitution improves everything downstream of it: teaching `sit` about
    /// a seated pose also seats the pet when it sleeps.
    pub fn resolution(
        &self,
        capability: PetCapability,
    ) -> (Option<&PetAnimationTrack>, Provenance) {
        let mut seen: Vec<PetCapability> = Vec::new();
        let mut step = capability;
        while !seen.contains(&step) {
            seen.push(step);
            for name in self.candidates(step) {
                if let Some(resolved) = self.track_named(&name, &mut Vec::new()) {
                    let provenance = if step == capability {
                        Provenance::Authored
                    } else {
                        Provenance::Substituted(step)
                    };
                    return (Some(resolved), provenance);
                }
            }
            match borrows(step) {
                Some(next) => step = next.capability(),
                None => break,
            }
        }
        // A package with nothing recognisable still has to render something, or
        // the pet disappears. Callers are told, because a silent stand-in for
        // every state is exactly the failure this reports.
        (
            self.tracks
                .get("idle")
                .or_else(|| self.tracks.values().next()),
            Provenance::Placeholder,
        )
    }

    pub fn coverage(&self) -> Coverage {
        let mut result = Coverage::default();
        for capability in PET_CAPABILITIES {
            match self.resolution(capability).1 {
                Provenance::Authored => result.authored.push(capability),
                Provenance::Substituted(_) => result.substituted.push(capability),
                Provenance::Placeholder => result.placeholder.push(capability),
            }
        }
        result
    }

    fn track_named(&self, name: &str, visited: &mut Vec<String>) -> Option<&PetAnimationTrack> {
        if visited.iter().any(|seen| seen == name) {
            return None;
        }
        let track = self.tracks.get(name)?;
        if !track.frames.is_empty() {
            return Some(track);
        }
        let fallback = track.fallback.clone()?;
        visited.push(name.to_string());
        self.track_named(&fallback, visited)
    }

    /// Every track name that answers for one capability, best first. Generated
    /// from the capability's own declaration rather than written out per
    /// capability: the flat table this replaced listed `jumping` ahead of
    /// `waving` under `celebrate`, which is backwards.
    fn candidates(&self, capability: PetCapability) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        if let Some(mapped) = self.explicit_behaviors.get(capability_name(capability)) {
            names.push(mapped.clone());
        }
        names.extend(authored_names(capability).iter().map(|name| name.to_string()));
        if let Some(state) = petdex_state(capability) {
            names.extend(state.track_names().iter().map(|name| name.to_string()));
        }
        names
    }
}

/// The `rawValue` of the Swift capability, which is the key a manifest's
/// `behaviors` map uses.
pub fn capability_name(capability: PetCapability) -> &'static str {
    match capability {
        PetCapability::Idle => "idle",
        PetCapability::MoveLeft => "moveLeft",
        PetCapability::MoveRight => "moveRight",
        PetCapability::Sit => "sit",
        PetCapability::Sleep => "sleep",
        PetCapability::Work => "work",
        PetCapability::Observe => "observe",
        PetCapability::Gaze => "gaze",
        PetCapability::Paw => "paw",
        PetCapability::Spark => "spark",
        PetCapability::Celebrate => "celebrate",
        PetCapability::Fail => "fail",
        PetCapability::Stretch => "stretch",
        PetCapability::Caught => "caught",
        PetCapability::Dragged => "dragged",
        PetCapability::Landing => "landing",
    }
}

/// Where the pet looks when it is watching the cursor, as a frame index.
///
/// Ported from `PetAsset.lookFrameIndex`. Sixteen directions across two extra
/// rows: the first eight on row nine, the rest on row ten. A sheet without
/// those rows has no answer, and the player then plays the track instead.
pub fn look_frame_index(degrees: f64, columns: usize, rows: usize) -> Option<usize> {
    if !(rows >= 11 && columns >= 8) {
        return None;
    }
    let normalized = degrees % 360.0 + if degrees < 0.0 { 360.0 } else { 0.0 };
    // Swift's `rounded()` is half-away-from-zero, which `f64::round` matches.
    let direction = ((normalized / 22.5).round() as i64).rem_euclid(16) as usize;
    if direction < 8 {
        Some(9 * columns + direction)
    } else {
        Some(10 * columns + (direction - 8))
    }
}

/// Which frame of which track is on screen right now.
#[derive(Debug, Clone)]
pub struct PetAnimationPlayer {
    capability: PetCapability,
    track: PetAnimationTrack,
    track_name: String,
    current_frame_index: usize,
    frame_cursor: usize,
    elapsed_in_frame: f64,
    /// A gaze frame chosen by angle rather than by time. While one is set the
    /// clock does not advance the track: the pet is holding a look, not
    /// playing an animation.
    look_override: Option<usize>,
}

impl PetAnimationPlayer {
    pub fn new(resolver: &AnimationResolver) -> Self {
        let initial = resolver.resolve(PetCapability::Idle).cloned().unwrap_or_else(|| {
            PetAnimationTrack::new("fallback", vec![PetAnimationFrame::new(0, 1.0)], true)
        });
        let index = initial.frames.first().map_or(0, |frame| frame.index);
        Self {
            capability: PetCapability::Idle,
            track_name: initial.name.clone(),
            track: initial,
            current_frame_index: index,
            frame_cursor: 0,
            elapsed_in_frame: 0.0,
            look_override: None,
        }
    }

    pub fn capability(&self) -> PetCapability {
        self.capability
    }

    pub fn track_name(&self) -> &str {
        &self.track_name
    }

    pub fn current_frame_index(&self) -> usize {
        self.current_frame_index
    }

    pub fn set_capability(&mut self, resolver: &AnimationResolver, capability: PetCapability) {
        if capability == self.capability {
            return;
        }
        self.capability = capability;
        self.look_override = None;
        let Some(resolved) = resolver.resolve(capability) else { return };
        self.track = resolved.clone();
        self.track_name = resolved.name.clone();
        self.frame_cursor = 0;
        self.elapsed_in_frame = 0.0;
        self.current_frame_index = resolved.frames.first().map_or(0, |frame| frame.index);
    }

    /// `look` is the frame the pet's own atlas offers for that angle, which the
    /// caller resolves because only it has the sheet.
    pub fn set_look_frame(&mut self, look: Option<usize>) {
        match look {
            Some(index) => {
                self.look_override = Some(index);
                self.current_frame_index = index;
            }
            None => {
                self.look_override = None;
                self.current_frame_index = self
                    .track
                    .frames
                    .get(self.frame_cursor)
                    .map_or(0, |frame| frame.index);
            }
        }
    }

    pub fn update(&mut self, delta_time: f64) {
        if self.look_override.is_some() || self.track.frames.is_empty() {
            return;
        }
        self.elapsed_in_frame += crate::geometry::swift_max(0.0, delta_time);

        // The safety counter is not decoration: a caller that hands over a
        // minute of elapsed time would otherwise walk a six-frame loop ten
        // thousand times to arrive where sixty steps would have.
        let mut safety = 0;
        while self.elapsed_in_frame >= self.track.frames[self.frame_cursor].duration && safety < 64
        {
            self.elapsed_in_frame -= self.track.frames[self.frame_cursor].duration;
            if self.frame_cursor + 1 < self.track.frames.len() {
                self.frame_cursor += 1;
            } else if self.track.loops {
                self.frame_cursor = 0;
            } else {
                self.elapsed_in_frame = 0.0;
                break;
            }
            safety += 1;
        }
        self.current_frame_index = self.track.frames[self.frame_cursor].index;
    }
}
