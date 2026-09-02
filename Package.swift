// swift-tools-version: 6.2

// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import PackageDescription

let package = Package(
    name: "Roamling",
    defaultLocalization: "en",
    platforms: [
        .macOS(.v13)
    ],
    products: [
        .executable(name: "Roamling", targets: ["RoamlingApp"]),
        .executable(name: "RoamlingLogicTests", targets: ["RoamlingLogicTests"]),
        .library(name: "RoamlingCore", targets: ["RoamlingCore"]),
        .library(name: "RoamlingPet", targets: ["RoamlingPet"]),
        .library(name: "RoamlingSources", targets: ["RoamlingSources"]),
        .library(name: "RoamlingEngine", targets: ["RoamlingEngine"]),
        .library(name: "RoamlingShell", targets: ["RoamlingShell"])
    ],
    targets: [
        .target(name: "RoamlingCore"),
        // Foundation only since W2: decoding is the platform's job and every
        // other step is arithmetic on bytes.
        .target(
            name: "RoamlingPet",
            dependencies: ["RoamlingCore"],
            resources: [
                .process("Resources")
            ]
        ),
        // The app's orchestration. Foundation and the portable modules only --
        // W2 took the last CGImage out of the frame it hands the overlay.
        .target(
            name: "RoamlingEngine",
            dependencies: ["RoamlingCore", "RoamlingPet", "RoamlingSources"]
        ),
        // The user-facing surface with no widgets in it: the menu tree, the
        // words, and what the alerts say. Each platform renders this rather
        // than restating it.
        .target(
            name: "RoamlingShell",
            dependencies: ["RoamlingCore", "RoamlingPet", "RoamlingSources", "RoamlingEngine"],
            resources: [
                .process("Resources")
            ]
        ),
        .target(
            name: "RoamlingMac",
            dependencies: [
                "RoamlingCore", "RoamlingPet", "RoamlingSources", "RoamlingEngine", "RoamlingShell"
            ],
            linkerSettings: [
                .linkedFramework("AppKit"),
                .linkedFramework("CoreGraphics"),
                .linkedFramework("ScreenCaptureKit"),
                .linkedFramework("SwiftUI")
            ]
        ),
        // Foundation and BSD sockets since W3: the loopback receiver stopped
        // needing Apple's Network framework, which was one of the two lines
        // that would not compile on Windows.
        .target(
            name: "RoamlingSources",
            dependencies: ["RoamlingCore"]
        ),
        .executableTarget(
            name: "RoamlingApp",
            dependencies: ["RoamlingMac"]
        ),
        .executableTarget(
            name: "RoamlingLogicTests",
            dependencies: ["RoamlingCore", "RoamlingPet", "RoamlingSources", "RoamlingEngine", "RoamlingShell"],
            path: "Tests/RoamlingLogicTests",
            linkerSettings: [
                .linkedFramework("ImageIO"),
                .linkedFramework("UniformTypeIdentifiers")
            ]
        )
    ]
)
