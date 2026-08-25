// swift-tools-version: 6.2

// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import PackageDescription

let package = Package(
    name: "Roamling",
    platforms: [
        .macOS(.v13)
    ],
    products: [
        .executable(name: "Roamling", targets: ["RoamlingApp"]),
        .executable(name: "RoamlingLogicTests", targets: ["RoamlingLogicTests"]),
        .library(name: "RoamlingCore", targets: ["RoamlingCore"]),
        .library(name: "RoamlingPet", targets: ["RoamlingPet"])
    ],
    targets: [
        .target(name: "RoamlingCore"),
        .target(
            name: "RoamlingPet",
            dependencies: ["RoamlingCore"],
            linkerSettings: [
                .linkedFramework("AppKit"),
                .linkedFramework("ImageIO")
            ]
        ),
        .target(
            name: "RoamlingMac",
            dependencies: ["RoamlingCore", "RoamlingPet"],
            linkerSettings: [
                .linkedFramework("AppKit"),
                .linkedFramework("CoreGraphics"),
                .linkedFramework("SwiftUI")
            ]
        ),
        .executableTarget(
            name: "RoamlingApp",
            dependencies: ["RoamlingMac"]
        ),
        .executableTarget(
            name: "RoamlingLogicTests",
            dependencies: ["RoamlingCore", "RoamlingPet"],
            path: "Tests/RoamlingLogicTests",
            linkerSettings: [
                .linkedFramework("ImageIO"),
                .linkedFramework("UniformTypeIdentifiers")
            ]
        )
    ]
)
