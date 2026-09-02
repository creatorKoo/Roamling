// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import AppKit
import RoamlingCore
import RoamlingShell
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
        window.title = localized("tuning.window.title")
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

    func binding(_ key: RuntimeTuningKey) -> Binding<Double> {
        Binding(
            get: { self.tuning[key] },
            set: { [weak self] value in
                guard let self else { return }
                var updated = self.tuning
                updated[key] = value
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
            Text(localized("tuning.header"))
                .font(.title2.weight(.semibold))
            Text(localized("tuning.footer"))
                .font(.callout)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)

            Divider()
            Text(localized("tuning.section.movement"))
                .font(.headline)
            TuningSliderRow(
                title: localized("tuning.walkSpeed"),
                value: model.binding(.walkingSpeed),
                range: model.tuning.limits(for: .walkingSpeed),
                step: 1,
                style: .pointsPerSecond
            )
            TuningSliderRow(
                title: localized("tuning.wanderPause"),
                value: model.binding(.wanderPause),
                range: model.tuning.limits(for: .wanderPause),
                step: 1,
                style: .seconds
            )
            TuningSliderRow(
                title: localized("tuning.crossDisplay"),
                value: model.binding(.crossDisplayWanderChance),
                range: model.tuning.limits(for: .crossDisplayWanderChance),
                step: 0.01,
                style: .percent
            )
            TuningSliderRow(
                title: localized("tuning.idleBeforeRest"),
                value: model.binding(.idleBeforeRest),
                range: model.tuning.limits(for: .idleBeforeRest),
                step: 5,
                style: .seconds
            )

            Divider()
            Text(localized("tuning.section.pointer"))
                .font(.headline)
            TuningSliderRow(
                title: localized("tuning.noticeDistance"),
                value: model.binding(.pointerAwarenessDistance),
                range: model.tuning.limits(for: .pointerAwarenessDistance),
                step: 5,
                style: .points
            )
            TuningSliderRow(
                title: localized("tuning.evadeSpeed"),
                value: model.binding(.evadeSpeedScale),
                range: model.tuning.limits(for: .evadeSpeedScale),
                step: 0.05,
                style: .multiplier
            )
            TuningSliderRow(
                title: localized("tuning.catchArm"),
                value: model.binding(.catchArmDistance),
                range: model.tuning.limits(for: .catchArmDistance),
                step: 2,
                style: .points
            )
            TuningSliderRow(
                title: localized("tuning.catchSpeed"),
                value: model.binding(.catchApproachSpeed),
                range: model.tuning.limits(for: .catchApproachSpeed),
                step: 10,
                style: .pointsPerSecond
            )
            TuningSliderRow(
                title: localized("tuning.catchWindow"),
                value: model.binding(.catchWindow),
                range: model.tuning.limits(for: .catchWindow),
                step: 0.05,
                style: .secondsDecimal
            )
            TuningSliderRow(
                title: localized("tuning.hitRegion"),
                value: model.binding(.hitRegionScale),
                range: model.tuning.limits(for: .hitRegionScale),
                step: 0.01,
                style: .multiplier
            )
            Divider()
            // Parked here until the real options window exists; this panel is
            // the only surface that can carry it today.
            Text(localized("tuning.section.advanced"))
                .font(.headline)
            TuningSliderRow(
                title: localized("tuning.gaitCadence"),
                value: model.binding(.gaitCadence),
                range: model.tuning.limits(for: .gaitCadence),
                step: 0.05,
                style: .multiplier
            )
            Text(localized("tuning.gaitCadenceNote"))
                .font(.caption)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)

            Text(localized("tuning.pointerNote"))
                .font(.caption)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)

            Spacer(minLength: 0)
            HStack {
                Button(localized("button.resetDefaults")) { model.reset() }
                Spacer()
                Button(localized("button.done"), action: onDone)
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
            localizedFormat("unit.speed", Int(value.rounded()))
        case .seconds:
            localizedFormat("unit.seconds", Int(value.rounded()))
        case .secondsDecimal:
            localizedFormat("unit.secondsPrecise", value)
        case .percent:
            "\(Int((value * 100).rounded()))%"
        case .points:
            localizedFormat("unit.points", Int(value.rounded()))
        case .multiplier:
            localizedFormat("unit.multiplier", value)
        }
    }
}
