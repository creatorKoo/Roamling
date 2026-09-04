// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation
import RoamlingCore
import RoamlingEngine
import RoamlingPet

/// Saved settings used to freeze the defaults that were current when they were
/// written. All eleven tuning values went into the file whenever any one of
/// them moved, and a stored value beats a default -- so the wander pause going
/// from 12 to 40 never reached anyone who had opened the panel. Pressing
/// "Reset Defaults" is what wrote the file, so the most careful users were the
/// most thoroughly stuck.
///
/// These fix the shape of the fix: only what was moved is written, and what
/// was not moved keeps taking its answer from the default.
func tuningPersistenceLogicTests() -> [LogicTest] {
    [
        LogicTest(name: "a tuning left at its defaults writes nothing down") {
            try MainActor.assumeIsolated {
                let suite = try makeTestDefaults()
                defer { suite.discard() }
                let runtime = makeTuningRuntime(on: suite)

                // Move something first, so the key exists and its removal is
                // what the test observes rather than its never having existed.
                var moved = RuntimeTuning.standard
                moved.walkingSpeed = 240
                runtime.applyTuning(moved)
                try expect(suite.storedTuning() != nil, "moving a slider wrote nothing")

                runtime.resetTuning()
                try expect(
                    suite.storedTuning() == nil,
                    "Reset Defaults left \(suite.storedTuning() ?? [:]) behind, "
                        + "which is the file that froze the defaults"
                )
            }
        },
        LogicTest(name: "moving one value does not pin the other ten") {
            try MainActor.assumeIsolated {
                let suite = try makeTestDefaults()
                defer { suite.discard() }
                let runtime = makeTuningRuntime(on: suite)

                var moved = RuntimeTuning.standard
                moved.wanderPause = 55
                runtime.applyTuning(moved)

                let stored = try require(suite.storedTuning(), "nothing was written")
                try expect(
                    Array(stored.keys) == ["wanderPause"],
                    "expected only wanderPause, got \(stored.keys.sorted())"
                )
                try expect(stored["wanderPause"] == 55)
            }
        },
        LogicTest(name: "a value nobody moved follows the default") {
            // The point of the whole change. A file that names one value has to
            // leave the other ten answering from `standard`, so that changing
            // one of those defaults reaches a machine that already has a file.
            try MainActor.assumeIsolated {
                let suite = try makeTestDefaults()
                defer { suite.discard() }
                suite.storeTuning(["wanderPause": 55])

                let loaded = makeTuningRuntime(on: suite).tuning
                try expect(loaded.wanderPause == 55, "the moved value did not survive")
                for key in RuntimeTuningKey.allCases where key != .wanderPause {
                    try expect(
                        loaded[key] == RuntimeTuning.standard[key],
                        "\(key) came back as \(loaded[key]) instead of following the "
                            + "default \(RuntimeTuning.standard[key])"
                    )
                }
            }
        },
        LogicTest(name: "a file written by an older build still reads") {
            // Every machine that has ever opened the panel has one of these:
            // all eleven values in one object. It has to keep working, and
            // values that differ from today's defaults have to survive -- a
            // stored 12 cannot be told apart from a deliberate 12.
            try MainActor.assumeIsolated {
                let suite = try makeTestDefaults()
                defer { suite.discard() }

                var old: [String: Double] = [:]
                for key in RuntimeTuningKey.allCases {
                    old[key.rawValue] = RuntimeTuning.standard[key]
                }
                old["wanderPause"] = 12
                old["walkingSpeed"] = 200
                suite.storeTuning(old)

                let loaded = makeTuningRuntime(on: suite).tuning
                try expect(loaded.wanderPause == 12, "the old wander pause was lost")
                try expect(loaded.walkingSpeed == 200, "the old walking speed was lost")
            }
        },
        LogicTest(name: "the stored names are the tuning keys") {
            // Persistence writes `RuntimeTuningKey.rawValue` and the decoder
            // reads `RuntimeTuning`'s synthesized coding keys. They are two
            // lists of the same eleven names, and nothing in the compiler says
            // so -- a rename on one side would quietly stop a value from ever
            // loading again.
            let encoded = try JSONEncoder().encode(RuntimeTuning.standard)
            let object = try require(
                try JSONSerialization.jsonObject(with: encoded) as? [String: Any],
                "a tuning did not encode as an object"
            )
            try expect(
                Set(object.keys) == Set(RuntimeTuningKey.allCases.map(\.rawValue)),
                "encoded \(object.keys.sorted()) but the keys are "
                    + "\(RuntimeTuningKey.allCases.map(\.rawValue).sorted())"
            )
        }
    ]
}

/// A runtime on a given defaults suite and nothing else -- the tuning is the
/// only thing these tests read out of it.
@MainActor
private func makeTuningRuntime(on suite: TestDefaults) -> RoamlingRuntime {
    let platform = FakePlatform(
        display: DisplaySnapshot(
            id: "1", name: "test",
            frame: WorldRect(x: 0, y: 0, width: 1440, height: 900),
            visibleFrame: WorldRect(x: 0, y: 25, width: 1440, height: 850),
            scale: 2
        ),
        worldTop: 900
    )
    return RoamlingRuntime(
        services: platform.services,
        defaults: suite.defaults,
        catalog: PetCatalog(roots: []),
        clock: { 0 }
    )
}

extension TestDefaults {
    func storeTuning(_ values: [String: Double]) {
        defaults.set(try? JSONEncoder().encode(values), forKey: "roamling.runtimeTuning")
    }

    func storedTuning() -> [String: Double]? {
        guard let data = defaults.data(forKey: "roamling.runtimeTuning") else { return nil }
        return try? JSONDecoder().decode([String: Double].self, from: data)
    }
}
