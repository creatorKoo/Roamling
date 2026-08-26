// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation
import RoamlingCore
import RoamlingSources

func sourceLogicTests() -> [LogicTest] {
    [
        LogicTest(name: "Claude hook normalizes lifecycle without content metadata") {
            let data = Data(#"""
            {
              "session_id":"session-1",
              "prompt_id":"prompt-1",
              "hook_event_name":"PreToolUse",
              "transcript_path":"/private/conversation.jsonl",
              "tool_name":"Edit",
              "tool_input":{"file_path":"/private/source.swift","new_string":"secret"},
              "prompt":"private prompt"
            }
            """#.utf8)
            let event = try require(try ClaudeCodeEventNormalizer.event(from: data, timestamp: 12))
            try expect(event.sourceID == "claude-code:session-1")
            try expect(event.sourceType == .agent)
            try expect(event.kind == .activityStarted)
            try expectNear(event.intensity, 0.72)
            try expect(event.context == .working)
            try expect(event.metadata.isEmpty)
        },
        LogicTest(name: "Claude hook maps attention completion and failure") {
            func event(_ name: String, extra: String = "") throws -> CompanionEvent? {
                let data = Data("{\"session_id\":\"s\",\"hook_event_name\":\"\(name)\"\(extra)}".utf8)
                return try ClaudeCodeEventNormalizer.event(from: data, timestamp: 1)
            }
            let permission = try event("PermissionRequest")
            let stop = try event("Stop")
            let failure = try event("StopFailure")
            let end = try event("SessionEnd")
            let ignored = try event("Notification", extra: ",\"notification_type\":\"other\"")
            try expect(permission?.kind == .attentionRequired)
            try expect(stop?.kind == .achievement)
            try expect(failure?.kind == .negative)
            try expect(end?.kind == .activityEnded)
            try expect(ignored == nil)
        },
        LogicTest(name: "Claude loopback receiver authenticates and emits event") {
            let port = UInt16.random(in: 49_000...59_000)
            let source = ClaudeCodeSource(token: "loopback-test-token", port: port)
            defer { source.stop() }
            try source.start()
            for _ in 0..<50 where source.state != .ready {
                Thread.sleep(forTimeInterval: 0.02)
            }
            try expect(source.state == .ready, "Receiver state: \(source.state)")

            let received = LockedBox<CompanionEvent?>(nil)
            let eventSignal = DispatchSemaphore(value: 0)
            let streamTask = Task.detached {
                for await event in source.makeEventStream() {
                    received.set(event)
                    eventSignal.signal()
                    break
                }
            }
            defer { streamTask.cancel() }

            var request = URLRequest(url: URL(
                string: "http://127.0.0.1:\(port)\(ClaudeCodeHookInstaller.path)"
            )!)
            request.httpMethod = "POST"
            request.setValue("application/json", forHTTPHeaderField: "Content-Type")
            request.setValue("loopback-test-token", forHTTPHeaderField: ClaudeCodeHookInstaller.tokenHeader)
            request.httpBody = Data("{\"session_id\":\"http-session\",\"hook_event_name\":\"Stop\"}".utf8)

            let responseStatus = LockedBox<Int?>(nil)
            let responseSignal = DispatchSemaphore(value: 0)
            URLSession.shared.dataTask(with: request) { _, response, _ in
                responseStatus.set((response as? HTTPURLResponse)?.statusCode)
                responseSignal.signal()
            }.resume()
            try expect(responseSignal.wait(timeout: .now() + 3) == .success)
            try expect(responseStatus.value == 204)
            try expect(eventSignal.wait(timeout: .now() + 3) == .success)
            try expect(received.value?.sourceID == "claude-code:http-session")
            try expect(received.value?.kind == .achievement)
        },
        LogicTest(name: "Claude hook install is idempotent and preserves siblings") {
            let folder = FileManager.default.temporaryDirectory
                .appendingPathComponent("roamling-hooks-\(UUID().uuidString)", isDirectory: true)
            defer { try? FileManager.default.removeItem(at: folder) }
            try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
            let settings = folder.appendingPathComponent("settings.json")
            let original = Data(#"""
            {
              "model":"sonnet",
              "hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"true"}]}]}
            }
            """#.utf8)
            try original.write(to: settings)
            let installer = ClaudeCodeHookInstaller(settingsURL: settings, token: "test-token")

            try installer.install()
            try installer.install()
            try expect(installer.status() == .installed)
            try expect(FileManager.default.fileExists(atPath: installer.backupURL.path))

            let installed = try jsonRoot(at: settings)
            try expect(installed["model"] as? String == "sonnet")
            let hooks = try require(installed["hooks"] as? [String: Any])
            let preToolGroups = try require(hooks["PreToolUse"] as? [[String: Any]])
            let handlers = preToolGroups.flatMap { $0["hooks"] as? [[String: Any]] ?? [] }
            try expect(handlers.filter { $0["type"] as? String == "command" }.count == 1)
            try expect(handlers.filter { $0["url"] as? String == installer.endpoint.absoluteString }.count == 1)

            try installer.remove()
            try expect(installer.status() == .notInstalled)
            let removed = try jsonRoot(at: settings)
            let removedHooks = try require(removed["hooks"] as? [String: Any])
            let remainingGroups = try require(removedHooks["PreToolUse"] as? [[String: Any]])
            let remaining = remainingGroups.flatMap { $0["hooks"] as? [[String: Any]] ?? [] }
            try expect(remaining.count == 1)
            try expect(remaining[0]["command"] as? String == "true")
        },
        LogicTest(name: "Codex hook normalizes lifecycle without content metadata") {
            let data = Data(#"""
            {
              "session_id":"thread-1",
              "turn_id":"turn-1",
              "hook_event_name":"PreToolUse",
              "transcript_path":"/private/conversation.jsonl",
              "tool_name":"exec_command",
              "tool_input":{"cmd":"cat private.swift"},
              "prompt":"private prompt"
            }
            """#.utf8)
            let event = try require(try CodexEventNormalizer.event(from: data, timestamp: 21))
            try expect(event.sourceID == "codex:thread-1")
            try expect(event.kind == .activityStarted)
            try expectNear(event.intensity, 0.72)
            try expect(event.context == .working)
            try expect(event.metadata.isEmpty)
        },
        LogicTest(name: "Codex hook maps attention completion and session end") {
            func event(_ name: String) throws -> CompanionEvent? {
                let data = Data("{\"session_id\":\"s\",\"turn_id\":\"t\",\"hook_event_name\":\"\(name)\"}".utf8)
                return try CodexEventNormalizer.event(from: data, timestamp: 1)
            }
            let permission = try event("PermissionRequest")
            let stop = try event("Stop")
            let end = try event("SessionEnd")
            let compact = try event("PreCompact")
            try expect(permission?.kind == .attentionRequired)
            try expect(stop?.kind == .achievement)
            try expect(end?.kind == .activityEnded)
            try expect(compact == nil)
        },
        LogicTest(name: "Codex hook install is idempotent and preserves siblings") {
            let folder = FileManager.default.temporaryDirectory
                .appendingPathComponent("roamling-codex-hooks-\(UUID().uuidString)", isDirectory: true)
            defer { try? FileManager.default.removeItem(at: folder) }
            try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
            let hooksURL = folder.appendingPathComponent("hooks.json")
            try Data(#"""
            {
              "description":"existing hooks",
              "hooks":{"PreToolUse":[{"matcher":"exec_command","hooks":[{"type":"command","command":"true"}]}]}
            }
            """#.utf8).write(to: hooksURL)
            let installer = CodexHookInstaller(hooksURL: hooksURL, token: "codex-test-token")

            try installer.install()
            try installer.install()
            try expect(installer.status() == .installed)
            try expect(FileManager.default.fileExists(atPath: installer.backupURL.path))

            let installed = try jsonRoot(at: hooksURL)
            try expect(installed["description"] as? String == "existing hooks")
            let hooks = try require(installed["hooks"] as? [String: Any])
            let groups = try require(hooks["PreToolUse"] as? [[String: Any]])
            let handlers = groups.flatMap { $0["hooks"] as? [[String: Any]] ?? [] }
            try expect(handlers.filter { $0["command"] as? String == "true" }.count == 1)
            try expect(handlers.filter {
                ($0["command"] as? String)?.contains(CodexHookInstaller.marker) == true
            }.count == 1)

            try installer.remove()
            try expect(installer.status() == .notInstalled)
            let removed = try jsonRoot(at: hooksURL)
            let removedHooks = try require(removed["hooks"] as? [String: Any])
            let remainingGroups = try require(removedHooks["PreToolUse"] as? [[String: Any]])
            let remaining = remainingGroups.flatMap { $0["hooks"] as? [[String: Any]] ?? [] }
            try expect(remaining.count == 1)
            try expect(remaining[0]["command"] as? String == "true")
        },
        LogicTest(name: "Codex command hook forwards stdin to loopback receiver") {
            let port = UInt16.random(in: 49_000...59_000)
            let token = "codex-loopback-test-token"
            let source = CodexSource(token: token, port: port)
            defer { source.stop() }
            try source.start()
            for _ in 0..<50 where source.state != .ready {
                Thread.sleep(forTimeInterval: 0.02)
            }
            try expect(source.state == .ready, "Receiver state: \(source.state)")

            let received = LockedBox<CompanionEvent?>(nil)
            let eventSignal = DispatchSemaphore(value: 0)
            let streamTask = Task.detached {
                for await event in source.makeEventStream() {
                    received.set(event)
                    eventSignal.signal()
                    break
                }
            }
            defer { streamTask.cancel() }

            let installer = CodexHookInstaller(
                hooksURL: FileManager.default.temporaryDirectory.appendingPathComponent("unused-hooks.json"),
                token: token,
                port: port
            )
            let process = Process()
            process.executableURL = URL(fileURLWithPath: "/bin/sh")
            process.arguments = ["-c", installer.command]
            let input = Pipe()
            process.standardInput = input
            try process.run()
            input.fileHandleForWriting.write(Data(
                "{\"session_id\":\"command-session\",\"turn_id\":\"turn-1\",\"hook_event_name\":\"Stop\"}".utf8
            ))
            try input.fileHandleForWriting.close()
            process.waitUntilExit()

            try expect(process.terminationStatus == 0)
            try expect(eventSignal.wait(timeout: .now() + 3) == .success)
            try expect(received.value?.sourceID == "codex:command-session")
            try expect(received.value?.kind == .achievement)
        },
        LogicTest(name: "interest placement stays near window edge and avoids pointer") {
            let display = DisplaySnapshot(
                id: "main",
                name: "main",
                frame: WorldRect(x: 0, y: 0, width: 1_200, height: 900),
                visibleFrame: WorldRect(x: 0, y: 24, width: 1_200, height: 830),
                scale: 2
            )
            let window = WorldRect(x: 180, y: 100, width: 800, height: 620)
            let destination = try require(BasicInterestPositionPlanner.destination(
                for: LocationHint(approximateRegion: window, confidence: 0.55),
                in: DesktopWorldSnapshot(displays: [display]),
                currentPosition: WorldPoint(x: 600, y: 300),
                pointerPosition: WorldPoint(x: 120, y: 650),
                objectSize: WorldSize(width: 96, height: 104)
            ))
            try expect(destination.displayID == "main")
            try expect(display.visibleFrame.insetBy(dx: 58, dy: 62).contains(destination.point))
            try expect(destination.point.y >= window.maxY - 80)
            try expect(destination.point.distance(to: WorldPoint(x: 120, y: 650)) > 300)
        },
        LogicTest(name: "behavior enters interest travel without overriding drag") {
            var behavior = BehaviorController(state: .idle, enteredAt: 0)
            behavior.handle(.beginInterestTravel, at: 1)
            try expect(behavior.state == .travelToInterest)
            behavior.handle(.catchBegan, at: 2)
            behavior.handle(.dragMoved, at: 2.1)
            behavior.handle(.beginInterestTravel, at: 2.2)
            try expect(behavior.state == .dragged)
        }
    ]
}

private func jsonRoot(at url: URL) throws -> [String: Any] {
    let object = try JSONSerialization.jsonObject(with: Data(contentsOf: url))
    return try require(object as? [String: Any])
}

private final class LockedBox<Value>: @unchecked Sendable {
    private let lock = NSLock()
    private var storage: Value

    init(_ value: Value) {
        storage = value
    }

    var value: Value {
        lock.lock()
        defer { lock.unlock() }
        return storage
    }

    func set(_ value: Value) {
        lock.lock()
        storage = value
        lock.unlock()
    }
}
