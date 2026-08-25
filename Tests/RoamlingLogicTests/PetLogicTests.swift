// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import CoreGraphics
import Foundation
import ImageIO
import RoamlingPet
import UniformTypeIdentifiers

func petLogicTests() -> [LogicTest] {
    [
        LogicTest(name: "official minimal v2 manifest decodes") {
            let json = """
            {
              "id":"calico",
              "displayName":"Calico",
              "description":"A tiny cat.",
              "spriteVersionNumber":2,
              "spritesheetPath":"spritesheet.webp"
            }
            """
            let manifest = try JSONDecoder().decode(PetManifest.self, from: Data(json.utf8))
            try expect(manifest.id == "calico")
            try expect(manifest.spriteVersionNumber == 2)
            try expect(manifest.frame == nil)
            try expect(manifest.animations == nil)
        },
        LogicTest(name: "capability resolver falls back gracefully") {
            let idle = PetAnimationTrack(name: "idle", frames: [PetAnimationFrame(index: 0, duration: 1)])
            let running = PetAnimationTrack(name: "running", frames: [PetAnimationFrame(index: 1, duration: 1)])
            let resolver = AnimationResolver(tracks: ["idle": idle, "running": running])
            try expect(resolver.resolve(.work)?.name == "running")
            try expect(resolver.resolve(.sleep)?.name == "idle")
            try expect(resolver.resolve(.celebrate)?.name == "idle")
        },
        LogicTest(name: "roamling explicit behavior wins with safe fallback") {
            let idle = PetAnimationTrack(name: "idle", frames: [PetAnimationFrame(index: 0, duration: 1)])
            let nap = PetAnimationTrack(name: "deep-nap", frames: [PetAnimationFrame(index: 2, duration: 1)])
            let resolver = AnimationResolver(
                tracks: ["idle": idle, "deep-nap": nap],
                explicitBehaviors: ["sleep": "deep-nap", "caught": "missing"]
            )
            try expect(resolver.resolve(.sleep)?.name == "deep-nap")
            try expect(resolver.resolve(.caught)?.name == "idle")
        },
        LogicTest(name: "placeholder implements v2 look directions") {
            let pet = PlaceholderPetFactory.make()
            try expect(pet.columns == 8)
            try expect(pet.rows == 11)
            try expect(pet.lookFrameIndex(degrees: 0) == 72)
            try expect(pet.lookFrameIndex(degrees: 90) == 76)
            try expect(pet.lookFrameIndex(degrees: 180) == 80)
            try expect(pet.lookFrameIndex(degrees: 337.5) == 87)
            try expect(pet.frameImage(at: 0) != nil)
            try expect(pet.frameImage(at: 87) != nil)
        },
        LogicTest(name: "loader keeps valid custom animation and isolates invalid track") {
            let fixture = try FixturePackage(frameWidth: 1, frameHeight: 1, rows: 9)
            defer { fixture.remove() }
            let manifest = PetManifest(
                id: "fixture",
                displayName: "Fixture",
                description: "Test",
                spriteVersionNumber: 1,
                spritesheetPath: "spritesheet.png",
                frame: PetFrameManifest(width: 1, height: 1, columns: 8, rows: 9),
                animations: [
                    "typing": PetAnimationManifest(frames: [0, 1, 2], fps: 20, loop: true, fallback: "running"),
                    "broken": PetAnimationManifest(frames: [999], fps: 12)
                ]
            )
            try fixture.write(manifest: manifest)
            let pet = try PetLoader().load(packageAt: fixture.url)
            try expect(pet.tracks["typing"]?.frames.count == 3)
            try expect(pet.tracks["broken"] == nil)
            try expect(pet.warnings.contains(where: { $0.contains("broken") }))
        },
        LogicTest(name: "loader rejects spritesheet path traversal") {
            let fixture = try FixturePackage(frameWidth: 1, frameHeight: 1, rows: 9)
            defer { fixture.remove() }
            let manifest = PetManifest(
                id: "unsafe",
                displayName: "Unsafe",
                description: "Test",
                spritesheetPath: "../outside.png"
            )
            try fixture.write(manifest: manifest)
            do {
                _ = try PetLoader().load(packageAt: fixture.url)
                throw LogicTestFailure(message: "Expected unsafe path error", file: #filePath, line: #line)
            } catch let error as PetLoadError {
                try expect(error == .unsafeSpritesheetPath("../outside.png"))
            }
        },
        LogicTest(name: "v2 atlas without version is rejected as v1 mismatch") {
            let fixture = try FixturePackage(frameWidth: 192, frameHeight: 208, rows: 11)
            defer { fixture.remove() }
            let manifest = PetManifest(
                id: "missing-version",
                displayName: "Missing Version",
                description: "Test",
                spritesheetPath: "spritesheet.png"
            )
            try fixture.write(manifest: manifest)
            do {
                _ = try PetLoader().load(packageAt: fixture.url)
                throw LogicTestFailure(message: "Expected layout error", file: #filePath, line: #line)
            } catch let error as PetLoadError {
                guard case let .invalidFrameLayout(message) = error else {
                    throw LogicTestFailure(message: "Unexpected error: \(error)", file: #filePath, line: #line)
                }
                try expect(message.contains("1536x1872"))
            }

            let validV2 = PetManifest(
                id: "valid-v2",
                displayName: "Valid V2",
                description: "Test",
                spriteVersionNumber: 2,
                spritesheetPath: "spritesheet.png"
            )
            try JSONEncoder().encode(validV2).write(to: fixture.url.appendingPathComponent("pet.json"))
            let loaded = try PetLoader().load(packageAt: fixture.url)
            try expect(loaded.rows == 11)
            try expect(loaded.supportsDirectionalLook)
        },
        LogicTest(name: "standard v1 atlas loads without explicit version") {
            let fixture = try FixturePackage(frameWidth: 192, frameHeight: 208, rows: 9)
            defer { fixture.remove() }
            let manifest = PetManifest(
                id: "valid-v1",
                displayName: "Valid V1",
                description: "Test",
                spritesheetPath: "spritesheet.png"
            )
            try fixture.write(manifest: manifest)
            let loaded = try PetLoader().load(packageAt: fixture.url)
            try expect(loaded.rows == 9)
            try expect(!loaded.supportsDirectionalLook)
            try expect(loaded.resolver.resolve(.moveLeft)?.name == "running-left")
        },
        LogicTest(name: "frame zero slices visual top row") {
            let fixture = try FixturePackage(frameWidth: 1, frameHeight: 1, rows: 9, striped: true)
            defer { fixture.remove() }
            let manifest = PetManifest(
                id: "rows",
                displayName: "Rows",
                description: "Test",
                spritesheetPath: "spritesheet.png",
                frame: PetFrameManifest(width: 1, height: 1, columns: 8, rows: 9)
            )
            try fixture.write(manifest: manifest)
            let pet = try PetLoader().load(packageAt: fixture.url)
            let image = try require(pet.frameImage(at: 0))
            let rgba = try require(sampleRGBA(image))
            try expect(rgba.0 > 200, "Expected red top row, got \(rgba)")
            try expect(rgba.2 < 50, "Expected little blue in top row, got \(rgba)")
        }
    ]
}

private func sampleRGBA(_ image: CGImage) -> (UInt8, UInt8, UInt8, UInt8)? {
    var bytes = [UInt8](repeating: 0, count: 4)
    guard let context = CGContext(
        data: &bytes,
        width: 1,
        height: 1,
        bitsPerComponent: 8,
        bytesPerRow: 4,
        space: CGColorSpaceCreateDeviceRGB(),
        bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
    ) else { return nil }
    context.draw(image, in: CGRect(x: 0, y: 0, width: 1, height: 1))
    return (bytes[0], bytes[1], bytes[2], bytes[3])
}

private final class FixturePackage {
    let url: URL
    private let image: CGImage

    init(frameWidth: Int, frameHeight: Int, rows: Int, striped: Bool = false) throws {
        url = FileManager.default.temporaryDirectory
            .appendingPathComponent("roamling-pet-test-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        let width = frameWidth * 8
        let height = frameHeight * rows
        guard let context = CGContext(
            data: nil,
            width: width,
            height: height,
            bitsPerComponent: 8,
            bytesPerRow: 0,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        ) else { throw FixtureError.context }
        context.clear(CGRect(x: 0, y: 0, width: width, height: height))
        if striped {
            context.setFillColor(CGColor(red: 0, green: 0, blue: 1, alpha: 1))
            context.fill(CGRect(x: 0, y: 0, width: width, height: frameHeight))
            context.setFillColor(CGColor(red: 1, green: 0, blue: 0, alpha: 1))
            context.fill(CGRect(x: 0, y: height - frameHeight, width: width, height: frameHeight))
        }
        guard let image = context.makeImage() else { throw FixtureError.context }
        self.image = image
    }

    func write(manifest: PetManifest) throws {
        try JSONEncoder().encode(manifest).write(to: url.appendingPathComponent("pet.json"))
        let imageURL = url.appendingPathComponent("spritesheet.png")
        guard let destination = CGImageDestinationCreateWithURL(
            imageURL as CFURL,
            UTType.png.identifier as CFString,
            1,
            nil
        ) else { throw FixtureError.destination }
        CGImageDestinationAddImage(destination, image, nil)
        guard CGImageDestinationFinalize(destination) else { throw FixtureError.destination }
    }

    func remove() {
        try? FileManager.default.removeItem(at: url)
    }

    enum FixtureError: Error {
        case context
        case destination
    }
}
