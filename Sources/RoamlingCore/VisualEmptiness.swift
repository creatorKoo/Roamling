// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation

/// A downsampled grayscale view of part of the desktop.
///
/// Captured pixels never leave the capture adapter as an image. They arrive
/// here as a small grid of luminance samples covering a known world rect, get
/// scored, and are dropped. Nothing is written to disk or logged, and no text
/// is recognised — this type cannot express what was on screen, only how busy
/// each region looked.
public struct LuminanceField: Sendable, Hashable {
    public let bounds: WorldRect
    public let columns: Int
    public let rows: Int
    /// Row-major, top-left first, each clamped to `0...1`.
    public let samples: [Double]

    public init?(bounds: WorldRect, columns: Int, rows: Int, samples: [Double]) {
        guard columns > 0, rows > 0,
              !bounds.isEmpty,
              samples.count == columns * rows else { return nil }
        self.bounds = bounds
        self.columns = columns
        self.rows = rows
        self.samples = samples.map { $0.clamped(to: 0...1) }
    }

    public func sample(column: Int, row: Int) -> Double? {
        guard column >= 0, column < columns, row >= 0, row < rows else { return nil }
        return samples[row * columns + column]
    }

    public var cellSize: WorldSize {
        WorldSize(
            width: bounds.size.width / Double(columns),
            height: bounds.size.height / Double(rows)
        )
    }
}

/// Scores how visually empty a candidate region looks.
///
/// Text, code, and dense controls all raise the local gradient; photographs and
/// busy imagery raise the spread. Weighting the gradient higher keeps a smooth
/// wallpaper gradient — which is fine to sit on — from scoring as busy.
public enum VisualEmptiness {
    /// A mean neighbour difference at or above this reads as fully busy.
    ///
    /// Downsampling averages a couple of glyphs into one sample, so a page of
    /// text arrives far flatter than it looks: measured against rendered
    /// terminal output it lands near 0.025, not near 0.5. The first calibration
    /// used 0.10 and scored solid body text at 0.79 — indistinguishable from
    /// wallpaper — which let the pet park on the user's work.
    private static let gradientReference = 0.02
    /// A standard deviation at or above this reads as fully busy.
    private static let spreadReference = 0.05
    private static let gradientWeight = 0.7

    /// Returns `0...1`, where 1 is flat and safe to sit on.
    ///
    /// Returns nil when the region does not overlap enough of the field to
    /// judge, so callers can fall back instead of trusting a guess made from
    /// two samples. Reference constants are calibrated against a downsampled
    /// field and are the first thing to revisit if real screens score wrong.
    public static func score(of rect: WorldRect, in field: LuminanceField) -> Double? {
        let cell = field.cellSize
        guard cell.width > 0, cell.height > 0 else { return nil }

        let firstColumn = max(0, Int(((rect.minX - field.bounds.minX) / cell.width).rounded(.down)))
        let lastColumn = min(
            field.columns - 1,
            Int(((rect.maxX - field.bounds.minX) / cell.width).rounded(.up)) - 1
        )
        let firstRow = max(0, Int(((rect.minY - field.bounds.minY) / cell.height).rounded(.down)))
        let lastRow = min(
            field.rows - 1,
            Int(((rect.maxY - field.bounds.minY) / cell.height).rounded(.up)) - 1
        )
        guard lastColumn - firstColumn >= 1, lastRow - firstRow >= 1 else { return nil }

        var values: [Double] = []
        var gradientTotal = 0.0
        var gradientCount = 0
        for row in firstRow...lastRow {
            for column in firstColumn...lastColumn {
                guard let value = field.sample(column: column, row: row) else { continue }
                values.append(value)
                if column < lastColumn, let right = field.sample(column: column + 1, row: row) {
                    gradientTotal += abs(right - value)
                    gradientCount += 1
                }
                if row < lastRow, let below = field.sample(column: column, row: row + 1) {
                    gradientTotal += abs(below - value)
                    gradientCount += 1
                }
            }
        }
        guard values.count >= 4, gradientCount > 0 else { return nil }

        let meanGradient = gradientTotal / Double(gradientCount)
        let mean = values.reduce(0, +) / Double(values.count)
        let variance = values.reduce(0) { $0 + ($1 - mean) * ($1 - mean) } / Double(values.count)
        let spread = variance.squareRoot()

        let gradientTerm = min(1, meanGradient / gradientReference)
        let spreadTerm = min(1, spread / spreadReference)
        let busyness = gradientTerm * gradientWeight + spreadTerm * (1 - gradientWeight)
        return (1 - busyness).clamped(to: 0...1)
    }

    /// Picks a spot from `points` that is not sitting on content.
    ///
    /// Roaming is supposed to look aimless, so this keeps the order it was
    /// given rather than always taking the single emptiest point: the first
    /// candidate clear enough to sit on wins, and only when none of them clear
    /// the bar does the least bad one. Returns nil when the field cannot judge
    /// any of them, which leaves the caller's own pick alone.
    public static func firstComfortable(
        among points: [WorldPoint],
        objectSize: WorldSize,
        in field: LuminanceField,
        atLeast threshold: Double
    ) -> WorldPoint? {
        var best: (point: WorldPoint, score: Double)?
        for point in points {
            let frame = WorldRect(
                x: point.x - objectSize.width / 2,
                y: point.y - objectSize.height / 2,
                width: objectSize.width,
                height: objectSize.height
            )
            guard let score = score(of: frame, in: field) else { continue }
            if score >= threshold { return point }
            if score > (best?.score ?? -1) { best = (point, score) }
        }
        return best?.point
    }

    /// The frames a spot has to clear, each as a multiple of the pet's own,
    /// before it counts as having room around it. Passing the first means the
    /// pet is not on content; passing the last means there is a pet's width of
    /// nothing on every side. A spot beside a paragraph passes the first only,
    /// and until this existed that was indistinguishable from the middle of
    /// the desktop.
    public static let clearanceScales: [Double] = [1, 1.5, 2]

    /// How many of `clearanceScales` the spot clears, starting from the
    /// smallest. Zero means it is on content; nil means the field cannot judge
    /// even the pet's own frame there.
    public static func clearance(
        at point: WorldPoint,
        objectSize: WorldSize,
        in field: LuminanceField,
        atLeast threshold: Double
    ) -> Int? {
        clearanceAndScore(at: point, objectSize: objectSize, in: field, atLeast: threshold)?.tier
    }

    /// The tier plus the score at the pet's own frame, so a caller ranking the
    /// spots that failed does not pay for the same score twice.
    private static func clearanceAndScore(
        at point: WorldPoint,
        objectSize: WorldSize,
        in field: LuminanceField,
        atLeast threshold: Double
    ) -> (tier: Int, base: Double)? {
        var passed = 0
        var base = 0.0
        for (index, scale) in clearanceScales.enumerated() {
            let frame = WorldRect(
                x: point.x - objectSize.width * scale / 2,
                y: point.y - objectSize.height * scale / 2,
                width: objectSize.width * scale,
                height: objectSize.height * scale
            )
            guard let score = score(of: frame, in: field) else {
                if index == 0 { return nil }
                break
            }
            if index == 0 { base = score }
            if !(score >= threshold) { break }
            passed += 1
        }
        return (passed, base)
    }

    /// Picks the spot from `points` with the most room around it, without
    /// making roaming look calculated.
    ///
    /// The first candidate that clears every scale wins outright -- any spot
    /// with a pet's width of nothing on every side is as good as any other, and
    /// taking the first keeps the walk aimless. Only when none does is the
    /// best tier taken, and only when nothing passes at all does the least bad
    /// one come back, marked as such so the caller can decline the walk.
    public static func mostComfortable(
        among points: [WorldPoint],
        objectSize: WorldSize,
        in field: LuminanceField,
        atLeast threshold: Double
    ) -> ComfortPick {
        var bestClear: (point: WorldPoint, tier: Int)?
        var leastBad: (point: WorldPoint, base: Double)?
        for point in points {
            guard let (tier, base) = clearanceAndScore(
                at: point, objectSize: objectSize, in: field, atLeast: threshold
            ) else { continue }
            if tier >= clearanceScales.count { return .clear(point) }
            if tier >= 1 {
                if tier > (bestClear?.tier ?? 0) { bestClear = (point, tier) }
            } else if base > (leastBad?.base ?? -1) {
                leastBad = (point, base)
            }
        }
        if let bestClear { return .clear(bestClear.point) }
        if let leastBad { return .marginal(leastBad.point) }
        return .unjudged
    }
}

/// What `VisualEmptiness.mostComfortable` found.
public enum ComfortPick: Equatable, Sendable {
    /// Off content, with the most room around it of the spots offered.
    case clear(WorldPoint)
    /// Every judgeable spot is on content; this one least so.
    case marginal(WorldPoint)
    /// The field could not judge any of them.
    case unjudged
}
