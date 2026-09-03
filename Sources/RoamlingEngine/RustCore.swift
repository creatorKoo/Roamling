// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation
import RoamlingCore
import RoamlingCoreRs

/// The seam the port crosses on macOS.
///
/// Swift's own types stay on this side while the units come over one at a time,
/// so each function converts, calls, and converts back. That conversion is the
/// cost -- `docs/windows.md` section 12 measured the crossing itself at 27 ns
/// and the payload at everything else -- which is why these take a whole world
/// rather than being called per rectangle.
///
/// Windows will not have this file. Its shell links the crate and calls the
/// same functions with no boundary at all.
enum RustCore {
    private static func displays(_ world: DesktopWorldSnapshot) -> [FfiDisplay] {
        world.displays.map { display in
            FfiDisplay(
                id: display.id,
                frame: rect(display.frame),
                visibleFrame: rect(display.visibleFrame)
            )
        }
    }

    private static func rect(_ value: WorldRect) -> FfiRect {
        FfiRect(x: value.minX, y: value.minY, width: value.size.width, height: value.size.height)
    }

    private static func zone(_ value: FfiSafeZone) -> SafeZone {
        SafeZone(
            frame: WorldRect(
                x: value.frame.x,
                y: value.frame.y,
                width: value.frame.width,
                height: value.frame.height
            ),
            score: value.score,
            confidence: value.confidence,
            reason: value.reason
        )
    }

    static func safeZones(in world: DesktopWorldSnapshot) -> [SafeZone] {
        RoamlingCoreRs.safeZones(displays: displays(world)).map(zone)
    }

    static func napsInPlace(
        at position: WorldPoint,
        objectSize: WorldSize,
        in field: LuminanceField?,
        atLeast threshold: Double
    ) -> Bool {
        RoamlingCoreRs.napsInPlace(
            x: position.x,
            y: position.y,
            objectWidth: objectSize.width,
            objectHeight: objectSize.height,
            field: field.map {
                FfiLuminanceField(
                    bounds: rect($0.bounds),
                    columns: UInt32($0.columns),
                    rows: UInt32($0.rows),
                    samples: $0.samples
                )
            },
            threshold: threshold
        )
    }

    static func restDestination(
        in world: DesktopWorldSnapshot,
        currentPosition: WorldPoint,
        pointerPosition: WorldPoint?,
        objectSize: WorldSize
    ) -> RestDestination? {
        let answer = RoamlingCoreRs.restDestination(
            displays: displays(world),
            zones: world.safeZones.map {
                FfiSafeZone(
                    frame: rect($0.frame),
                    score: $0.score,
                    confidence: $0.confidence,
                    reason: $0.reason
                )
            },
            currentX: currentPosition.x,
            currentY: currentPosition.y,
            pointer: pointerPosition.map { [$0.x, $0.y] },
            objectWidth: objectSize.width,
            objectHeight: objectSize.height
        )
        return answer.map {
            RestDestination(
                point: WorldPoint(x: $0.x, y: $0.y),
                displayID: $0.displayId,
                reason: $0.reason,
                score: $0.score
            )
        }
    }
}

/// The switch-over test compares the Rust answer against the Swift one it
/// replaced. Public only for that, and it goes when the Swift planner does.
public enum RustCoreTestBridge {
    public static func safeZones(in world: DesktopWorldSnapshot) -> [SafeZone] {
        RustCore.safeZones(in: world)
    }

    public static func napsInPlace(
        at position: WorldPoint,
        objectSize: WorldSize,
        in field: LuminanceField?,
        atLeast threshold: Double
    ) -> Bool {
        RustCore.napsInPlace(
            at: position, objectSize: objectSize, in: field, atLeast: threshold
        )
    }

    public static func restDestination(
        in world: DesktopWorldSnapshot,
        currentPosition: WorldPoint,
        pointerPosition: WorldPoint?,
        objectSize: WorldSize
    ) -> RestDestination? {
        RustCore.restDestination(
            in: world,
            currentPosition: currentPosition,
            pointerPosition: pointerPosition,
            objectSize: objectSize
        )
    }
}
