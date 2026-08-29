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
            try expect(handlers.count == 2)
            try expect(handlers.filter {
                ($0["command"] as? String)?.contains(ClaudeCodeHookInstaller.marker) == true
            }.count == 1)
            try expect(handlers.contains { $0["command"] as? String == "true" })

            try installer.remove()
            try expect(installer.status() == .notInstalled)
            let removed = try jsonRoot(at: settings)
            let removedHooks = try require(removed["hooks"] as? [String: Any])
            let remainingGroups = try require(removedHooks["PreToolUse"] as? [[String: Any]])
            let remaining = remainingGroups.flatMap { $0["hooks"] as? [[String: Any]] ?? [] }
            try expect(remaining.count == 1)
            try expect(remaining[0]["command"] as? String == "true")
        },
        LogicTest(name: "Claude hook install replaces the legacy http handler") {
            let folder = FileManager.default.temporaryDirectory
                .appendingPathComponent("roamling-claude-legacy-\(UUID().uuidString)", isDirectory: true)
            defer { try? FileManager.default.removeItem(at: folder) }
            try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
            let settings = folder.appendingPathComponent("settings.json")
            let installer = ClaudeCodeHookInstaller(settingsURL: settings, token: "test-token")
            let legacy = Data(#"""
            {
              "hooks":{"SessionEnd":[{"hooks":[{
                "type":"http",
                "url":"http://127.0.0.1:47831/v1/hooks/claude-code",
                "timeout":2,
                "headers":{"X-Roamling-Token":"test-token"}
              }]}]}
            }
            """#.utf8)
            try legacy.write(to: settings)

            // A pre-command install is stale, not absent, so launch repair can
            // upgrade it without asking the user to opt in again.
            try expect(installer.status() == .needsRepair)

            try installer.install()
            try expect(installer.status() == .installed)

            let repaired = try jsonRoot(at: settings)
            let hooks = try require(repaired["hooks"] as? [String: Any])
            let groups = try require(hooks["SessionEnd"] as? [[String: Any]])
            let handlers = groups.flatMap { $0["hooks"] as? [[String: Any]] ?? [] }
            try expect(handlers.count == 1)
            try expect(handlers[0]["type"] as? String == "command")
            try expect(!handlers.contains { $0["url"] is String })

            let command = try require(handlers[0]["command"] as? String)
            try expect(command.contains(ClaudeCodeHookInstaller.marker))
            try expect(command.contains(ClaudeCodeHookInstaller.tokenHeader))
            // Swallowed failure is the whole point of the migration.
            try expect(command.contains(">/dev/null 2>&1 || true"))
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
        LogicTest(name: "Loopback receiver accepts a large tool payload") {
            let port = UInt16.random(in: 49_000...59_000)
            let token = "codex-large-payload-token"
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

            // A real PostToolUse can carry a large tool_response. curl still
            // frames it with Content-Length below 1 MiB, so the receiver has to
            // accept it instead of rejecting it as oversized.
            let filler = String(repeating: "x", count: 800 * 1_024)
            let payload = "{\"session_id\":\"large-session\",\"turn_id\":\"turn-1\","
                + "\"hook_event_name\":\"Stop\",\"tool_input\":{\"cmd\":\"\(filler)\"}}"
            try expect(payload.utf8.count > 512 * 1_024)
            input.fileHandleForWriting.write(Data(payload.utf8))
            try input.fileHandleForWriting.close()
            process.waitUntilExit()

            try expect(process.terminationStatus == 0)
            try expect(eventSignal.wait(timeout: .now() + 5) == .success)
            try expect(received.value?.sourceID == "codex:large-session")
            try expect(received.value?.kind == .achievement)
            // The oversized field must not survive into the domain event.
            try expect(received.value?.metadata.isEmpty == true)
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
        LogicTest(name: "focus placement refuses to sit on the caret") {
            let fixture = FocusPlacementFixture()

            // Without accessibility the planner picks the inner-left seat.
            let baseline = try require(fixture.destination(focus: nil))
            try expect(baseline.point == WorldPoint(x: 124, y: 726))

            // A caret sitting exactly there has to push the pet somewhere else.
            let caret = WorldRect(x: 121, y: 700, width: 2, height: 40)
            let focused = try require(fixture.destination(
                focus: FocusSnapshot(caretFrame: caret, confidence: 0.9)
            ))
            try expect(focused.point != baseline.point)
            try expect(!fixture.petFrame(at: focused.point).intersects(caret))
            // It still stays on the caret's side rather than fleeing the window.
            try expect(focused.point == WorldPoint(x: 58, y: 726))
        },
        LogicTest(name: "focus placement treats a collapsed caret as an obstacle") {
            let fixture = FocusPlacementFixture()
            // A collapsed insertion point reports zero width. Dropping it as an
            // empty rect would remove the only obstacle that matters.
            let snapshot = FocusSnapshot(
                caretFrame: WorldRect(x: 121, y: 700, width: 0, height: 40),
                confidence: 0.9
            )
            let caret = try require(snapshot.caretFrame)
            try expect(caret.size.width >= 2)

            let focused = try require(fixture.destination(focus: snapshot))
            try expect(focused.point == WorldPoint(x: 58, y: 726))
            try expect(!fixture.petFrame(at: focused.point).intersects(caret))
        },
        LogicTest(name: "focus placement prefers the focused window over the coarse hint") {
            let fixture = FocusPlacementFixture()
            // The coarse hint only knows the frontmost process, so it can point
            // at a stale region while accessibility knows the real window.
            let stale = LocationHint(
                approximateRegion: WorldRect(x: 0, y: 24, width: 100, height: 100),
                confidence: 0.55
            )
            let focused = try require(fixture.destination(
                hint: stale,
                focus: FocusSnapshot(windowFrame: fixture.window, confidence: 0.9)
            ))
            try expect(focused.point.y == 726)

            // Same stale hint without accessibility still lands in the corner.
            let coarse = try require(fixture.destination(hint: stale, focus: nil))
            try expect(coarse.point.y < 200)
        },
        LogicTest(name: "visual placement moves the seat toward empty space") {
            let fixture = FocusPlacementFixture()
            let baseline = try require(fixture.destination(focus: nil))

            // The window's left half is dense; the right half is blank wall.
            let columns = 60
            let rows = 30
            var samples: [Double] = []
            for row in 0..<rows {
                for column in 0..<columns {
                    let busy = column < columns / 2 && (column + row).isMultiple(of: 2)
                    samples.append(busy ? 0.05 : 0.9)
                }
            }
            let field = try require(LuminanceField(
                bounds: fixture.display.frame,
                columns: columns,
                rows: rows,
                samples: samples
            ))

            let placed = try require(fixture.destination(focus: nil, luminance: field))
            try expect(placed.point.x > baseline.point.x, "seat should move off the dense half")
            try expect(placed.point.x > fixture.display.frame.midX)

            // Without a capture the extra sweep does not exist at all.
            try expect(baseline.point.x < fixture.display.frame.midX)
        },
        LogicTest(name: "focus placement falls back when accessibility gives no window") {
            let fixture = FocusPlacementFixture()
            let stale = LocationHint(
                approximateRegion: WorldRect(x: 0, y: 24, width: 100, height: 100),
                confidence: 0.55
            )
            // A collapsed window rect means the query answered without a usable
            // bound, so the coarse hint has to stay in charge of the region.
            let collapsed = FocusSnapshot(
                windowFrame: WorldRect(x: 300, y: 300, width: 0, height: 0),
                confidence: 0.7
            )
            try expect(collapsed.windowFrame == nil)
            let placed = try require(fixture.destination(hint: stale, focus: collapsed))
            try expect(placed.point.y < 200)
        },
        LogicTest(name: "behavior enters interest travel without overriding drag") {
            var behavior = BehaviorController(state: .idle, enteredAt: 0)
            behavior.handle(.beginInterestTravel, at: 1)
            try expect(behavior.state == .travelToInterest)
            behavior.handle(.catchBegan, at: 2)
            behavior.handle(.dragMoved, at: 2.1)
            behavior.handle(.beginInterestTravel, at: 2.2)
            try expect(behavior.state == .dragged)
        },
        LogicTest(name: "a working terminal seats the pet beside the caret, not at the screen edge") {
            // The report this covers: launching an agent sent the pet to a
            // screen corner while the window it was watching had empty room.
            let display = DisplaySnapshot(
                id: "main",
                name: "main",
                frame: WorldRect(x: 0, y: 0, width: 1_512, height: 982),
                visibleFrame: WorldRect(x: 0, y: 25, width: 1_512, height: 932),
                scale: 2
            )
            let window = WorldRect(x: 60, y: 60, width: 1_390, height: 870)
            let caret = WorldRect(x: 300, y: 700, width: 2, height: 20)

            // Output fills the window below the caret line and leaves the rest
            // of it clear, which is what a terminal mid-run looks like.
            let columns = 64
            let rows = 42
            let cellHeight = display.frame.size.height / Double(rows)
            let cellWidth = display.frame.size.width / Double(columns)
            var samples: [Double] = []
            for row in 0..<rows {
                let y = (Double(row) + 0.5) * cellHeight
                for column in 0..<columns {
                    let x = (Double(column) + 0.5) * cellWidth
                    let inText = window.contains(WorldPoint(x: x, y: y))
                        && y > 690
                        && x < 1_000
                    samples.append(inText && (column + row).isMultiple(of: 3) ? 0.78 : 0.86)
                }
            }
            let field = try require(LuminanceField(
                bounds: display.frame, columns: columns, rows: rows, samples: samples
            ))

            let placed = try require(BasicInterestPositionPlanner.destination(
                for: LocationHint(approximateRegion: window, confidence: 0.55),
                in: DesktopWorldSnapshot(
                    displays: [display],
                    focus: FocusSnapshot(
                        windowFrame: window,
                        focusedElementFrame: window,
                        caretFrame: caret,
                        confidence: 0.9
                    ),
                    luminance: field
                ),
                currentPosition: WorldPoint(x: 700, y: 400),
                pointerPosition: WorldPoint(x: 1_400, y: 120),
                objectSize: WorldSize(width: 96, height: 104)
            ))

            // Not flung to the edge of the screen.
            try expect(placed.point.x > 200, "seat landed at the screen edge: \(placed.point)")
            // Close enough to read as watching this work.
            try expect(
                caret.distance(to: placed.point) < 500,
                "seat sat \(caret.distance(to: placed.point)) away from the caret"
            )
            // And not on the output.
            let frame = WorldRect(
                x: placed.point.x - 48, y: placed.point.y - 52, width: 96, height: 104
            )
            let emptiness = try require(VisualEmptiness.score(of: frame, in: field))
            try expect(emptiness > 0.85, "seat scored \(emptiness) on the output")
        },
        LogicTest(name: "seat evaluation tells a clear seat from a covered one") {
            let fixture = FocusPlacementFixture()
            let seat = WorldPoint(x: 124, y: 726)

            // Without a capture nothing saw the pixels, so a seat is not
            // condemned on a guess. That is the MVP 3 path and it stays put.
            let unseen = try require(fixture.seat(at: seat))
            try expect(unseen.isHoldable)

            let clear = try require(fixture.field { _ in false })
            let flat = try require(fixture.seat(at: seat, luminance: clear))
            try expect(flat.isHoldable)

            // Output arrived under the pet while nobody typed anything.
            let buried = try require(fixture.field { $0 > 640 })
            let covered = try require(fixture.seat(at: seat, luminance: buried))
            let emptiness = try require(covered.emptiness)
            try expect(!covered.isHoldable)
            try expect(emptiness < BasicInterestPositionPlanner.holdEmptiness)

            // The caret arriving under the pet is enough on its own.
            let onCaret = try require(fixture.seat(
                at: seat,
                focus: FocusSnapshot(
                    caretFrame: WorldRect(x: 121, y: 700, width: 2, height: 40),
                    confidence: 0.9
                ),
                luminance: clear
            ))
            try expect(onCaret.coversCaret)
            try expect(!onCaret.isHoldable)

            // A seat that no longer belongs to the window unsticks itself, so
            // switching windows still moves the pet.
            let elsewhere = try require(fixture.seat(
                at: WorldPoint(x: 1196, y: 880),
                luminance: clear
            ))
            try expect(!elsewhere.watchesRegion)
            try expect(!elsewhere.isHoldable)
        },
        LogicTest(name: "placement stays out of the caret's path") {
            let fixture = FocusPlacementFixture()
            // The caret shares the row the bottom seats use and keeps moving
            // right, so a seat ahead of it is empty now and inside a sentence
            // a minute from now.
            let ahead = try require(fixture.destination(
                focus: FocusSnapshot(
                    caretFrame: WorldRect(x: 1_000, y: 700, width: 2, height: 40),
                    confidence: 0.9
                )
            ))
            try expect(ahead.point.x < 1_000, "sat in the caret's path at \(ahead.point.x)")

            // Control: the same caret lifted just clear of the seat row is a
            // comparable distance away, so only the shared line changed. The
            // near seat wins again, which is what makes the case above the
            // advance penalty and not simple proximity.
            let clearOfIt = try require(fixture.destination(
                focus: FocusSnapshot(
                    caretFrame: WorldRect(x: 1_000, y: 620, width: 2, height: 40),
                    confidence: 0.9
                )
            ))
            try expect(clearOfIt.point.x > 1_000)
        },
        LogicTest(name: "the sweep leaves the bottom line when the bottom line is full") {
            let fixture = FocusPlacementFixture()
            // Agent output fills the bottom of a terminal and leaves the middle
            // clear. A sweep that only walks the bottom line never sees that.
            let field = try require(fixture.field { $0 > 660 })
            let placed = try require(fixture.destination(focus: nil, luminance: field))
            try expect(placed.point.y < 700, "stayed on the full bottom line at \(placed.point.y)")
            try expect(fixture.petFrame(at: placed.point).maxY < 660)
        },
        LogicTest(name: "a pet watching an agent can still fall asleep on its seat") {
            // Rest used to be reachable only from idle/wander/dropped, so a pet
            // parked beside a working agent stayed awake for the whole run.
            for state in [BehaviorState.observe, .work] {
                var behavior = BehaviorController(state: state, enteredAt: 0)
                behavior.handle(.beginRest, at: 1)
                try expect(behavior.state == .sit, "\(state) refused to sit")
                behavior.handle(.seekSleepSpot, at: 2)
                behavior.handle(.sleepSpotReached, at: 3)
                try expect(behavior.state == .sleep)
            }

            // Being carried is still not a nap.
            var dragged = BehaviorController(state: .dragged, enteredAt: 0)
            dragged.handle(.beginRest, at: 1)
            try expect(dragged.state == .dragged)
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

/// Shared geometry for the MVP 3 caret-aware placement tests. The window fills
/// the safe area so every candidate lands inside it, which keeps the outside
/// bonus out of the comparison and leaves caret handling as the only variable.
struct FocusPlacementFixture {
    let display = DisplaySnapshot(
        id: "main",
        name: "main",
        frame: WorldRect(x: 0, y: 0, width: 1_200, height: 900),
        visibleFrame: WorldRect(x: 0, y: 24, width: 1_200, height: 830),
        scale: 2
    )
    let window = WorldRect(x: 58, y: 86, width: 1_084, height: 706)
    let objectSize = WorldSize(width: 96, height: 104)

    func petFrame(at point: WorldPoint) -> WorldRect {
        WorldRect(
            x: point.x - objectSize.width / 2,
            y: point.y - objectSize.height / 2,
            width: objectSize.width,
            height: objectSize.height
        )
    }

    func destination(
        hint: LocationHint? = nil,
        focus: FocusSnapshot?,
        luminance: LuminanceField? = nil
    ) -> InterestDestination? {
        BasicInterestPositionPlanner.destination(
            for: hint ?? LocationHint(approximateRegion: window, confidence: 0.55),
            in: world(focus: focus, luminance: luminance),
            currentPosition: WorldPoint(x: 200, y: 400),
            pointerPosition: WorldPoint(x: 600, y: 100),
            objectSize: objectSize
        )
    }

    func seat(
        at point: WorldPoint,
        focus: FocusSnapshot? = nil,
        luminance: LuminanceField? = nil
    ) -> SeatEvaluation? {
        BasicInterestPositionPlanner.evaluateSeat(
            at: point,
            for: LocationHint(approximateRegion: window, confidence: 0.55),
            in: world(focus: focus, luminance: luminance),
            currentPosition: point,
            pointerPosition: WorldPoint(x: 600, y: 100),
            objectSize: objectSize
        )
    }

    /// A field over the whole display, filled by a rule keyed on world y.
    func field(_ isBusy: (Double) -> Bool) -> LuminanceField? {
        let columns = 60
        let rows = 45
        let cellHeight = display.frame.size.height / Double(rows)
        var samples: [Double] = []
        for row in 0..<rows {
            let y = (Double(row) + 0.5) * cellHeight
            for column in 0..<columns {
                let busy = isBusy(y) && (column + row).isMultiple(of: 2)
                samples.append(busy ? 0.05 : 0.9)
            }
        }
        return LuminanceField(
            bounds: display.frame,
            columns: columns,
            rows: rows,
            samples: samples
        )
    }

    private func world(focus: FocusSnapshot?, luminance: LuminanceField?) -> DesktopWorldSnapshot {
        DesktopWorldSnapshot(
            displays: [display],
            windows: [WindowSnapshot(id: "w1", frame: window, isFocused: true)],
            focus: focus,
            luminance: luminance
        )
    }
}
