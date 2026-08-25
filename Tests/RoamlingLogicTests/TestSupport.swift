// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation

struct LogicTest {
    let name: String
    let body: () throws -> Void
}

struct LogicTestFailure: Error, CustomStringConvertible {
    let message: String
    let file: StaticString
    let line: UInt

    var description: String { "\(file):\(line): \(message)" }
}

func expect(
    _ condition: @autoclosure () -> Bool,
    _ message: String = "Expectation failed",
    file: StaticString = #filePath,
    line: UInt = #line
) throws {
    if !condition() {
        throw LogicTestFailure(message: message, file: file, line: line)
    }
}

func expectNear(
    _ lhs: Double,
    _ rhs: Double,
    tolerance: Double = 0.001,
    file: StaticString = #filePath,
    line: UInt = #line
) throws {
    try expect(
        abs(lhs - rhs) <= tolerance,
        "Expected \(lhs) to be within \(tolerance) of \(rhs)",
        file: file,
        line: line
    )
}

func require<T>(
    _ value: T?,
    _ message: String = "Required value was nil",
    file: StaticString = #filePath,
    line: UInt = #line
) throws -> T {
    guard let value else { throw LogicTestFailure(message: message, file: file, line: line) }
    return value
}
