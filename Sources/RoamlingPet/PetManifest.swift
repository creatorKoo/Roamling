// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation

public struct PetFrameManifest: Codable, Equatable, Sendable {
    public let width: Int
    public let height: Int
    public let columns: Int
    public let rows: Int

    public init(width: Int, height: Int, columns: Int, rows: Int) {
        self.width = width
        self.height = height
        self.columns = columns
        self.rows = rows
    }
}

public struct PetAnimationManifest: Codable, Equatable, Sendable {
    public let frames: [Int]
    public let fps: Double?
    public let loop: Bool?
    public let fallback: String?

    public init(frames: [Int], fps: Double? = nil, loop: Bool? = nil, fallback: String? = nil) {
        self.frames = frames
        self.fps = fps
        self.loop = loop
        self.fallback = fallback
    }
}

public struct PetManifest: Codable, Equatable, Sendable {
    public let id: String
    public let displayName: String
    public let description: String
    public let spriteVersionNumber: Int?
    public let spritesheetPath: String
    public let frame: PetFrameManifest?
    public let animations: [String: PetAnimationManifest]?

    public init(
        id: String,
        displayName: String,
        description: String,
        spriteVersionNumber: Int? = nil,
        spritesheetPath: String,
        frame: PetFrameManifest? = nil,
        animations: [String: PetAnimationManifest]? = nil
    ) {
        self.id = id
        self.displayName = displayName
        self.description = description
        self.spriteVersionNumber = spriteVersionNumber
        self.spritesheetPath = spritesheetPath
        self.frame = frame
        self.animations = animations
    }
}

public struct RoamlingManifest: Codable, Equatable, Sendable {
    public let schemaVersion: Int
    public let behaviors: [String: String]

    public init(schemaVersion: Int = 1, behaviors: [String: String]) {
        self.schemaVersion = schemaVersion
        self.behaviors = behaviors
    }
}

public enum PetLoadError: LocalizedError, Equatable {
    case missingManifest(String)
    case malformedManifest(String)
    case unsupportedSpriteVersion(Int)
    case unsafeSpritesheetPath(String)
    case missingSpritesheet(String)
    case spritesheetTooLarge(Int)
    case unsupportedImage(String)
    case invalidFrameLayout(String)

    public var errorDescription: String? {
        switch self {
        case let .missingManifest(path):
            "Missing pet.json at \(path)"
        case let .malformedManifest(message):
            "Invalid pet.json: \(message)"
        case let .unsupportedSpriteVersion(version):
            "Unsupported spriteVersionNumber \(version); expected 1 or 2"
        case let .unsafeSpritesheetPath(path):
            "spritesheetPath must stay inside the pet package: \(path)"
        case let .missingSpritesheet(path):
            "Missing spritesheet at \(path)"
        case let .spritesheetTooLarge(bytes):
            "Spritesheet is too large (\(bytes) bytes)"
        case let .unsupportedImage(path):
            "Could not decode PNG/WebP spritesheet at \(path)"
        case let .invalidFrameLayout(message):
            "Invalid sprite frame layout: \(message)"
        }
    }
}
