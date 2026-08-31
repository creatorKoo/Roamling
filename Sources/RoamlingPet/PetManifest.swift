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

/// The optional `roamling.json` beside a Petdex package.
///
/// Roamling needs pictures Petdex has no word for -- sleeping, being carried,
/// watching the cursor -- and the nine-row contract has neither names nor cells
/// for them. They live here, on a sheet of their own, so `pet.json` and
/// `spritesheet.webp` stay exactly the contract and nothing has to be argued
/// about when the package is submitted to the gallery.
///
/// Frames on the extension sheet are addressed by continuing the index past the
/// end of the package grid: with an 8x9 package, index 72 is the extension
/// sheet's first cell. A track may therefore mix the two -- `landing` reuses the
/// package's jump frames and needs no drawing of its own.
///
/// A package without this file is the important compatibility path and behaves
/// as it always did.
public struct RoamlingManifest: Codable, Equatable, Sendable {
    public static let currentSchemaVersion = 1

    public let schemaVersion: Int
    /// The extension sheet, relative to the package directory. Omit it when the
    /// file only remaps behaviours onto frames the package already has.
    public let spritesheetPath: String?
    /// The extension sheet's grid. Cell size comes from the package, so the two
    /// sheets always share one cell geometry.
    public let frame: PetExtensionGrid?
    public let behaviors: [String: String]
    public let animations: [String: PetAnimationManifest]?

    public init(
        schemaVersion: Int = RoamlingManifest.currentSchemaVersion,
        spritesheetPath: String? = nil,
        frame: PetExtensionGrid? = nil,
        behaviors: [String: String] = [:],
        animations: [String: PetAnimationManifest]? = nil
    ) {
        self.schemaVersion = schemaVersion
        self.spritesheetPath = spritesheetPath
        self.frame = frame
        self.behaviors = behaviors
        self.animations = animations
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        schemaVersion = try container.decode(Int.self, forKey: .schemaVersion)
        spritesheetPath = try container.decodeIfPresent(String.self, forKey: .spritesheetPath)
        frame = try container.decodeIfPresent(PetExtensionGrid.self, forKey: .frame)
        behaviors = try container.decodeIfPresent([String: String].self, forKey: .behaviors) ?? [:]
        animations = try container.decodeIfPresent(
            [String: PetAnimationManifest].self,
            forKey: .animations
        )
    }
}

/// The extension sheet's grid. Only the column and row counts: the cell size is
/// the package's, because a pet drawn at two scales is not one pet.
public struct PetExtensionGrid: Codable, Equatable, Sendable {
    public let columns: Int
    public let rows: Int

    public init(columns: Int, rows: Int) {
        self.columns = columns
        self.rows = rows
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
    case unsupportedExtensionSchema(Int)

    public var errorDescription: String? {
        switch self {
        case let .missingManifest(path):
            "Missing pet.json at \(path)"
        case let .malformedManifest(message):
            "Invalid pet.json: \(message)"
        case let .unsupportedSpriteVersion(version):
            "Unsupported spriteVersionNumber \(version); expected 1 or 2"
        case let .unsupportedExtensionSchema(version):
            "Unsupported roamling.json schemaVersion \(version); expected \(RoamlingManifest.currentSchemaVersion)"
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
