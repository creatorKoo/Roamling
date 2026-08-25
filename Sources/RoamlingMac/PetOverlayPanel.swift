// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import AppKit
import RoamlingCore

@MainActor
public protocol PetOverlayViewDelegate: AnyObject {
    func petOverlayMouseDown(screenPoint: NSPoint)
    func petOverlayDragged(screenPoint: NSPoint, distance: CGFloat)
    func petOverlayMouseUp(screenPoint: NSPoint, wasDragged: Bool)
}

@MainActor
public final class PetOverlayView: NSView {
    public weak var delegate: PetOverlayViewDelegate?

    private var image: CGImage?
    private var dragOrigin: NSPoint?
    private var draggedDistance: CGFloat = 0

    public override var isFlipped: Bool { true }

    public func setImage(_ image: CGImage?) {
        guard self.image !== image else { return }
        self.image = image
        needsDisplay = true
    }

    public override func draw(_ dirtyRect: NSRect) {
        super.draw(dirtyRect)
        guard let image else { return }
        NSGraphicsContext.current?.imageInterpolation = .none
        let appKitImage = NSImage(cgImage: image, size: bounds.size)
        appKitImage.draw(
            in: bounds,
            from: .zero,
            operation: .sourceOver,
            fraction: 1,
            respectFlipped: true,
            hints: [.interpolation: NSImageInterpolation.none]
        )
    }

    /// Excludes most transparent atlas padding without claiming pixel-perfect
    /// knowledge of arbitrary artwork. The window-level gate uses this too.
    public func containsPet(at point: NSPoint) -> Bool {
        guard bounds.contains(point) else { return false }
        let hitRect = bounds.insetBy(dx: bounds.width * 0.13, dy: bounds.height * 0.08)
            .offsetBy(dx: 0, dy: bounds.height * 0.03)
        return NSBezierPath(ovalIn: hitRect).contains(point)
    }

    public override func hitTest(_ point: NSPoint) -> NSView? {
        containsPet(at: point) ? self : nil
    }

    public override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    public override func mouseDown(with event: NSEvent) {
        let point = NSEvent.mouseLocation
        dragOrigin = point
        draggedDistance = 0
        delegate?.petOverlayMouseDown(screenPoint: point)
    }

    public override func mouseDragged(with event: NSEvent) {
        let point = NSEvent.mouseLocation
        if let dragOrigin {
            draggedDistance = max(draggedDistance, hypot(point.x - dragOrigin.x, point.y - dragOrigin.y))
        }
        delegate?.petOverlayDragged(screenPoint: point, distance: draggedDistance)
    }

    public override func mouseUp(with event: NSEvent) {
        let wasDragged = draggedDistance > 4
        delegate?.petOverlayMouseUp(screenPoint: NSEvent.mouseLocation, wasDragged: wasDragged)
        dragOrigin = nil
        draggedDistance = 0
    }
}

@MainActor
public final class PetOverlayPanel: NSPanel {
    public init(contentView: NSView, size: NSSize) {
        super.init(
            contentRect: NSRect(origin: .zero, size: size),
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        level = .floating
        isFloatingPanel = true
        isOpaque = false
        backgroundColor = .clear
        hasShadow = false
        hidesOnDeactivate = false
        isMovable = false
        isMovableByWindowBackground = false
        becomesKeyOnlyIfNeeded = true
        acceptsMouseMovedEvents = true
        animationBehavior = .none
        collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary, .ignoresCycle]
        sharingType = .none
        isReleasedWhenClosed = false
        ignoresMouseEvents = true
        self.contentView = contentView
    }

    public override var canBecomeKey: Bool { false }
    public override var canBecomeMain: Bool { false }
}

@MainActor
public final class MacOverlayProvider: OverlayProviding {
    public static let baseSize = WorldSize(width: 96, height: 104)

    public let view: PetOverlayView
    public let panel: PetOverlayPanel

    public private(set) var scale: Double
    public var coordinateSpace: DesktopCoordinateSpace
    private var worldPosition: WorldPoint = .zero
    private var interactionEnabled = false

    public init(coordinateSpace: DesktopCoordinateSpace, scale: Double = 1) {
        self.coordinateSpace = coordinateSpace
        self.scale = scale.clamped(to: 0.6...1.8)
        let size = NSSize(
            width: Self.baseSize.width * self.scale,
            height: Self.baseSize.height * self.scale
        )
        view = PetOverlayView(frame: NSRect(origin: .zero, size: size))
        panel = PetOverlayPanel(contentView: view, size: size)
    }

    public var objectSize: WorldSize {
        WorldSize(width: Self.baseSize.width * scale, height: Self.baseSize.height * scale)
    }

    public func setFrameImage(_ image: CGImage?) {
        view.setImage(image)
    }

    public func setPosition(_ position: WorldPoint) {
        worldPosition = position
        let appKitCenter = coordinateSpace.pointToAppKit(position)
        let origin = NSPoint(
            x: appKitCenter.x - panel.frame.width / 2,
            y: appKitCenter.y - panel.frame.height / 2
        )
        panel.setFrameOrigin(origin)
    }

    public func setScale(_ newScale: Double) {
        let next = newScale.clamped(to: 0.6...1.8)
        guard abs(next - scale) > 0.001 else { return }
        scale = next
        let size = NSSize(width: Self.baseSize.width * scale, height: Self.baseSize.height * scale)
        panel.setContentSize(size)
        view.frame = NSRect(origin: .zero, size: size)
        setPosition(worldPosition)
    }

    public func setVisible(_ visible: Bool) {
        if visible {
            panel.orderFrontRegardless()
        } else {
            panel.orderOut(nil)
        }
    }

    public func setInteractionEnabled(_ enabled: Bool) {
        guard enabled != interactionEnabled else { return }
        interactionEnabled = enabled
        panel.ignoresMouseEvents = !enabled
    }

    public func containsPet(atWorldPoint worldPoint: WorldPoint) -> Bool {
        let appKit = coordinateSpace.pointToAppKit(worldPoint)
        guard panel.frame.contains(NSPoint(x: appKit.x, y: appKit.y)) else { return false }
        let windowPoint = panel.convertPoint(fromScreen: NSPoint(x: appKit.x, y: appKit.y))
        let viewPoint = view.convert(windowPoint, from: nil)
        return view.containsPet(at: viewPoint)
    }
}
