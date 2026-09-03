// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Ported from `Sources/RoamlingCore/Geometry.swift`, deliberately line for
//! line. Where Swift's spelling and Rust's differ the Swift behaviour wins --
//! `clamped` is `min(max(v, lo), hi)` rather than `f64::clamp`, which panics
//! when the bounds cross; `WorldVector / 0` is zero rather than infinity.

/// Swift's `max(_:_:)`, which is `y >= x ? y : x` -- it returns the *second*
/// argument when the two compare equal.
///
/// The name records where the tie-breaking comes from, and is a warning: do
/// not swap these for `f64::min`/`f64::max`. Later units clamp with them too,
/// and a silent sign flip on zero moves the pet.
///
/// That is visible for signed zero: `max(0, -0.0)` is `-0.0`, and Rust's
/// `f64::max` documents itself as returning either one non-deterministically
/// in that case. The differential fixture caught this on its first run.
#[inline]
pub fn swift_max(x: f64, y: f64) -> f64 {
    if y >= x { y } else { x }
}

/// Swift's `min(_:_:)`, which is `y < x ? y : x` -- note the strict `<`. It
/// breaks ties toward the *first* argument, the opposite way from `max`. The
/// stdlib says so in a comment and nothing else does; the fixture found it.
#[inline]
pub fn swift_min(x: f64, y: f64) -> f64 {
    if y < x { y } else { x }
}

/// Swift's `Double.clamped(to:)`, which is `min(max(self, lower), upper)`.
///
/// Not `f64::clamp`: that panics when the bounds cross, and this returns the
/// upper bound -- which `RuntimeTuning` relies on, since one of its ceilings
/// moves with another field.
pub fn clamped(value: f64, lower: f64, upper: f64) -> f64 {
    swift_min(swift_max(value, lower), upper)
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct WorldPoint {
    pub x: f64,
    pub y: f64,
}

impl WorldPoint {
    pub const ZERO: WorldPoint = WorldPoint { x: 0.0, y: 0.0 };

    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn distance(self, other: WorldPoint) -> f64 {
        (other.x - self.x).hypot(other.y - self.y)
    }

    pub fn offset(self, by: WorldVector) -> WorldPoint {
        WorldPoint::new(self.x + by.dx, self.y + by.dy)
    }

    pub fn pulled_back(self, by: WorldVector) -> WorldPoint {
        WorldPoint::new(self.x - by.dx, self.y - by.dy)
    }

    /// The vector from `other` to `self`, which is Swift's `point - point`.
    pub fn vector_from(self, other: WorldPoint) -> WorldVector {
        WorldVector::new(self.x - other.x, self.y - other.y)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct WorldVector {
    pub dx: f64,
    pub dy: f64,
}

impl WorldVector {
    pub const ZERO: WorldVector = WorldVector { dx: 0.0, dy: 0.0 };

    pub fn new(dx: f64, dy: f64) -> Self {
        Self { dx, dy }
    }

    pub fn length(self) -> f64 {
        self.dx.hypot(self.dy)
    }

    /// Zero below a millionth, so a vector that is only floating-point noise
    /// does not get scaled up into a direction.
    pub fn normalized(self) -> WorldVector {
        let magnitude = self.length();
        if magnitude <= 0.000_001 {
            return WorldVector::ZERO;
        }
        self.divided_by(magnitude)
    }

    pub fn dot(self, other: WorldVector) -> f64 {
        self.dx * other.dx + self.dy * other.dy
    }

    pub fn limited(self, maximum: f64) -> WorldVector {
        if !(maximum >= 0.0) || !(self.length() > maximum) || !(self.length() > 0.0) {
            return self;
        }
        self.normalized().scaled(maximum)
    }

    /// Steps toward `target` by at most `maximum_delta`. Returns `target`
    /// itself when the step would overshoot, which is what stops a controller
    /// oscillating around its destination.
    pub fn moved_toward(self, target: WorldVector, maximum_delta: f64) -> WorldVector {
        let delta = target.minus(self);
        if !(delta.length() > maximum_delta) || !(maximum_delta >= 0.0) {
            return target;
        }
        self.plus(delta.normalized().scaled(maximum_delta))
    }

    pub fn plus(self, other: WorldVector) -> WorldVector {
        WorldVector::new(self.dx + other.dx, self.dy + other.dy)
    }

    pub fn minus(self, other: WorldVector) -> WorldVector {
        WorldVector::new(self.dx - other.dx, self.dy - other.dy)
    }

    pub fn negated(self) -> WorldVector {
        WorldVector::new(-self.dx, -self.dy)
    }

    pub fn scaled(self, factor: f64) -> WorldVector {
        WorldVector::new(self.dx * factor, self.dy * factor)
    }

    /// Zero rather than infinity when dividing by zero: callers treat the
    /// result as a direction, and an infinite direction is not one.
    pub fn divided_by(self, divisor: f64) -> WorldVector {
        if divisor == 0.0 {
            return WorldVector::ZERO;
        }
        WorldVector::new(self.dx / divisor, self.dy / divisor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct WorldSize {
    pub width: f64,
    pub height: f64,
}

impl WorldSize {
    pub const ZERO: WorldSize = WorldSize { width: 0.0, height: 0.0 };

    pub fn new(width: f64, height: f64) -> Self {
        Self { width, height }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldRect {
    pub origin: WorldPoint,
    pub size: WorldSize,
}

impl WorldRect {
    /// A negative extent becomes zero, so no rect is inside out. The origin is
    /// kept as given.
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            origin: WorldPoint::new(x, y),
            size: WorldSize::new(swift_max(0.0, width), swift_max(0.0, height)),
        }
    }

    pub fn from_parts(origin: WorldPoint, size: WorldSize) -> Self {
        Self::new(origin.x, origin.y, size.width, size.height)
    }

    pub fn min_x(&self) -> f64 { self.origin.x }
    pub fn min_y(&self) -> f64 { self.origin.y }
    pub fn max_x(&self) -> f64 { self.origin.x + self.size.width }
    pub fn max_y(&self) -> f64 { self.origin.y + self.size.height }
    pub fn mid_x(&self) -> f64 { self.origin.x + self.size.width / 2.0 }
    pub fn mid_y(&self) -> f64 { self.origin.y + self.size.height / 2.0 }

    pub fn center(&self) -> WorldPoint {
        WorldPoint::new(self.mid_x(), self.mid_y())
    }

    pub fn is_empty(&self) -> bool {
        self.size.width <= 0.0 || self.size.height <= 0.0
    }

    pub fn contains(&self, point: WorldPoint) -> bool {
        point.x >= self.min_x()
            && point.x <= self.max_x()
            && point.y >= self.min_y()
            && point.y <= self.max_y()
    }

    pub fn intersects(&self, other: &WorldRect, tolerance: f64) -> bool {
        self.max_x() + tolerance >= other.min_x()
            && other.max_x() + tolerance >= self.min_x()
            && self.max_y() + tolerance >= other.min_y()
            && other.max_y() + tolerance >= self.min_y()
    }

    pub fn inset_by(&self, dx: f64, dy: f64) -> WorldRect {
        let inset_x = swift_min(swift_max(0.0, dx), self.size.width / 2.0);
        let inset_y = swift_min(swift_max(0.0, dy), self.size.height / 2.0);
        WorldRect::new(
            self.min_x() + inset_x,
            self.min_y() + inset_y,
            self.size.width - inset_x * 2.0,
            self.size.height - inset_y * 2.0,
        )
    }

    pub fn closest_point(&self, point: WorldPoint) -> WorldPoint {
        WorldPoint::new(
            swift_min(swift_max(point.x, self.min_x()), self.max_x()),
            swift_min(swift_max(point.y, self.min_y()), self.max_y()),
        )
    }

    pub fn distance(&self, point: WorldPoint) -> f64 {
        self.closest_point(point).distance(point)
    }

    pub fn clamped_center(&self, point: WorldPoint, object_size: WorldSize) -> WorldPoint {
        self.inset_by(object_size.width / 2.0, object_size.height / 2.0)
            .closest_point(point)
    }

    pub fn union(&self, other: &WorldRect) -> WorldRect {
        let min_x = swift_min(self.min_x(), other.min_x());
        let min_y = swift_min(self.min_y(), other.min_y());
        WorldRect::new(
            min_x,
            min_y,
            swift_max(self.max_x(), other.max_x()) - min_x,
            swift_max(self.max_y(), other.max_y()) - min_y,
        )
    }
}
