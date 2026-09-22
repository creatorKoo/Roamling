// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import AppKit
import RoamlingEngine

@MainActor
private final class PetEffectsView: NSView {
    var effects: [PetEffectFrame] = [] {
        didSet { needsDisplay = true }
    }

    override var isFlipped: Bool { true }
    override func hitTest(_ point: NSPoint) -> NSView? { nil }

    override func draw(_ dirtyRect: NSRect) {
        super.draw(dirtyRect)
        NSGraphicsContext.current?.cgContext.clear(bounds)
        let bodyWidth = bounds.width / 2
        for effect in effects where effect.points.count >= 3 && effect.opacity > 0 {
            let path = NSBezierPath()
            for (index, point) in effect.points.enumerated() {
                let target = NSPoint(
                    x: bodyWidth + point.x * bodyWidth,
                    y: bounds.height * 0.75 + point.y * bodyWidth
                )
                if index == 0 { path.move(to: target) } else { path.line(to: target) }
            }
            path.close()
            NSColor(
                srgbRed: Double(effect.red) / 255,
                green: Double(effect.green) / 255,
                blue: Double(effect.blue) / 255,
                alpha: effect.opacity
            ).setFill()
            path.fill()
        }
    }
}

/// One child panel for all particles. It never changes the pet's body or hit region.
@MainActor
final class PetEffectsOverlay {
    private let view: PetEffectsView
    private let panel: PetOverlayPanel
    private var showing = false

    init() {
        view = PetEffectsView(frame: .zero)
        panel = PetOverlayPanel(contentView: view, size: NSSize(width: 1, height: 1))
        panel.ignoresMouseEvents = true
    }

    func update(_ effects: [PetEffectFrame], owner: NSPanel, visible: Bool) {
        guard visible && !effects.isEmpty else {
            guard showing else { return }
            showing = false
            view.effects = []
            if panel.parent != nil { panel.parent?.removeChildWindow(panel) }
            panel.orderOut(nil)
            return
        }
        let body = owner.frame.size
        let size = NSSize(width: body.width * 2, height: body.height * 2)
        let origin = NSPoint(x: owner.frame.midX - body.width, y: owner.frame.midY - body.height * 0.5)
        panel.setFrame(NSRect(origin: origin, size: size), display: false)
        view.frame = NSRect(origin: .zero, size: size)
        view.effects = effects
        if panel.parent !== owner { owner.addChildWindow(panel, ordered: .above) }
        if !showing { panel.orderFrontRegardless() }
        showing = true
    }
}
