// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation

public enum ClaudeCodeIntegrationStatus: String, Sendable {
    case notInstalled
    case needsRepair
    case installed
}

public struct ClaudeCodeHookInstaller: Sendable {
    public static let defaultPort: UInt16 = 47_831
    public static let path = "/v1/hooks/claude-code"
    public static let tokenHeader = "X-Roamling-Token"
    public static let marker = "roamling-claude-code-hook"

    public let settingsURL: URL
    public let endpoint: URL
    public let token: String

    public init(
        settingsURL: URL,
        token: String,
        port: UInt16 = Self.defaultPort
    ) {
        self.settingsURL = settingsURL
        endpoint = URL(string: "http://127.0.0.1:\(port)\(Self.path)")!
        self.token = token
    }

    public var backupURL: URL {
        settingsURL.appendingPathExtension("roamling-backup")
    }

    public func status() -> ClaudeCodeIntegrationStatus {
        guard let root = try? readRoot() else { return .needsRepair }
        guard let hooks = root["hooks"] as? [String: Any] else { return .notInstalled }
        let matching = matchingHandlers(in: hooks)
        if matching.isEmpty { return .notInstalled }
        let everyEventIsCurrent = ClaudeCodeHookEvent.allCases.allSatisfy { event in
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
        for event in ClaudeCodeHookEvent.allCases {
            var groups = hooks[event.rawValue] as? [[String: Any]] ?? []
            groups.append(["hooks": [currentHandler()]])
            hooks[event.rawValue] = groups
        }
        root["hooks"] = hooks
        try write(root)
    }

    public func remove() throws {
        guard FileManager.default.fileExists(atPath: settingsURL.path) else { return }
        var root = try readRoot()
        try createBackupIfNeeded()
        removeHandlers(from: &root)
        try write(root)
    }

    private func readRoot() throws -> [String: Any] {
        guard FileManager.default.fileExists(atPath: settingsURL.path) else { return [:] }
        let data = try Data(contentsOf: settingsURL)
        guard !data.isEmpty else { return [:] }
        let object = try JSONSerialization.jsonObject(with: data)
        guard let root = object as? [String: Any] else {
            throw CocoaError(.propertyListReadCorrupt)
        }
        return root
    }

    /// Claude Code command hooks receive JSON on stdin. curl forwards that byte
    /// stream only to Roamling's authenticated loopback receiver. Failure is
    /// swallowed so a closed companion can never surface a hook error, which a
    /// native `http` handler did on every session that outlived the app.
    public var command: String {
        HookCommand.forwardStandardInput(
            to: endpoint,
            tokenHeader: Self.tokenHeader,
            token: token,
            marker: Self.marker
        )
    }

    private func currentHandler() -> [String: Any] {
        [
            "type": "command",
            "command": command,
            "timeout": 2
        ]
    }

    private func handlerIsCurrent(_ handler: [String: Any]) -> Bool {
        guard handler["type"] as? String == "command",
              handler["command"] as? String == command else { return false }
        return (handler["timeout"] as? NSNumber)?.intValue == 2
    }

    /// Also matches the legacy `http` handler so an existing install can be
    /// repaired or removed instead of being stranded in the user's settings.
    private func isRoamlingHandler(_ handler: [String: Any]) -> Bool {
        switch handler["type"] as? String {
        case "command":
            guard let command = handler["command"] as? String else { return false }
            return command.contains(Self.marker)
                || (command.contains(Self.path) && command.contains(Self.tokenHeader))
        case "http":
            guard let url = handler["url"] as? String else { return false }
            return url == endpoint.absoluteString
                || (url.hasPrefix("http://127.0.0.1:") && url.hasSuffix(Self.path))
        default:
            return false
        }
    }

    private func matchingHandlers(in hooks: [String: Any]) -> [[String: Any]] {
        ClaudeCodeHookEvent.allCases.flatMap { matchingHandlers(for: $0, in: hooks) }
    }

    private func matchingHandlers(
        for event: ClaudeCodeHookEvent,
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
        guard manager.fileExists(atPath: settingsURL.path),
              !manager.fileExists(atPath: backupURL.path) else { return }
        try manager.copyItem(at: settingsURL, to: backupURL)
    }

    private func write(_ root: [String: Any]) throws {
        let manager = FileManager.default
        try manager.createDirectory(
            at: settingsURL.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        let permissions = (try? manager.attributesOfItem(atPath: settingsURL.path)[.posixPermissions])
        var data = try JSONSerialization.data(
            withJSONObject: root,
            options: [.prettyPrinted, .sortedKeys, .withoutEscapingSlashes]
        )
        data.append(0x0A)
        try data.write(to: settingsURL, options: .atomic)
        if let permissions {
            try manager.setAttributes(
                [.posixPermissions: permissions],
                ofItemAtPath: settingsURL.path
            )
        }
    }
}
