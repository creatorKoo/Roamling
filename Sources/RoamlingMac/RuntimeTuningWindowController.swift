// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import AppKit
import RoamlingCore
import SwiftUI

@MainActor
public final class RuntimeTuningWindowController: NSWindowController {
    private let model: RuntimeTuningViewModel

    public init(
        tuning: RuntimeTuning,
        onChange: @escaping @MainActor (RuntimeTuning) -> Void
    ) {
        let model = RuntimeTuningViewModel(tuning: tuning, onChange: onChange)
        self.model = model

        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 520, height: 610),
            styleMask: [.titled, .closable, .miniaturizable],
            backing: .buffered,
            defer: false
        )
        window.title = "Roamling Behavior Tuning"
        window.isReleasedWhenClosed = false
        window.center()
        window.setFrameAutosaveName("RoamlingBehaviorTuning")
        window.contentView = NSHostingView(rootView: RuntimeTuningView(
            model: model,
            onDone: { [weak window] in window?.close() }
        ))

        super.init(window: window)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) is unavailable")
    }

    public func present(tuning: RuntimeTuning) {
        model.replace(with: tuning, notify: false)
        showWindow(nil)
        window?.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
    }
}

@MainActor
private final class RuntimeTuningViewModel: ObservableObject {
    @Published private(set) var tuning: RuntimeTuning

    private let onChange: @MainActor (RuntimeTuning) -> Void

    init(
        tuning: RuntimeTuning,
        onChange: @escaping @MainActor (RuntimeTuning) -> Void
    ) {
        self.tuning = tuning.normalized
        self.onChange = onChange
    }

    func binding(_ keyPath: WritableKeyPath<RuntimeTuning, Double>) -> Binding<Double> {
        Binding(
            get: { self.tuning[keyPath: keyPath] },
            set: { [weak self] value in
                guard let self else { return }
                var updated = self.tuning
                updated[keyPath: keyPath] = value
                self.replace(with: updated, notify: true)
            }
        )
    }

    func replace(with proposed: RuntimeTuning, notify: Bool) {
        let normalized = proposed.normalized
        guard normalized != tuning else { return }
        tuning = normalized
        if notify { onChange(normalized) }
    }

    func reset() {
        replace(with: .standard, notify: true)
    }
}

@MainActor
private struct RuntimeTuningView: View {
    @ObservedObject var model: RuntimeTuningViewModel
    let onDone: @MainActor () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("MVP 0 / 0.5 feel lab")
                .font(.title2.weight(.semibold))
            Text("Changes apply immediately and are saved. This panel tunes roaming and pointer interaction only; MVP 0.7 rest timing stays fixed during its first validation pass.")
                .font(.callout)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)

            Divider()
            Text("Movement")
                .font(.headline)
            TuningSliderRow(
                title: "Walk speed",
                value: model.binding(\.walkingSpeed),
                range: 20...160,
                step: 1,
                style: .pointsPerSecond
            )
            TuningSliderRow(
                title: "Pause between walks",
                value: model.binding(\.wanderPause),
                range: 2...40,
                step: 1,
                style: .seconds
            )
            TuningSliderRow(
                title: "Other-display trips",
                value: model.binding(\.crossDisplayWanderChance),
                range: 0...1,
                step: 0.01,
                style: .percent
            )

            Divider()
            Text("Pointer & Catch")
                .font(.headline)
            TuningSliderRow(
                title: "Notice distance",
                value: model.binding(\.pointerAwarenessDistance),
                range: 140...360,
                step: 5,
                style: .points
            )
            TuningSliderRow(
                title: "Catch arm radius",
                value: model.binding(\.catchArmDistance),
                range: 40...140,
                step: 2,
                style: .points
            )
            TuningSliderRow(
                title: "Catch speed needed",
                value: model.binding(\.catchApproachSpeed),
                range: 150...900,
                step: 10,
                style: .pointsPerSecond
            )
            TuningSliderRow(
                title: "Catch window",
                value: model.binding(\.catchWindow),
                range: 0.15...1.2,
                step: 0.05,
                style: .secondsDecimal
            )
            TuningSliderRow(
                title: "Hit region",
                value: model.binding(\.hitRegionScale),
                range: 0.75...1.3,
                step: 0.01,
                style: .multiplier
            )
            Text("A lower catch-speed requirement and a larger arm radius/window make trackpad catching easier. Normal UI remains click-through until a fast approach is detected over the pet.")
                .font(.caption)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)

            Spacer(minLength: 0)
            HStack {
                Button("Reset Defaults") { model.reset() }
                Spacer()
                Button("Done", action: onDone)
                    .keyboardShortcut(.defaultAction)
            }
        }
        .padding(20)
        .frame(minWidth: 500, minHeight: 590)
    }
}

private struct TuningSliderRow: View {
    let title: String
    @Binding var value: Double
    let range: ClosedRange<Double>
    let step: Double
    let style: TuningValueStyle

    var body: some View {
        HStack(spacing: 12) {
            Text(title)
                .frame(width: 145, alignment: .leading)
            Slider(value: $value, in: range, step: step)
            Text(style.string(for: value))
                .monospacedDigit()
                .frame(width: 78, alignment: .trailing)
        }
    }
}

private enum TuningValueStyle {
    case pointsPerSecond
    case seconds
    case secondsDecimal
    case percent
    case points
    case multiplier

    func string(for value: Double) -> String {
        switch self {
        case .pointsPerSecond:
            "\(Int(value.rounded())) pt/s"
        case .seconds:
            "\(Int(value.rounded())) s"
        case .secondsDecimal:
            String(format: "%.2f s", value)
        case .percent:
            "\(Int((value * 100).rounded()))%"
        case .points:
            "\(Int(value.rounded())) pt"
        case .multiplier:
            String(format: "%.2f×", value)
        }
    }
}
