// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation
import RoamlingCore

public enum ClaudeCodeHookEvent: String, CaseIterable, Sendable {
    case sessionStart = "SessionStart"
    case userPromptSubmit = "UserPromptSubmit"
    case preToolUse = "PreToolUse"
    case postToolUse = "PostToolUse"
    case postToolUseFailure = "PostToolUseFailure"
    case permissionRequest = "PermissionRequest"
    case notification = "Notification"
    case stop = "Stop"
    case stopFailure = "StopFailure"
    case sessionEnd = "SessionEnd"
}

public struct ClaudeCodeHookPayload: Decodable, Sendable {
    public let sessionID: String
    public let promptID: String?
    public let event: ClaudeCodeHookEvent
    public let notificationType: String?
    /// The tool's name only, matched against a fixed list. See `ToolActivity`.
    public let toolName: String?

    private enum CodingKeys: String, CodingKey {
        case sessionID = "session_id"
        case promptID = "prompt_id"
        case hookEventName = "hook_event_name"
        case notificationType = "notification_type"
        case toolName = "tool_name"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        sessionID = try container.decode(String.self, forKey: .sessionID)
        promptID = try container.decodeIfPresent(String.self, forKey: .promptID)
        let name = try container.decode(String.self, forKey: .hookEventName)
        guard let event = ClaudeCodeHookEvent(rawValue: name) else {
            throw DecodingError.dataCorruptedError(
                forKey: .hookEventName,
                in: container,
                debugDescription: "Unsupported Claude Code hook event"
            )
        }
        self.event = event
        notificationType = try container.decodeIfPresent(String.self, forKey: .notificationType)
        toolName = try container.decodeIfPresent(String.self, forKey: .toolName)
    }
}

/// Reads only lifecycle identifiers and the tool's name. Event-specific prompt,
/// transcript, tool input/output, and source content are intentionally absent
/// from the model. The name is matched against `ToolActivity`'s fixed list so the
/// pet can tell reading from doing, which is the split Petdex draws.
public enum ClaudeCodeEventNormalizer {
    public static func event(
        from data: Data,
        timestamp: TimeInterval
    ) throws -> CompanionEvent? {
        let payload = try JSONDecoder().decode(ClaudeCodeHookPayload.self, from: data)
        guard let mapping = mapping(for: payload) else { return nil }
        return CompanionEvent(
            sourceID: "claude-code:\(payload.sessionID)",
            sourceType: .agent,
            timestamp: timestamp,
            kind: mapping.kind,
            intensity: mapping.intensity,
            context: .working
        )
    }

    private static func mapping(
        for payload: ClaudeCodeHookPayload
    ) -> (kind: CompanionEventKind, intensity: Double)? {
        switch payload.event {
        case .sessionStart:
            (.activityStarted, 0.35)
        case .userPromptSubmit:
            (.activityStarted, 0.55)
        case .preToolUse:
            ToolActivity.isInspecting(payload.toolName)
                ? (.inspecting, 0.45)
                : (.highIntensity, 0.72)
        case .postToolUse:
            (.positive, 0.08)
        case .postToolUseFailure:
            (.setback, 0.65)
        case .permissionRequest:
            (.attentionRequired, 0.95)
        case .notification:
            switch payload.notificationType {
            case "permission_prompt", "idle_prompt":
                (.attentionRequired, 0.8)
            default:
                nil
            }
        case .stop:
            (.achievement, 0.55)
        case .stopFailure:
            (.negative, 0.75)
        case .sessionEnd:
            (.activityEnded, 0.1)
        }
    }
}
