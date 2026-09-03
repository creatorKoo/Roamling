// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Ported from `Sources/RoamlingCore/DesktopWorld.swift`.
//!
//! Only what the ported units read is here. Windows, focus and luminance stay
//! on the Swift side until the units that use them come across.

use crate::geometry::{clamped, swift_max, WorldPoint, WorldRect, WorldSize};

#[derive(Debug, Clone, PartialEq)]
pub struct DisplaySnapshot {
    pub id: String,
    pub name: String,
    pub frame: WorldRect,
    pub visible_frame: WorldRect,
    pub scale: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SafeZone {
    pub frame: WorldRect,
    pub score: f64,
    pub confidence: f64,
    pub reason: String,
}

impl SafeZone {
    pub fn new(frame: WorldRect, score: f64, confidence: f64, reason: impl Into<String>) -> Self {
        Self {
            frame,
            score,
            // Swift's initializer clamps this, and callers depend on it.
            confidence: crate::geometry::clamped(confidence, 0.0, 1.0),
            reason: reason.into(),
        }
    }
}

/// Where a source says the user's work is, at whatever confidence the platform
/// could manage. Coarser than a focus snapshot and available without any
/// permission, which is what the pet falls back to when accessibility is off.
#[derive(Debug, Clone, PartialEq)]
pub struct LocationHint {
    pub approximate_region: Option<WorldRect>,
    pub confidence: f64,
}

impl LocationHint {
    pub fn new(approximate_region: Option<WorldRect>, confidence: f64) -> Self {
        Self { approximate_region, confidence: clamped(confidence, 0.0, 1.0) }
    }
}

/// What accessibility can say about the focused window, when the user granted
/// it. Empty rects are dropped on the way in, so a `Some` here means something
/// real was measured.
#[derive(Debug, Clone, PartialEq)]
pub struct FocusSnapshot {
    pub window_frame: Option<WorldRect>,
    pub focused_element_frame: Option<WorldRect>,
    pub caret_frame: Option<WorldRect>,
    pub confidence: f64,
}

impl FocusSnapshot {
    pub fn new(
        window_frame: Option<WorldRect>,
        focused_element_frame: Option<WorldRect>,
        caret_frame: Option<WorldRect>,
        confidence: f64,
    ) -> Self {
        Self {
            window_frame: window_frame.filter(|rect| !rect.is_empty()),
            focused_element_frame: focused_element_frame.filter(|rect| !rect.is_empty()),
            caret_frame: Self::usable_caret(caret_frame),
            confidence: clamped(confidence, 0.0, 1.0),
        }
    }

    /// An insertion point legitimately reports zero width, so dropping empty
    /// rects would discard exactly the one placement cares about most. Widen it
    /// into a usable obstacle instead.
    fn usable_caret(rect: Option<WorldRect>) -> Option<WorldRect> {
        let rect = rect?;
        if !(rect.size.width > 0.0 || rect.size.height > 0.0) {
            return None;
        }
        Some(WorldRect::new(
            rect.min_x(),
            rect.min_y(),
            swift_max(rect.size.width, 2.0),
            swift_max(rect.size.height, 2.0),
        ))
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DesktopWorldSnapshot {
    pub displays: Vec<DisplaySnapshot>,
    pub safe_zones: Vec<SafeZone>,
    pub focus: Option<FocusSnapshot>,
    /// Downsampled luminance for the display being placed on, when the user
    /// turned visual placement on.
    pub luminance: Option<crate::emptiness::LuminanceField>,
}

impl DesktopWorldSnapshot {
    pub fn new(displays: Vec<DisplaySnapshot>, safe_zones: Vec<SafeZone>) -> Self {
        Self { displays, safe_zones, focus: None, luminance: None }
    }

    pub fn display_containing(&self, point: WorldPoint) -> Option<&DisplaySnapshot> {
        self.displays.iter().find(|display| display.frame.contains(point))
    }

    /// Swift's `min(by:)` keeps the first of equal elements, so a point exactly
    /// between two displays lands on the earlier one -- and the OS order is
    /// what makes that stable.
    pub fn nearest_display(&self, point: WorldPoint) -> Option<&DisplaySnapshot> {
        first_minimum(&self.displays, |display| display.frame.distance(point))
    }

    pub fn clamp(&self, point: WorldPoint, object_size: WorldSize) -> WorldPoint {
        let containing = self.displays.iter().find(|display| {
            display
                .visible_frame
                .inset_by(object_size.width / 2.0, object_size.height / 2.0)
                .contains(point)
        });
        if let Some(display) = containing {
            return display.visible_frame.clamped_center(point, object_size);
        }
        match first_minimum(&self.displays, |display| display.visible_frame.distance(point)) {
            Some(nearest) => nearest.visible_frame.clamped_center(point, object_size),
            None => point,
        }
    }
}

/// Swift's `min(by:)`: scan in order, replace only on a strict improvement, so
/// the first of several equal elements wins. Written out because getting this
/// backwards is invisible until two things tie, and then it is a coin flip.
pub fn first_minimum<T>(items: &[T], key: impl Fn(&T) -> f64) -> Option<&T> {
    let mut best: Option<(&T, f64)> = None;
    for item in items {
        let value = key(item);
        match best {
            Some((_, current)) if !(value < current) => {}
            _ => best = Some((item, value)),
        }
    }
    best.map(|(item, _)| item)
}

/// Swift's `max(by:)`, which keeps the *last* of equal elements -- the opposite
/// of `min(by:)`. W0m.2's port only matched once it reproduced this.
pub fn last_maximum<T>(items: &[T], is_less: impl Fn(&T, &T) -> bool) -> Option<&T> {
    let mut best: Option<&T> = None;
    for item in items {
        match best {
            Some(current) if !is_less(current, item) => {}
            _ => best = Some(item),
        }
    }
    best
}
