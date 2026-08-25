// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation

public struct PetAnimationPlayer {
    public private(set) var capability: PetCapability = .idle
    public private(set) var trackName: String
    public private(set) var currentFrameIndex: Int

    private let asset: PetAsset
    private var track: PetAnimationTrack
    private var frameCursor = 0
    private var elapsedInFrame: TimeInterval = 0
    private var lookOverride: Int?

    public init(asset: PetAsset) {
        self.asset = asset
        let initial = asset.resolver.resolve(.idle)
            ?? PetAnimationTrack(name: "fallback", frames: [PetAnimationFrame(index: 0, duration: 1)], loops: true)
        track = initial
        trackName = initial.name
        currentFrameIndex = initial.frames.first?.index ?? 0
    }

    public mutating func setCapability(_ newCapability: PetCapability) {
        guard newCapability != capability else { return }
        capability = newCapability
        lookOverride = nil
        guard let resolved = asset.resolver.resolve(newCapability) else { return }
        track = resolved
        trackName = resolved.name
        frameCursor = 0
        elapsedInFrame = 0
        currentFrameIndex = resolved.frames.first?.index ?? 0
    }

    public mutating func setLookDirection(degrees: Double?) {
        guard let degrees else {
            lookOverride = nil
            currentFrameIndex = track.frames[safe: frameCursor]?.index ?? 0
            return
        }
        lookOverride = asset.lookFrameIndex(degrees: degrees)
        if let lookOverride { currentFrameIndex = lookOverride }
    }

    public mutating func update(deltaTime: TimeInterval) {
        guard lookOverride == nil, !track.frames.isEmpty else { return }
        elapsedInFrame += max(0, deltaTime)

        var safety = 0
        while elapsedInFrame >= track.frames[frameCursor].duration, safety < 64 {
            elapsedInFrame -= track.frames[frameCursor].duration
            if frameCursor + 1 < track.frames.count {
                frameCursor += 1
            } else if track.loops {
                frameCursor = 0
            } else {
                elapsedInFrame = 0
                break
            }
            safety += 1
        }
        currentFrameIndex = track.frames[frameCursor].index
    }
}

private extension Collection {
    subscript(safe index: Index) -> Element? {
        indices.contains(index) ? self[index] : nil
    }
}
