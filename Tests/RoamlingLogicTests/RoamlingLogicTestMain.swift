// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation
// `exit` only. Same shape as `LoopbackSocket.swift`, which is the module this
// harness has to keep buildable wherever the code it tests builds.
#if canImport(Darwin)
import Darwin
#elseif canImport(Glibc)
import Glibc
#elseif canImport(WinSDK)
import WinSDK
#endif

@main
struct RoamlingLogicTestMain {
    static func main() {
        let tests = coreLogicTests() + petLogicTests() + sourceLogicTests() + runtimeLogicTests() + tuningPersistenceLogicTests() + shellLogicTests() + focusActivityLogicTests() + stuckTravelLogicTests() + runtimeTraceTests()
        var failures = 0

        for test in tests {
            do {
                try test.body()
                print("✓ \(test.name)")
            } catch {
                failures += 1
                print("✗ \(test.name)")
                print("  \(error)")
            }
        }

        if failures == 0 {
            print("\n\(tests.count) tests passed")
        } else {
            print("\n\(failures) of \(tests.count) tests failed")
            exit(1)
        }
    }
}
