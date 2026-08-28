// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import AppKit
import ApplicationServices
import RoamlingCore

/// MVP 3 focus detail from the accessibility tree.
///
/// It reads geometry only. The focused element's value, the selected string,
/// the window title, and the document are never requested, so nothing the user
/// is writing can reach a `FocusSnapshot`. Without the Accessibility permission
/// every query returns nil and placement falls back to the MVP 1 coarse hint.
@MainActor
public final class MacFocusProvider: FocusProviding {
    /// A busy or unresponsive app must never stall the pet. Accessibility calls
    /// are synchronous, so the timeout is the only thing keeping a frame on
    /// time.
    private static let messagingTimeout: Float = 0.25

    private let coordinateSpace: () -> DesktopCoordinateSpace

    public init(coordinateSpace: @escaping () -> DesktopCoordinateSpace) {
        self.coordinateSpace = coordinateSpace
    }

    public var isAuthorized: Bool { AXIsProcessTrusted() }

    /// Shows the system prompt. Call only from an explicit menu action so the
    /// app never asks for Accessibility just because it launched.
    @discardableResult
    public func requestAuthorization() -> Bool {
        // `kAXTrustedCheckOptionPrompt` is an imported global var, which Swift 6
        // rejects as shared mutable state. The key's value is stable API.
        let promptKey = "AXTrustedCheckOptionPrompt"
        return AXIsProcessTrustedWithOptions([promptKey: true] as CFDictionary)
    }

    public func currentFocus() -> FocusSnapshot? {
        guard AXIsProcessTrusted(),
              let app = NSWorkspace.shared.frontmostApplication,
              app.processIdentifier != ProcessInfo.processInfo.processIdentifier else { return nil }

        let application = AXUIElementCreateApplication(app.processIdentifier)
        AXUIElementSetMessagingTimeout(application, Self.messagingTimeout)

        guard let element = Self.copyElement(application, kAXFocusedUIElementAttribute) else {
            return nil
        }

        let primaryTop = Double(NSScreen.screens.first?.frame.maxY ?? 0)
        let space = coordinateSpace()
        let convert = { (rect: CGRect) -> WorldRect? in
            guard rect.width > 0 || rect.height > 0 else { return nil }
            return space.rectFromCoreGraphics(
                WorldRect(
                    x: Double(rect.minX),
                    y: Double(rect.minY),
                    width: Double(rect.width),
                    height: Double(rect.height)
                ),
                primaryTop: primaryTop
            )
        }

        let elementFrame = Self.frame(of: element).flatMap(convert)
        let caretFrame = Self.caretRect(of: element).flatMap(convert)
        guard elementFrame != nil || caretFrame != nil else { return nil }

        // windowID stays nil until the focused window can be matched to a
        // CGWindowNumber without private API. The planner then keeps using the
        // coarse hint region, which is already the focused window's frame.
        return FocusSnapshot(
            focusedElementFrame: elementFrame,
            caretFrame: caretFrame,
            confidence: caretFrame == nil ? 0.7 : 0.9
        )
    }

    private static func copyValue(_ element: AXUIElement, _ attribute: String) -> CFTypeRef? {
        var value: CFTypeRef?
        guard AXUIElementCopyAttributeValue(element, attribute as CFString, &value) == .success else {
            return nil
        }
        return value
    }

    private static func copyElement(_ element: AXUIElement, _ attribute: String) -> AXUIElement? {
        guard let value = copyValue(element, attribute),
              CFGetTypeID(value) == AXUIElementGetTypeID() else { return nil }
        return (value as! AXUIElement)
    }

    private static func axValue(_ value: CFTypeRef?) -> AXValue? {
        guard let value, CFGetTypeID(value) == AXValueGetTypeID() else { return nil }
        return (value as! AXValue)
    }

    private static func frame(of element: AXUIElement) -> CGRect? {
        guard let positionValue = axValue(copyValue(element, kAXPositionAttribute)),
              let sizeValue = axValue(copyValue(element, kAXSizeAttribute)) else { return nil }
        var origin = CGPoint.zero
        var size = CGSize.zero
        guard AXValueGetValue(positionValue, .cgPoint, &origin),
              AXValueGetValue(sizeValue, .cgSize, &size) else { return nil }
        return CGRect(origin: origin, size: size)
    }

    private static func caretRect(of element: AXUIElement) -> CGRect? {
        guard let rangeValue = axValue(copyValue(element, kAXSelectedTextRangeAttribute)) else {
            return nil
        }
        var selected = CFRange()
        guard AXValueGetValue(rangeValue, .cfRange, &selected) else { return nil }

        // Collapse to the insertion point. A long selection would otherwise turn
        // the whole passage into one obstacle and push the pet off screen.
        var insertion = CFRange(location: selected.location, length: 0)
        guard let insertionValue = AXValueCreate(.cfRange, &insertion) else { return nil }

        var bounds: CFTypeRef?
        guard AXUIElementCopyParameterizedAttributeValue(
            element,
            kAXBoundsForRangeParameterizedAttribute as CFString,
            insertionValue,
            &bounds
        ) == .success, let boundsValue = axValue(bounds) else { return nil }

        var rect = CGRect.zero
        guard AXValueGetValue(boundsValue, .cgRect, &rect) else { return nil }
        return rect
    }
}
