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

        if let customAnimations = manifest.animations {
            for (name, definition) in customAnimations {
                guard !name.isEmpty else {
                    warnings.append("Ignored a custom animation with an empty name")
                    continue
                }
                guard definition.frames.count <= Self.maximumFrames,
                      definition.frames.allSatisfy({ $0 >= 0 && $0 < layout.columns * layout.rows }) else {
                    warnings.append("Ignored animation '\(name)' because a frame index is out of range")
                    continue
                }
                let fps = definition.fps ?? 12
                guard fps > 0, fps <= 60 else {
                    warnings.append("Ignored animation '\(name)' because fps must be in 0...60")
                    continue
                }
                let frames = definition.frames.map {
                    PetAnimationFrame(index: $0, duration: 1 / fps)
                }
                tracks[name] = PetAnimationTrack(
                    name: name,
                    frames: frames,
                    loops: definition.loop ?? true,
                    fallback: definition.fallback
                )
            }
        }

        var behaviorMappings: [String: String] = [:]
        let extensionURL = package.appendingPathComponent("roamling.json")
        if FileManager.default.fileExists(atPath: extensionURL.path) {
            do {
                let extensionManifest = try JSONDecoder().decode(
                    RoamlingManifest.self,
                    from: Data(contentsOf: extensionURL)
                )
                if extensionManifest.schemaVersion == 1 {
                    behaviorMappings = extensionManifest.behaviors
                } else {
                    warnings.append("Ignored roamling.json schemaVersion \(extensionManifest.schemaVersion)")
                }
            } catch {
                warnings.append("Ignored malformed roamling.json: \(error.localizedDescription)")
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
            warnings: warnings
        )
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
