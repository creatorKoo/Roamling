// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import AppKit
import CoreGraphics
import RoamlingCore
import ScreenCaptureKit

/// MVP 4 visual placement input.
///
/// Takes one downsampled snapshot of a display and turns it into a
/// `LuminanceField`. The full-resolution image never leaves this method, is
/// never written to disk, and is never logged. ScreenCaptureKit does the
/// downsampling itself, so the process only ever holds the small grid.
@MainActor
public final class MacCaptureProvider: CaptureProviding {
    /// Wide enough to separate a paragraph from a margin, small enough that the
    /// capture stays cheap and cannot reconstruct anything readable.
    private static let sampleColumns = 64

    public init() {}

    public var isAuthorized: Bool { CGPreflightScreenCaptureAccess() }

    /// Shows the system prompt. Call only from an explicit menu action.
    @discardableResult
    public func requestAuthorization() -> Bool {
        CGRequestScreenCaptureAccess()
    }

    /// The stage the capture in flight is waiting on. Read by the runtime
    /// when it gives up on a request, so the log says *where* it stuck.
    ///
    /// Not cleared on return: a call the runtime already abandoned may return
    /// while a newer one is at its first stage, and clearing here would erase
    /// the newer one's answer. Every call sets it before its first await, so a
    /// stale value cannot outlive the start of the next call.
    public private(set) var inFlightStage: String?

    public func captureLuminanceField(for display: DisplaySnapshot) async -> CaptureOutcome {
        guard isAuthorized,
              !display.frame.isEmpty,
              let displayID = UInt32(display.id) else { return .failed(.unavailable) }
        // SCScreenshotManager is the only single-shot path; a stream would keep
        // capturing to answer one question.
        guard #available(macOS 14.0, *) else { return .failed(.unavailable) }

        inFlightStage = "content"
        guard let content = try? await SCShareableContent.excludingDesktopWindows(
            false,
            onScreenWindowsOnly: true
        ) else { return .failed(.noContent) }
        // A request the runtime has already given up on must not go on to
        // take a screenshot nobody will read. Cancellation is cooperative, so
        // this is the one place between the two system calls where it can be
        // honoured.
        if Task.isCancelled { return .failed(.cancelled) }
        // Named apart from `noContent`: the content list came back fine and the
        // display the runtime chose is simply not in it. With three displays
        // and a `nearestDisplay` fallback that is a real way to fail every
        // time, and it needs its own line in the log.
        guard let target = content.displays.first(where: { $0.displayID == displayID }) else {
            return .failed(.displayNotFound)
        }

        // The pet must not make its own seat look busy.
        let ownBundleID = Bundle.main.bundleIdentifier
        let ownApplications = content.applications.filter {
            $0.bundleIdentifier == ownBundleID
        }
        let filter = SCContentFilter(
            display: target,
            excludingApplications: ownApplications,
            exceptingWindows: []
        )

        let aspect = display.frame.size.height / display.frame.size.width
        let rows = max(2, Int((Double(Self.sampleColumns) * aspect).rounded()))
        let configuration = SCStreamConfiguration()
        configuration.width = Self.sampleColumns
        configuration.height = rows
        configuration.showsCursor = false

        inFlightStage = "screenshot"
        guard let image = try? await SCScreenshotManager.captureImage(
            contentFilter: filter,
            configuration: configuration
        ) else { return .failed(.noImage) }

        guard let field = Self.luminanceField(from: image, bounds: display.frame) else {
            return .failed(.unusableImage)
        }
        return .field(field)
    }

    /// Draws the snapshot into an 8-bit gray bitmap and keeps only the samples.
    /// Bitmap context memory is laid out top row first, which is the order
    /// `LuminanceField` expects.
    private static func luminanceField(from image: CGImage, bounds: WorldRect) -> LuminanceField? {
        let columns = image.width
        let rows = image.height
        guard columns > 1, rows > 1 else { return nil }

        var bytes = [UInt8](repeating: 0, count: columns * rows)
        let drawn = bytes.withUnsafeMutableBytes { buffer -> Bool in
            guard let context = CGContext(
                data: buffer.baseAddress,
                width: columns,
                height: rows,
                bitsPerComponent: 8,
                bytesPerRow: columns,
                space: CGColorSpaceCreateDeviceGray(),
                bitmapInfo: CGImageAlphaInfo.none.rawValue
            ) else { return false }
            context.draw(image, in: CGRect(x: 0, y: 0, width: columns, height: rows))
            return true
        }
        guard drawn else { return nil }

        return LuminanceField(
            bounds: bounds,
            columns: columns,
            rows: rows,
            samples: bytes.map { Double($0) / 255 }
        )
    }
}
