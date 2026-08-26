// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation
import RoamlingCore

/// Events implemented by the installed Codex 0.147.0 hook registry.
public enum CodexHookEvent: String, CaseIterable, Sendable {
    case preToolUse = "PreToolUse"
    case permissionRequest = "PermissionRequest"
    case postToolUse = "PostToolUse"
    case preCompact = "PreCompact"
    case postCompact = "PostCompact"
    case sessionStart = "SessionStart"
    case sessionEnd = "SessionEnd"
    case userPromptSubmit = "UserPromptSubmit"
    case subagentStart = "SubagentStart"
    case subagentStop = "SubagentStop"
    case stop = "Stop"
}

public struct CodexHookPayload: Decodable, Sendable {
    public let sessionID: String
    public let turnID: String?
    public let event: CodexHookEvent

    private enum CodingKeys: String, CodingKey {
        case sessionID = "session_id"
        case turnID = "turn_id"
        case hookEventName = "hook_event_name"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        sessionID = try container.decode(String.self, forKey: .sessionID)
        turnID = try container.decodeIfPresent(String.self, forKey: .turnID)
        let name = try container.decode(String.self, forKey: .hookEventName)
        guard let event = CodexHookEvent(rawValue: name) else {
            throw DecodingError.dataCorruptedError(
                forKey: .hookEventName,
                in: container,
                debugDescription: "Unsupported Codex hook event"
            )
        }
        self.event = event
    }
}

/// Reads only lifecycle identifiers. Prompt text, transcript paths, tool
/// input/output, source code, and assistant messages are intentionally absent.
public enum CodexEventNormalizer {
    public static func event(
        from data: Data,
        timestamp: TimeInterval
    ) throws -> CompanionEvent? {
        let payload = try JSONDecoder().decode(CodexHookPayload.self, from: data)
        guard let mapping = mapping(for: payload.event) else { return nil }
        return CompanionEvent(
            sourceID: "codex:\(payload.sessionID)",
            sourceType: .agent,
            timestamp: timestamp,
            kind: mapping.kind,
            intensity: mapping.intensity,
            context: .working
        )
    }

    private static func mapping(
        for event: CodexHookEvent
    ) -> (kind: CompanionEventKind, intensity: Double)? {
        switch event {
        case .sessionStart:
            (.activityStarted, 0.35)
        case .userPromptSubmit:
            (.activityStarted, 0.55)
        case .preToolUse:
            (.activityStarted, 0.72)
        case .postToolUse:
            (.positive, 0.08)
        case .permissionRequest:
            (.attentionRequired, 0.95)
        case .stop:
            (.achievement, 0.55)
        case .sessionEnd:
            (.activityEnded, 0.1)
        case .subagentStart:
            (.activityStarted, 0.3)
        case .subagentStop:
            (.positive, 0.12)
        case .preCompact, .postCompact:
            nil
        }
    }
}
