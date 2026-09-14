// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import AppKit
import RoamlingEngine
import RoamlingShell
import SwiftUI

/// The sliders, as the Windows shell has them.
///
/// Three groups -- markings, body, eyes -- each with a line saying what it is
/// for, five axes, and a button that hands off to the system colour picker.
/// The words are the same keys the Windows window reads, so the two say the
/// same thing without either restating it.
///
/// Reached from a held Option, because nine presets and a colour picker cover
/// what was actually asked for and this is for going further than that.
@MainActor
public final class PaletteWindowController: NSWindowController {
    private let model: PaletteViewModel

    public init(runtime: RoamlingRuntime) {
        let model = PaletteViewModel(runtime: runtime)
        self.model = model

        let window = NSWindow(
            // Three groups of five sliders do not fit a screen's worth of
            // height, so unlike the tuning panel this one expects to scroll.
            contentRect: NSRect(x: 0, y: 0, width: 560, height: 640),
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )
        window.title = localized("palette.window.title")
        window.isReleasedWhenClosed = false
        window.center()
        window.setFrameAutosaveName("RoamlingPalette")
        window.contentView = NSHostingView(rootView: PaletteView(
            model: model,
            onDone: { [weak window] in window?.close() }
        ))

        super.init(window: window)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) is unavailable")
    }

    public func present() {
        model.reload()
        showWindow(nil)
        window?.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
    }
}

@MainActor
private final class PaletteViewModel: ObservableObject {
    /// Bumped whenever the palette moves, so the swatches redraw. The values
    /// themselves are read straight from the runtime -- keeping a second copy
    /// here is how a slider and a cat end up disagreeing.
    @Published private(set) var revision = 0

    let runtime: RoamlingRuntime

    init(runtime: RoamlingRuntime) {
        self.runtime = runtime
    }

    func reload() { revision &+= 1 }

    func binding(_ part: RoamlingRuntime.PalettePart, _ axis: RoamlingRuntime.PaletteAxis) -> Binding<Double> {
        Binding(
            get: { self.runtime.paletteValue(part, axis) },
            set: { [weak self] value in
                guard let self else { return }
                runtime.setPaletteValue(part, axis, value)
                reload()
            }
        )
    }

    func swatch(_ part: RoamlingRuntime.PalettePart) -> Color {
        let rgb = runtime.paletteSwatch(part)
        return Color(
            .sRGB,
            red: Double(rgb.red) / 255,
            green: Double(rgb.green) / 255,
            blue: Double(rgb.blue) / 255
        )
    }

    /// Hands the part to the system picker. Same panel the menu opens, so a
    /// colour chosen here and a colour chosen there are the same act.
    func pick(_ part: RoamlingRuntime.PalettePart) {
        let rgb = runtime.paletteSwatch(part)
        let panel = NSColorPanel.shared
        panel.color = NSColor(
            srgbRed: CGFloat(rgb.red) / 255,
            green: CGFloat(rgb.green) / 255,
            blue: CGFloat(rgb.blue) / 255,
            alpha: 1
        )
        panel.showsAlpha = false
        panel.isContinuous = true
        panel.setTarget(self)
        panel.setAction(#selector(picked(_:)))
        pickingPart = part
        panel.makeKeyAndOrderFront(nil)
    }

    private var pickingPart: RoamlingRuntime.PalettePart?

    @objc private func picked(_ panel: NSColorPanel) {
        guard let pickingPart,
              let colour = panel.color.usingColorSpace(.sRGB) else { return }
        runtime.aimPalette(pickingPart, at: RoamlingRuntime.PaletteRGB(
            red: UInt8((colour.redComponent * 255).rounded()),
            green: UInt8((colour.greenComponent * 255).rounded()),
            blue: UInt8((colour.blueComponent * 255).rounded())
        ))
        reload()
    }

    func reset() {
        runtime.resetPalette()
        reload()
    }
}

private struct PaletteView: View {
    @ObservedObject fileprivate var model: PaletteViewModel
    let onDone: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            VStack(alignment: .leading, spacing: 8) {
                Text(localized("palette.header")).font(.headline)
                Text(localized("palette.footer"))
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .padding(20)

            Divider()

            ScrollView {
                VStack(alignment: .leading, spacing: 24) {
                    ForEach(Array(RoamlingRuntime.PalettePart.allCases.enumerated()), id: \.offset) { _, part in
                        section(part)
                    }
                }
                .padding(20)
            }

            Divider()

            HStack {
                Button(localized("palette.reset")) { model.reset() }
                Spacer()
                Button(localized("palette.done"), action: onDone)
                    .keyboardShortcut(.defaultAction)
            }
            .padding(20)
        }
        .frame(minWidth: 520, minHeight: 420)
    }

    private func section(_ part: RoamlingRuntime.PalettePart) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 10) {
                // The dot is built from the same arithmetic the recolour uses,
                // or it advertises a colour the cat is not wearing.
                Circle()
                    .fill(model.swatch(part))
                    .overlay(Circle().strokeBorder(.separator))
                    .frame(width: 18, height: 18)
                Text(localized(part.menuKey)).font(.headline)
                Spacer()
                Button(localized("palette.pick")) { model.pick(part) }
            }
            Text(localized(hintKey(part)))
                .font(.callout)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)

            ForEach(Array(RoamlingRuntime.PaletteAxis.allCases.enumerated()), id: \.offset) { _, axis in
                if axis.applies(to: part) {
                    HStack(spacing: 12) {
                        Text(localized(axis.labelKey))
                            .frame(width: 90, alignment: .leading)
                        Slider(value: model.binding(part, axis), in: axis.range)
                        Text("\(Int(model.runtime.paletteValue(part, axis).rounded()))")
                            .monospacedDigit()
                            .foregroundStyle(.secondary)
                            .frame(width: 44, alignment: .trailing)
                    }
                }
            }
        }
    }

    private func hintKey(_ part: RoamlingRuntime.PalettePart) -> String {
        switch part {
        case .marking: return "palette.hint.marking"
        case .body: return "palette.hint.body"
        case .eye: return "palette.hint.eye"
        }
    }
}
