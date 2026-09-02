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
        .library(name: "RoamlingEngine", targets: ["RoamlingEngine"])
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
        .target(
            name: "RoamlingMac",
            dependencies: ["RoamlingCore", "RoamlingPet", "RoamlingSources", "RoamlingEngine"],
            resources: [
                .process("Resources")
            ],
            linkerSettings: [
                .linkedFramework("AppKit"),
                .linkedFramework("CoreGraphics"),
                .linkedFramework("ScreenCaptureKit"),
                .linkedFramework("SwiftUI")
            ]
        ),
        .target(
            name: "RoamlingSources",
            dependencies: ["RoamlingCore"],
            linkerSettings: [
                .linkedFramework("Network")
            ]
        ),
        .executableTarget(
            name: "RoamlingApp",
            dependencies: ["RoamlingMac"]
        ),
        .executableTarget(
            name: "RoamlingLogicTests",
            dependencies: ["RoamlingCore", "RoamlingPet", "RoamlingSources", "RoamlingEngine"],
            path: "Tests/RoamlingLogicTests",
            linkerSettings: [
                .linkedFramework("ImageIO"),
                .linkedFramework("UniformTypeIdentifiers")
            ]
        )
    ]
)
