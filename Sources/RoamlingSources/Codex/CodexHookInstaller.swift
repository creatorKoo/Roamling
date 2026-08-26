// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation

public enum CodexIntegrationStatus: String, Sendable {
    case notInstalled
    case needsRepair
    case installed
}

public struct CodexHookInstaller: Sendable {
    public static let defaultPort: UInt16 = 47_832
    public static let path = "/v1/hooks/codex"
    public static let tokenHeader = "X-Roamling-Token"
    public static let marker = "roamling-codex-hook"

    /// Compact/child-agent events are intentionally omitted for MVP 2. They do
    /// not add a distinct companion state, and fewer synchronous hooks means
    /// less overhead in Codex.
    public static let installedEvents: [CodexHookEvent] = [
        .sessionStart,
        .userPromptSubmit,
        .preToolUse,
        .postToolUse,
        .permissionRequest,
        .stop,
        .sessionEnd
    ]

    public let hooksURL: URL
    public let endpoint: URL
    public let token: String

    public init(
        hooksURL: URL,
        token: String,
        port: UInt16 = Self.defaultPort
    ) {
        self.hooksURL = hooksURL
        endpoint = URL(string: "http://127.0.0.1:\(port)\(Self.path)")!
        self.token = token
    }

    public var backupURL: URL {
        hooksURL.appendingPathExtension("roamling-backup")
    }

    public func status() -> CodexIntegrationStatus {
        guard let root = try? readRoot() else { return .needsRepair }
        guard let hooks = root["hooks"] as? [String: Any] else { return .notInstalled }
        let matching = matchingHandlers(in: hooks)
        if matching.isEmpty { return .notInstalled }
        let everyEventIsCurrent = Self.installedEvents.allSatisfy { event in
            let handlers = matchingHandlers(for: event, in: hooks)
            return handlers.count == 1 && handlers.allSatisfy(handlerIsCurrent)
        }
        return everyEventIsCurrent ? .installed : .needsRepair
    }

    public func install() throws {
        var root = try readRoot()
        try createBackupIfNeeded()
        removeHandlers(from: &root)

        var hooks = root["hooks"] as? [String: Any] ?? [:]
        for event in Self.installedEvents {
            var groups = hooks[event.rawValue] as? [[String: Any]] ?? []
            groups.append(["hooks": [currentHandler()]])
            hooks[event.rawValue] = groups
        }
        root["hooks"] = hooks
        try write(root)
    }

    public func remove() throws {
        guard FileManager.default.fileExists(atPath: hooksURL.path) else { return }
        var root = try readRoot()
        try createBackupIfNeeded()
        removeHandlers(from: &root)
        try write(root)
    }

    /// Codex command hooks receive JSON on stdin. curl forwards that byte
    /// stream only to Roamling's authenticated loopback receiver. Failure is
    /// swallowed so an unavailable companion can never interrupt agent work.
    public var command: String {
        "/usr/bin/curl --silent --connect-timeout 0.15 --max-time 0.3 "
            + "--request POST --header 'Content-Type: application/json' "
            + "--header '\(Self.tokenHeader): \(token)' --data-binary @- "
            + "'\(endpoint.absoluteString)' >/dev/null 2>&1 || true # \(Self.marker)"
    }

    private func readRoot() throws -> [String: Any] {
        guard FileManager.default.fileExists(atPath: hooksURL.path) else { return [:] }
        let data = try Data(contentsOf: hooksURL)
        guard !data.isEmpty else { return [:] }
        let object = try JSONSerialization.jsonObject(with: data)
        guard let root = object as? [String: Any] else {
            throw CocoaError(.propertyListReadCorrupt)
        }
        return root
    }

    private func currentHandler() -> [String: Any] {
        [
            "type": "command",
            "command": command,
            "timeout": 2,
            "statusMessage": "Notifying Roamling"
        ]
    }

    private func handlerIsCurrent(_ handler: [String: Any]) -> Bool {
        guard isRoamlingHandler(handler), handler["command"] as? String == command else {
            return false
        }
        return (handler["timeout"] as? NSNumber)?.intValue == 2
            && handler["statusMessage"] as? String == "Notifying Roamling"
    }

    private func isRoamlingHandler(_ handler: [String: Any]) -> Bool {
        guard handler["type"] as? String == "command",
              let command = handler["command"] as? String else { return false }
        return command.contains(Self.marker)
            || (command.contains(Self.path) && command.contains(Self.tokenHeader))
    }

    private func matchingHandlers(in hooks: [String: Any]) -> [[String: Any]] {
        Self.installedEvents.flatMap { matchingHandlers(for: $0, in: hooks) }
    }

    private func matchingHandlers(
        for event: CodexHookEvent,
        in hooks: [String: Any]
    ) -> [[String: Any]] {
        guard let groups = hooks[event.rawValue] as? [[String: Any]] else { return [] }
        return groups.flatMap { group in
            (group["hooks"] as? [[String: Any]] ?? []).filter(isRoamlingHandler)
        }
    }

    private func removeHandlers(from root: inout [String: Any]) {
        guard var hooks = root["hooks"] as? [String: Any] else { return }
        for key in Array(hooks.keys) {
            guard let groups = hooks[key] as? [[String: Any]] else { continue }
            let repaired = groups.compactMap { group -> [String: Any]? in
                guard let handlers = group["hooks"] as? [[String: Any]] else { return group }
                let kept = handlers.filter { !isRoamlingHandler($0) }
                guard !kept.isEmpty else { return nil }
                var result = group
                result["hooks"] = kept
                return result
            }
            if repaired.isEmpty {
                hooks.removeValue(forKey: key)
            } else {
                hooks[key] = repaired
            }
        }
        if hooks.isEmpty {
            root.removeValue(forKey: "hooks")
        } else {
            root["hooks"] = hooks
        }
    }

    private func createBackupIfNeeded() throws {
        let manager = FileManager.default
        guard manager.fileExists(atPath: hooksURL.path),
              !manager.fileExists(atPath: backupURL.path) else { return }
        try manager.copyItem(at: hooksURL, to: backupURL)
    }

    private func write(_ root: [String: Any]) throws {
        let manager = FileManager.default
        try manager.createDirectory(
            at: hooksURL.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        let permissions = (try? manager.attributesOfItem(atPath: hooksURL.path)[.posixPermissions])
        var data = try JSONSerialization.data(
            withJSONObject: root,
            options: [.prettyPrinted, .sortedKeys, .withoutEscapingSlashes]
        )
        data.append(0x0A)
        try data.write(to: hooksURL, options: .atomic)
        if let permissions {
            try manager.setAttributes(
                [.posixPermissions: permissions],
                ofItemAtPath: hooksURL.path
            )
        }
    }
}
