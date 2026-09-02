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
        .target(
            name: "RoamlingPet",
            dependencies: ["RoamlingCore"],
            resources: [
                .process("Resources")
            ],
            linkerSettings: [
                .linkedFramework("AppKit"),
                .linkedFramework("ImageIO")
            ]
        ),
        // The app's orchestration, with no window system under it. The only
        // framework it links is CoreGraphics, for the CGImage that RoamlingPet
        // still hands out; W2 takes that away and leaves this pure Swift.
        .target(
            name: "RoamlingEngine",
            dependencies: ["RoamlingCore", "RoamlingPet", "RoamlingSources"],
            linkerSettings: [
                .linkedFramework("CoreGraphics")
            ]
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
            dependencies: ["RoamlingCore", "RoamlingPet", "RoamlingSources"],
            path: "Tests/RoamlingLogicTests",
            linkerSettings: [
                .linkedFramework("ImageIO"),
                .linkedFramework("UniformTypeIdentifiers")
            ]
        )
    ]
)
