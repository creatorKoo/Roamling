// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import AppKit
import RoamlingCore
import RoamlingEngine
import RoamlingPet

/// AppKit screen points, before anyone has decided what they mean. Stays
/// inside this file: `MacOverlayProvider` converts and forwards world points
/// to the runtime.
@MainActor
protocol PetOverlayViewDelegate: AnyObject {
    func petOverlayMouseDown(screenPoint: NSPoint)
    func petOverlayDragged(screenPoint: NSPoint, distance: CGFloat)
    func petOverlayMouseUp(screenPoint: NSPoint, wasDragged: Bool)
}

@MainActor
public final class PetOverlayView: NSView {
    weak var delegate: PetOverlayViewDelegate?

    public var hitRegionScale: Double = 1 {
        didSet {
            let normalized = hitRegionScale.clamped(to: 0.75...1.3)
            if normalized != hitRegionScale { hitRegionScale = normalized }
        }
    }

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
        let baseHitRect = bounds.insetBy(dx: bounds.width * 0.13, dy: bounds.height * 0.08)
            .offsetBy(dx: 0, dy: bounds.height * 0.03)
        let hitSize = NSSize(
            width: min(bounds.width, baseHitRect.width * hitRegionScale),
            height: min(bounds.height, baseHitRect.height * hitRegionScale)
        )
        let hitRect = NSRect(
            x: min(max(baseHitRect.midX - hitSize.width / 2, bounds.minX), bounds.maxX - hitSize.width),
            y: min(max(baseHitRect.midY - hitSize.height / 2, bounds.minY), bounds.maxY - hitSize.height),
            width: hitSize.width,
            height: hitSize.height
        )
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
public final class MacOverlayProvider: PetOverlayProviding, PetOverlayViewDelegate {
    public static let baseSize = WorldSize(width: 96, height: 104)

    public let view: PetOverlayView
    public let panel: PetOverlayPanel

    public private(set) var scale: Double
    public weak var inputHandler: (any PetOverlayInputHandling)?

    private let readCoordinateSpace: () -> DesktopCoordinateSpace
    private var coordinateSpace: DesktopCoordinateSpace { readCoordinateSpace() }
    private struct FrameKey: Hashable {
        let sheet: ObjectIdentifier
        let x: Int
        let y: Int
    }

    private var sheetImages: [ObjectIdentifier: CGImage] = [:]
    private var frameImages: [FrameKey: CGImage] = [:]
    private var worldPosition: WorldPoint = .zero
    private var interactionEnabled = false

    public init(
        scale: Double = 1,
        hitRegionScale: Double = 1,
        coordinateSpace: @escaping () -> DesktopCoordinateSpace
    ) {
        readCoordinateSpace = coordinateSpace
        self.scale = scale.clamped(to: 0.6...1.8)
        let size = NSSize(
            width: Self.baseSize.width * self.scale,
            height: Self.baseSize.height * self.scale
        )
        view = PetOverlayView(frame: NSRect(origin: .zero, size: size))
        view.hitRegionScale = hitRegionScale
        panel = PetOverlayPanel(contentView: view, size: size)
        view.delegate = self
    }

    private func worldPoint(fromScreen point: NSPoint) -> WorldPoint {
        coordinateSpace.pointFromAppKit(WorldPoint(x: Double(point.x), y: Double(point.y)))
    }

    func petOverlayMouseDown(screenPoint: NSPoint) {
        inputHandler?.petOverlayPointerDown(at: worldPoint(fromScreen: screenPoint))
    }

    func petOverlayDragged(screenPoint: NSPoint, distance: CGFloat) {
        inputHandler?.petOverlayPointerDragged(
            to: worldPoint(fromScreen: screenPoint),
            distance: Double(distance)
        )
    }

    func petOverlayMouseUp(screenPoint: NSPoint, wasDragged: Bool) {
        inputHandler?.petOverlayPointerUp(
            at: worldPoint(fromScreen: screenPoint),
            wasDragged: wasDragged
        )
    }

    public var objectSize: WorldSize {
        WorldSize(width: Self.baseSize.width * scale, height: Self.baseSize.height * scale)
    }

    /// One CGImage per sheet, then a crop per cell -- crops share the sheet's
    /// backing store, so a whole pet costs one copy of its bytes.
    ///
    /// The crops are cached because the view skips a redraw only when handed
    /// the identical image, and `cropping(to:)` returns a fresh one each call.
    public func setFrameImage(_ frame: PetFrame?) {
        guard let frame else {
            view.setImage(nil)
            return
        }
        let sheetKey = ObjectIdentifier(frame.sheet)
        let key = FrameKey(sheet: sheetKey, x: frame.x, y: frame.y)
        if let cached = frameImages[key] {
            view.setImage(cached)
            return
        }
        let sheet: CGImage
        if let cached = sheetImages[sheetKey] {
            sheet = cached
        } else {
            guard let converted = MacPetImageSource.cgImage(of: frame.sheet) else { return }
            // Bounded rather than cleared on pet reload: the overlay is never
            // told the pet changed, and a pet is two sheets.
            if sheetImages.count > 8 {
                sheetImages.removeAll(keepingCapacity: true)
                frameImages.removeAll(keepingCapacity: true)
            }
            sheetImages[sheetKey] = converted
            sheet = converted
        }
        guard let cropped = sheet.cropping(to: CGRect(
            x: frame.x, y: frame.y, width: frame.width, height: frame.height
        )) else { return }
        frameImages[key] = cropped
        view.setImage(cropped)
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

    public func setHitRegionScale(_ newScale: Double) {
        view.hitRegionScale = newScale
    }

    public func containsPet(atWorldPoint worldPoint: WorldPoint) -> Bool {
        let appKit = coordinateSpace.pointToAppKit(worldPoint)
        guard panel.frame.contains(NSPoint(x: appKit.x, y: appKit.y)) else { return false }
        let windowPoint = panel.convertPoint(fromScreen: NSPoint(x: appKit.x, y: appKit.y))
        let viewPoint = view.convert(windowPoint, from: nil)
        return view.containsPet(at: viewPoint)
    }
}
