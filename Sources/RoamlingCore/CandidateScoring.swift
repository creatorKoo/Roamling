// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation

public struct PositionCandidate: Codable, Hashable, Sendable {
    public let point: WorldPoint
    public let visualEmptyScore: Double
    public let distanceFromCaret: Double
    public let distanceFromControls: Double
    public let edgePreference: Double
    public let stabilityScore: Double
    public let contextPreference: Double
    public let petComfort: Double
    public let pointerProximity: Double
    public let obstructionPenalty: Double

    public init(
        point: WorldPoint,
        visualEmptyScore: Double = 0,
        distanceFromCaret: Double = 0,
        distanceFromControls: Double = 0,
        edgePreference: Double = 0,
        stabilityScore: Double = 0,
        contextPreference: Double = 0,
        petComfort: Double = 0,
        pointerProximity: Double = 0,
        obstructionPenalty: Double = 0
    ) {
        self.point = point
        self.visualEmptyScore = visualEmptyScore
        self.distanceFromCaret = distanceFromCaret
        self.distanceFromControls = distanceFromControls
        self.edgePreference = edgePreference
        self.stabilityScore = stabilityScore
        self.contextPreference = contextPreference
        self.petComfort = petComfort
        self.pointerProximity = pointerProximity
        self.obstructionPenalty = obstructionPenalty
    }

    public var score: Double {
        visualEmptyScore
            + distanceFromCaret
            + distanceFromControls
            + edgePreference
            + stabilityScore
            + contextPreference
            + petComfort
            - pointerProximity
            - obstructionPenalty
    }
}

public enum CandidatePositionScorer {
    public static func best(from candidates: [PositionCandidate]) -> PositionCandidate? {
        candidates.max { lhs, rhs in
            if lhs.score == rhs.score {
                if lhs.stabilityScore == rhs.stabilityScore {
                    return lhs.point.x > rhs.point.x
                }
                return lhs.stabilityScore < rhs.stabilityScore
            }
            return lhs.score < rhs.score
        }
    }
}
