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
    private static let gradientReference = 0.10
    /// A standard deviation at or above this reads as fully busy.
    private static let spreadReference = 0.20
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
}
