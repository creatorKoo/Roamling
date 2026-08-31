// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import CoreGraphics
import Foundation
import ImageIO

public struct PetLoader {
    public static let maximumEncodedBytes = 32 * 1_024 * 1_024
    public static let maximumFrames = 256

    public init() {}

    public func load(packageAt packageURL: URL) throws -> PetAsset {
        let package = packageURL.standardizedFileURL
        let manifestURL = package.appendingPathComponent("pet.json", isDirectory: false)
        guard FileManager.default.fileExists(atPath: manifestURL.path) else {
            throw PetLoadError.missingManifest(manifestURL.path)
        }

        let manifest: PetManifest
        do {
            manifest = try JSONDecoder().decode(PetManifest.self, from: Data(contentsOf: manifestURL))
        } catch {
            throw PetLoadError.malformedManifest(error.localizedDescription)
        }

        guard !manifest.id.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
              !manifest.displayName.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw PetLoadError.malformedManifest("id and displayName must not be empty")
        }

        let spriteURL = try safeAssetURL(path: manifest.spritesheetPath, inside: package)
        guard FileManager.default.fileExists(atPath: spriteURL.path) else {
            throw PetLoadError.missingSpritesheet(spriteURL.path)
        }
        if let byteCount = try? spriteURL.resourceValues(forKeys: [.fileSizeKey]).fileSize,
           byteCount > Self.maximumEncodedBytes {
            throw PetLoadError.spritesheetTooLarge(byteCount)
        }

        guard let source = CGImageSourceCreateWithURL(spriteURL as CFURL, nil),
              let atlas = CGImageSourceCreateImageAtIndex(source, 0, nil) else {
            throw PetLoadError.unsupportedImage(spriteURL.path)
        }

        let layout = try resolveLayout(manifest: manifest, atlas: atlas)
        var warnings: [String] = []
        var tracks = StandardPetAnimations.tracks(columns: layout.columns)

        let baseFrames = layout.columns * layout.rows
        install(
            manifest.animations,
            into: &tracks,
            addressable: baseFrames,
            warnings: &warnings
        )

        var behaviorMappings: [String: String] = [:]
        var extensionAtlas: CGImage?
        var extensionColumns = 0
        var extensionRows = 0
        let extensionURL = package.appendingPathComponent("roamling.json")
        if FileManager.default.fileExists(atPath: extensionURL.path) {
            do {
                let extensionManifest = try JSONDecoder().decode(
                    RoamlingManifest.self,
                    from: Data(contentsOf: extensionURL)
                )
                guard extensionManifest.schemaVersion == RoamlingManifest.currentSchemaVersion else {
                    throw PetLoadError.unsupportedExtensionSchema(extensionManifest.schemaVersion)
                }
                behaviorMappings = extensionManifest.behaviors
                if let path = extensionManifest.spritesheetPath {
                    let grid = try requireExtensionGrid(extensionManifest.frame)
                    let url = try safeAssetURL(path: path, inside: package)
                    extensionAtlas = try loadExtensionAtlas(at: url, grid: grid, layout: layout)
                    extensionColumns = grid.columns
                    extensionRows = grid.rows
                }
                // Installed after the package's own tracks, so an extension can
                // add what Petdex has no word for and correct what it does.
                install(
                    extensionManifest.animations,
                    into: &tracks,
                    addressable: baseFrames + extensionColumns * extensionRows,
                    warnings: &warnings
                )
            } catch {
                warnings.append("Ignored roamling.json: \(error.localizedDescription)")
            }
        }

        return PetAsset(
            manifest: manifest,
            packageURL: package,
            atlas: atlas,
            frameWidth: layout.frameWidth,
            frameHeight: layout.frameHeight,
            columns: layout.columns,
            rows: layout.rows,
            tracks: tracks,
            behaviorMappings: behaviorMappings,
            extensionAtlas: extensionAtlas,
            extensionColumns: extensionColumns,
            extensionRows: extensionRows,
            warnings: warnings
        )
    }

    /// Turns declared frame lists into tracks, dropping the ones that would not
    /// render. A bad entry is reported and skipped rather than failing the load:
    /// one unusable name should not cost the user the whole pet.
    /// The extension sheet must share the package's cell size, so its pixel
    /// dimensions follow from the grid rather than being declared again.
    private func loadExtensionAtlas(
        at url: URL,
        grid: PetExtensionGrid,
        layout: Layout
    ) throws -> CGImage {
        guard FileManager.default.fileExists(atPath: url.path) else {
            throw PetLoadError.missingSpritesheet(url.path)
        }
        if let byteCount = try? url.resourceValues(forKeys: [.fileSizeKey]).fileSize,
           byteCount > Self.maximumEncodedBytes {
            throw PetLoadError.spritesheetTooLarge(byteCount)
        }
        guard let source = CGImageSourceCreateWithURL(url as CFURL, nil),
              let atlas = CGImageSourceCreateImageAtIndex(source, 0, nil) else {
            throw PetLoadError.unsupportedImage(url.path)
        }
        let expected = (grid.columns * layout.frameWidth, grid.rows * layout.frameHeight)
        guard atlas.width == expected.0, atlas.height == expected.1 else {
            throw PetLoadError.invalidFrameLayout(
                "extension sheet is \(atlas.width)x\(atlas.height), expected \(expected.0)x\(expected.1)"
            )
        }
        return atlas
    }

    private func requireExtensionGrid(_ grid: PetExtensionGrid?) throws -> PetExtensionGrid {
        guard let grid, grid.columns > 0, grid.rows > 0 else {
            throw PetLoadError.invalidFrameLayout("extension sheet needs a frame grid")
        }
        guard grid.columns * grid.rows <= Self.maximumFrames else {
            throw PetLoadError.invalidFrameLayout("extension grid is larger than \(Self.maximumFrames) cells")
        }
        return grid
    }

    private func install(
        _ animations: [String: PetAnimationManifest]?,
        into tracks: inout [String: PetAnimationTrack],
        addressable: Int,
        warnings: inout [String]
    ) {
        guard let animations else { return }
        for (name, definition) in animations {
            guard !name.isEmpty else {
                warnings.append("Ignored a custom animation with an empty name")
                continue
            }
            guard definition.frames.count <= Self.maximumFrames,
                  definition.frames.allSatisfy({ $0 >= 0 && $0 < addressable }) else {
                warnings.append("Ignored animation '\(name)' because a frame index is out of range")
                continue
            }
            let fps = definition.fps ?? 12
            guard fps > 0, fps <= 60 else {
                warnings.append("Ignored animation '\(name)' because fps must be in 0...60")
                continue
            }
            tracks[name] = PetAnimationTrack(
                name: name,
                frames: definition.frames.map { PetAnimationFrame(index: $0, duration: 1 / fps) },
                loops: definition.loop ?? true,
                fallback: definition.fallback
            )
        }
    }

    private func safeAssetURL(path: String, inside package: URL) throws -> URL {
        guard !path.isEmpty, !path.hasPrefix("/"), !path.hasPrefix("~") else {
            throw PetLoadError.unsafeSpritesheetPath(path)
        }
        let components = NSString(string: path).pathComponents
        guard !components.contains("..") else { throw PetLoadError.unsafeSpritesheetPath(path) }

        let resolvedPackage = package.resolvingSymlinksInPath().standardizedFileURL
        let candidate = package.appendingPathComponent(path).resolvingSymlinksInPath().standardizedFileURL
        let packagePrefix = resolvedPackage.path.hasSuffix("/")
            ? resolvedPackage.path
            : resolvedPackage.path + "/"
        guard candidate.path.hasPrefix(packagePrefix) else {
            throw PetLoadError.unsafeSpritesheetPath(path)
        }
        return candidate
    }

    private func resolveLayout(manifest: PetManifest, atlas: CGImage) throws -> Layout {
        let layout: Layout
        if let frame = manifest.frame {
            guard frame.width > 0, frame.height > 0, frame.columns > 0, frame.rows > 0 else {
                throw PetLoadError.invalidFrameLayout("custom frame values must be positive")
            }
            layout = Layout(
                frameWidth: frame.width,
                frameHeight: frame.height,
                columns: frame.columns,
                rows: frame.rows
            )
        } else {
            let version = manifest.spriteVersionNumber ?? 1
            switch version {
            case 1:
                layout = Layout(frameWidth: 192, frameHeight: 208, columns: 8, rows: 9)
            case 2:
                layout = Layout(frameWidth: 192, frameHeight: 208, columns: 8, rows: 11)
            default:
                throw PetLoadError.unsupportedSpriteVersion(version)
            }
        }

        guard layout.columns * layout.rows <= Self.maximumFrames else {
            throw PetLoadError.invalidFrameLayout("atlas exceeds \(Self.maximumFrames) frames")
        }
        let expectedWidth = layout.frameWidth * layout.columns
        let expectedHeight = layout.frameHeight * layout.rows
        guard atlas.width == expectedWidth, atlas.height == expectedHeight else {
            throw PetLoadError.invalidFrameLayout(
                "expected \(expectedWidth)x\(expectedHeight), got \(atlas.width)x\(atlas.height)"
            )
        }
        guard layout.rows >= 9, layout.columns >= 8 else {
            throw PetLoadError.invalidFrameLayout("standard animations require at least an 8x9 grid")
        }
        return layout
    }

    private struct Layout {
        let frameWidth: Int
        let frameHeight: Int
        let columns: Int
        let rows: Int
    }
}
