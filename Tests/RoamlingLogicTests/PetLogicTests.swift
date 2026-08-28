// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import CoreGraphics
import Foundation
import ImageIO
import RoamlingCore
import RoamlingPet
import UniformTypeIdentifiers

func petLogicTests() -> [LogicTest] {
    [
        LogicTest(name: "only idle behavior maps to the idle capability") {
            for state in BehaviorState.allCases {
                for velocityDX in [-12.0, 0.0, 12.0] {
                    let capability = PetCapabilityMapping.capability(
                        for: state,
                        velocityDX: velocityDX,
                        isCaughtTransitionActive: false
                    )
                    try expect(
                        capability != .idle || state == .idle,
                        "\(state) fell back to the idle capability"
                    )
                }
            }
        },
        LogicTest(name: "every walking behavior animates directionally") {
            for state in PetCapabilityMapping.movingStates {
                let left = PetCapabilityMapping.capability(
                    for: state,
                    velocityDX: -12,
                    isCaughtTransitionActive: false
                )
                let right = PetCapabilityMapping.capability(
                    for: state,
                    velocityDX: 12,
                    isCaughtTransitionActive: false
                )
                try expect(left == .moveLeft, "\(state) must walk left")
                try expect(right == .moveRight, "\(state) must walk right")
            }
            // Hook-driven travel used to slide across the desktop on idle frames.
            try expect(PetCapabilityMapping.movingStates.contains(.travelToInterest))
        },
        LogicTest(name: "drag keeps the caught animation during its transition") {
            try expect(
                PetCapabilityMapping.capability(
                    for: .dragged,
                    velocityDX: 0,
                    isCaughtTransitionActive: true
                ) == .caught
            )
            try expect(
                PetCapabilityMapping.capability(
                    for: .dragged,
                    velocityDX: 0,
                    isCaughtTransitionActive: false
                ) == .dragged
            )
        },
        LogicTest(name: "every behavior resolves to a built-in track") {
            for kind in [BuiltInPetKind.fatMochi, .mochi] {
                let resolver = MascotPetFactory.make(kind).resolver
                for state in BehaviorState.allCases {
                    let capability = PetCapabilityMapping.capability(
                        for: state,
                        velocityDX: -12,
                        isCaughtTransitionActive: false
                    )
                    let track = try require(
                        resolver.resolve(capability),
                        "\(kind) has no track for \(state)"
                    )
                    try expect(!track.frames.isEmpty, "\(kind) track for \(state) is empty")
                    if PetCapabilityMapping.movingStates.contains(state) {
                        try expect(
                            track.name != "idle",
                            "\(kind) resolved \(state) to the idle track"
                        )
                    }
                }
            }
        },
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
            try expect(pet.resolver.resolve(.sleep)?.name == "sleeping")
            try expect(pet.resolver.resolve(.sit)?.name == "idle")
        },
        LogicTest(name: "built-in mascots load with semantic tracks") {
            try expect(MascotPetFactory.make().manifest.displayName == "FatMochi")
            for kind in BuiltInPetKind.allCases {
                let pet = MascotPetFactory.make(kind)
                let expectedRows = 7
                try expect(pet.manifest.displayName == kind.displayName)
                try expect(pet.manifest.id == "roamling-\(kind.rawValue)")
                try expect(pet.columns == 8)
                try expect(pet.rows == expectedRows)
                try expect(pet.frameCount == expectedRows * 8)
                try expect(pet.frameImage(at: 0) != nil)
                try expect(pet.frameImage(at: expectedRows * 8 - 1) != nil)
                try expect(pet.resolver.resolve(.moveLeft)?.name == "running-left")
                try expect(pet.resolver.resolve(.moveRight)?.name == "running-right")
                try expect(pet.resolver.resolve(.sleep)?.name == "sleeping")
                try expect(pet.resolver.resolve(.caught)?.name == "caught")
                try expect(pet.resolver.resolve(.dragged)?.name == "dragged")
            }
        },
        LogicTest(name: "FatMochi uses authored limb animation cycles") {
            let pet = MascotPetFactory.make(.fatMochi)
            let idle = try require(pet.tracks["idle"])
            let right = try require(pet.tracks["running-right"])
            let left = try require(pet.tracks["running-left"])
            let sleep = try require(pet.tracks["sleeping"])
            let caught = try require(pet.tracks["caught"])
            let dragged = try require(pet.tracks["dragged"])
            let stretch = try require(pet.tracks["stretching"])
            let landing = try require(pet.tracks["landing"])

            try expect(idle.frames.map(\.index) == [0, 1, 2, 3, 4, 5])
            try expect(idle.frames.first?.duration == 1.25)
            try expect(right.frames.map(\.index) == Array(8...15))
            try expect(left.frames.map(\.index) == Array(16...23))
            try expect(sleep.frames.map(\.index) == [24, 25, 26, 27])
            try expect(caught.frames.map(\.index) == [32, 33, 34, 35])
            try expect(!caught.loops)
            try expect(dragged.frames.map(\.index) == [36, 37, 38, 39])
            try expect(stretch.frames.map(\.index) == [40, 41, 42, 43, 44, 45])
            try expect(!stretch.loops)
            try expect(landing.frames.map(\.index) == [48, 49, 50, 51, 52])
            try expect(!landing.loops)

            let idleSignatures = try [0, 1, 2, 3].map {
                try require(imageSignature(try require(pet.frameImage(at: $0))))
            }
            let rightSignatures = try Array(8...15).map {
                try require(imageSignature(try require(pet.frameImage(at: $0))))
            }
            let sleepSignatures = try Array(24...27).map {
                try require(imageSignature(try require(pet.frameImage(at: $0))))
            }
            let walkMetrics = try Array(8...15).map {
                try require(alphaMetrics(try require(pet.frameImage(at: $0))))
            }
            let caughtIntroMetrics = try Array(32...35).map {
                try require(alphaMetrics(try require(pet.frameImage(at: $0))))
            }
            let draggedMetrics = try Array(36...39).map {
                try require(alphaMetrics(try require(pet.frameImage(at: $0))))
            }
            let draggedSignatures = try Array(36...39).map {
                try require(imageSignature(try require(pet.frameImage(at: $0))))
            }
            let idleFrame = try require(pet.frameImage(at: 0))
            let idleMetric = try require(alphaMetrics(idleFrame))
            let idleSignature = try require(imageSignature(idleFrame))
            try expect(Set(idleSignatures).count == 4)
            try expect(Set(rightSignatures).count == 8)
            try expect(Set(sleepSignatures).count == 4)
            let walkBoundsCenters = walkMetrics.map { $0.boundsCenterX }
            let minimumWalkBoundsCenter = try require(walkBoundsCenters.min())
            let maximumWalkBoundsCenter = try require(walkBoundsCenters.max())
            try expect(maximumWalkBoundsCenter - minimumWalkBoundsCenter < 2)
            try expect(walkMetrics.allSatisfy {
                abs($0.width - idleMetric.width) <= 2 && abs($0.height - idleMetric.height) <= 2
            })

            let caughtIntroCenters = caughtIntroMetrics.map { $0.boundsCenterX }
            let minimumCaughtIntroCenter = try require(caughtIntroCenters.min())
            let maximumCaughtIntroCenter = try require(caughtIntroCenters.max())
            try expect(maximumCaughtIntroCenter - minimumCaughtIntroCenter < 2)
            try expect(caughtIntroMetrics.allSatisfy {
                abs($0.width - idleMetric.width) <= 1 && abs($0.height - idleMetric.height) <= 1
            })
            let draggedBoundsCenters = draggedMetrics.map { $0.boundsCenterX }
            let minimumDraggedBoundsCenter = try require(draggedBoundsCenters.min())
            let maximumDraggedBoundsCenter = try require(draggedBoundsCenters.max())
            try expect(maximumDraggedBoundsCenter - minimumDraggedBoundsCenter < 2)
            try expect(draggedMetrics.allSatisfy {
                $0.width == 180 && $0.height == 183
            })
            try expect(draggedMetrics.allSatisfy { $0.height >= idleMetric.height + 10 })
            try expect(Set(draggedSignatures).count == 4)
            let caughtStart = try require(pet.frameImage(at: 32))
            let caughtStartSignature = try require(imageSignature(caughtStart))
            try expect(caughtStartSignature == idleSignature)

            let walkStart = try require(pet.frameImage(at: 16))
            let walkStartSignature = try require(imageSignature(walkStart))
            try expect(walkStartSignature == idleSignature)

            let idleFace = try require(idleFrame.cropping(to: CGRect(x: 24, y: 100, width: 104, height: 30)))
            let idleFaceSignature = try require(imageSignature(idleFace))
            for index in 16...23 {
                let walkFrame = try require(pet.frameImage(at: index))
                let walkFace = try require(walkFrame.cropping(to: CGRect(x: 24, y: 100, width: 104, height: 30)))
                let walkFaceSignature = try require(imageSignature(walkFace))
                try expect(walkFaceSignature == idleFaceSignature)
                let neckWidth = try require(centeredOpaqueRunWidth(walkFrame, y: 90))
                let cheekWidth = try require(centeredOpaqueRunWidth(walkFrame, y: 110))
                try expect(cheekWidth - neckWidth >= 3)
            }

            for index in 0..<pet.frameCount {
                let frame = try require(pet.frameImage(at: index))
                try expect(
                    opaqueComponentCount(frame) == 1,
                    "FatMochi frame \(index) contains detached opaque artwork"
                )
            }
        },
        LogicTest(name: "FatMochi caught intro hands off to a looping drag") {
            var player = PetAnimationPlayer(asset: MascotPetFactory.make(.fatMochi))
            player.setCapability(.caught)
            try expect(player.currentFrameIndex == 32)
            player.update(deltaTime: 0.05)
            try expect(player.currentFrameIndex == 33)
            player.update(deltaTime: 0.30)
            try expect(player.currentFrameIndex == 35)

            player.setCapability(.dragged)
            try expect(player.currentFrameIndex == 36)
            player.update(deltaTime: 0.10)
            try expect(player.currentFrameIndex == 37)
            player.update(deltaTime: 0.27)
            try expect(player.currentFrameIndex == 36)
        },
        LogicTest(name: "Mochi uses authored creature animation cycles") {
            let pet = MascotPetFactory.make(.mochi)
            let idle = try require(pet.tracks["idle"])
            let right = try require(pet.tracks["running-right"])
            let left = try require(pet.tracks["running-left"])
            let sleep = try require(pet.tracks["sleeping"])
            let caught = try require(pet.tracks["caught"])
            let dragged = try require(pet.tracks["dragged"])
            let stretch = try require(pet.tracks["stretching"])
            let landing = try require(pet.tracks["landing"])

            try expect(pet.rows == 7)
            try expect(idle.frames.map(\.index) == Array(0...7))
            try expect(right.frames.map(\.index) == Array(8...15))
            try expect(left.frames.map(\.index) == Array(16...23))
            try expect(sleep.frames.map(\.index) == [24, 25, 26, 27])
            try expect(caught.frames.map(\.index) == [32, 33, 34, 35])
            try expect(!caught.loops)
            try expect(dragged.frames.map(\.index) == [36, 37, 38, 39])
            try expect(stretch.frames.map(\.index) == Array(40...45))
            try expect(!stretch.loops)
            try expect(landing.frames.map(\.index) == Array(48...53))
            try expect(!landing.loops)

            let idleSignatures = try Array(0...7).map {
                try require(imageSignature(try require(pet.frameImage(at: $0))))
            }
            let walkSignatures = try Array(8...15).map {
                try require(imageSignature(try require(pet.frameImage(at: $0))))
            }
            let sleepSignatures = try Array(24...27).map {
                try require(imageSignature(try require(pet.frameImage(at: $0))))
            }
            let draggedSignatures = try Array(36...39).map {
                try require(imageSignature(try require(pet.frameImage(at: $0))))
            }
            let walkMetrics = try Array(8...15).map {
                try require(alphaMetrics(try require(pet.frameImage(at: $0))))
            }
            try expect(Set(idleSignatures).count >= 4)
            try expect(Set(walkSignatures).count == 8)
            try expect(Set(sleepSignatures).count >= 3)
            try expect(Set(draggedSignatures).count == 4)
            let walkCenters = walkMetrics.map(\.boundsCenterX)
            let minimumWalkCenter = try require(walkCenters.min())
            let maximumWalkCenter = try require(walkCenters.max())
            try expect(
                maximumWalkCenter - minimumWalkCenter <= 1,
                "Mochi walk frames must remain centered while the runtime moves the sprite"
            )

            let idleSignature = try require(imageSignature(try require(pet.frameImage(at: 0))))
            for index in [32, 40, 45, 48, 53] {
                let signature = try require(imageSignature(try require(pet.frameImage(at: index))))
                try expect(signature == idleSignature, "Mochi transition frame \(index) must match idle")
            }

            for index in 0..<pet.frameCount {
                let metrics = alphaMetrics(try require(pet.frameImage(at: index)))
                try expect(metrics != nil)
            }
        },
        LogicTest(name: "built-in completion tracks animate through the full reaction") {
            for kind in [BuiltInPetKind.fatMochi, .mochi] {
                let pet = MascotPetFactory.make(kind)
                let celebration = try require(pet.tracks["jumping"])
                let duration = celebration.frames.reduce(0) { $0 + $1.duration }

                try expect(!celebration.loops)
                try expect(duration >= 2.19)
                try expect(duration <= 2.21)
                try expect(Set(celebration.frames.map(\.index)).count >= 4)
                try expect(celebration.frames.last?.index == (kind == .fatMochi ? 48 : 53))
            }
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

private func imageSignature(_ image: CGImage) -> UInt64? {
    let bytesPerRow = image.width * 4
    var bytes = [UInt8](repeating: 0, count: bytesPerRow * image.height)
    guard let context = CGContext(
        data: &bytes,
        width: image.width,
        height: image.height,
        bitsPerComponent: 8,
        bytesPerRow: bytesPerRow,
        space: CGColorSpaceCreateDeviceRGB(),
        bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
    ) else { return nil }
    context.interpolationQuality = .none
    context.setShouldAntialias(false)
    context.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))

    return bytes.reduce(UInt64(1_469_598_103_934_665_603)) { partial, byte in
        (partial ^ UInt64(byte)) &* 1_099_511_628_211
    }
}

private func alphaMetrics(
    _ image: CGImage
) -> (width: Int, height: Int, boundsCenterX: Double)? {
    let bytesPerRow = image.width * 4
    var bytes = [UInt8](repeating: 0, count: bytesPerRow * image.height)
    guard let context = CGContext(
        data: &bytes,
        width: image.width,
        height: image.height,
        bitsPerComponent: 8,
        bytesPerRow: bytesPerRow,
        space: CGColorSpaceCreateDeviceRGB(),
        bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
    ) else { return nil }
    context.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))

    var minimumX = image.width
    var maximumX = -1
    var minimumY = image.height
    var maximumY = -1
    var pixelCount = 0
    for y in 0..<image.height {
        for x in 0..<image.width where bytes[y * bytesPerRow + x * 4 + 3] > 8 {
            minimumX = min(minimumX, x)
            maximumX = max(maximumX, x)
            minimumY = min(minimumY, y)
            maximumY = max(maximumY, y)
            pixelCount += 1
        }
    }
    guard pixelCount > 0 else { return nil }
    return (
        width: maximumX - minimumX + 1,
        height: maximumY - minimumY + 1,
        boundsCenterX: Double(minimumX + maximumX) / 2
    )
}

private func centeredOpaqueRunWidth(_ image: CGImage, y: Int) -> Int? {
    let bytesPerRow = image.width * 4
    var bytes = [UInt8](repeating: 0, count: bytesPerRow * image.height)
    guard y >= 0, y < image.height,
          let context = CGContext(
              data: &bytes,
              width: image.width,
              height: image.height,
              bitsPerComponent: 8,
              bytesPerRow: bytesPerRow,
              space: CGColorSpaceCreateDeviceRGB(),
              bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
          ) else { return nil }
    context.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))

    let centerX = image.width / 2
    var runs: [ClosedRange<Int>] = []
    var runStart: Int?
    for x in 0..<image.width {
        let isOpaque = bytes[y * bytesPerRow + x * 4 + 3] > 8
        if isOpaque, runStart == nil { runStart = x }
        if !isOpaque, let start = runStart {
            runs.append(start...(x - 1))
            runStart = nil
        }
    }
    if let start = runStart { runs.append(start...(image.width - 1)) }
    guard let run = runs.min(by: {
        distance(from: centerX, to: $0) < distance(from: centerX, to: $1)
    }) else { return nil }
    return run.upperBound - run.lowerBound + 1
}

private func opaqueComponentCount(_ image: CGImage) -> Int? {
    let bytesPerRow = image.width * 4
    var bytes = [UInt8](repeating: 0, count: bytesPerRow * image.height)
    guard let context = CGContext(
        data: &bytes,
        width: image.width,
        height: image.height,
        bitsPerComponent: 8,
        bytesPerRow: bytesPerRow,
        space: CGColorSpaceCreateDeviceRGB(),
        bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
    ) else { return nil }
    context.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))

    let width = image.width
    let height = image.height
    var visited = [Bool](repeating: false, count: width * height)
    var componentCount = 0
    let neighbors = [
        (-1, -1), (0, -1), (1, -1),
        (-1, 0),            (1, 0),
        (-1, 1),  (0, 1),   (1, 1)
    ]

    for y in 0..<height {
        for x in 0..<width {
            let start = y * width + x
            guard !visited[start], bytes[y * bytesPerRow + x * 4 + 3] > 0 else { continue }
            componentCount += 1
            visited[start] = true
            var queue = [start]
            var cursor = 0

            while cursor < queue.count {
                let point = queue[cursor]
                cursor += 1
                let pointX = point % width
                let pointY = point / width
                for (offsetX, offsetY) in neighbors {
                    let nextX = pointX + offsetX
                    let nextY = pointY + offsetY
                    guard nextX >= 0, nextX < width, nextY >= 0, nextY < height else {
                        continue
                    }
                    let next = nextY * width + nextX
                    guard !visited[next], bytes[nextY * bytesPerRow + nextX * 4 + 3] > 0 else {
                        continue
                    }
                    visited[next] = true
                    queue.append(next)
                }
            }
        }
    }
    return componentCount
}

private func distance(from x: Int, to range: ClosedRange<Int>) -> Int {
    if range.contains(x) { return 0 }
    return min(abs(x - range.lowerBound), abs(x - range.upperBound))
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
