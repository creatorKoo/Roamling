// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! A hook payload becomes a `CompanionEvent`, and nothing else gets through.
//!
//! Ported from `ClaudeCodeEventNormalizer` and `CodexEventNormalizer`. The
//! privacy line is the same and it is a line in the *code*, not a promise: only
//! the lifecycle identifiers and the tool's name are read. Prompt text,
//! transcript paths, tool input and output, source content and assistant
//! messages are never looked at, so they cannot leak by accident later.

use roamling_core::{CompanionEvent, CompanionEventKind, UserContext};
use serde_json::Value;

/// Splits a tool call into "looking at things" and "doing things".
///
/// Petdex draws the two differently -- `review` for a read or a search,
/// `running` for everything else -- so a pet drawn to that contract has artwork
/// for both. Only the name is read, and only to match this list.
fn is_inspecting(tool: Option<&str>) -> bool {
    matches!(
        tool.map(str::to_ascii_lowercase).as_deref(),
        Some("read" | "grep" | "glob")
    )
}

fn text<'a>(payload: &'a Value, key: &str) -> Option<&'a str> {
    payload.get(key)?.as_str()
}

/// Which agent a payload came from, which decides the event vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Agent {
    ClaudeCode,
    Codex,
}

impl Agent {
    pub fn id(self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude-code",
            Self::Codex => "codex",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::ClaudeCode => "Claude Code",
            Self::Codex => "Codex",
        }
    }

    /// The loopback port its hook posts to. Two ports rather than one path
    /// discriminator, matching the installers on the macOS side.
    pub fn port(self) -> u16 {
        match self {
            Self::ClaudeCode => 47_831,
            Self::Codex => 47_832,
        }
    }

    pub fn path(self) -> &'static str {
        match self {
            Self::ClaudeCode => "/v1/hooks/claude-code",
            Self::Codex => "/v1/hooks/codex",
        }
    }
}

pub const TOKEN_HEADER: &str = "X-Roamling-Token";

/// `None` where the event is real but says nothing the pet should react to --
/// a compaction, or a notification of a kind we do not answer.
pub fn event(agent: Agent, body: &[u8], timestamp: f64) -> Option<CompanionEvent> {
    let payload: Value = serde_json::from_slice(body).ok()?;
    let session = text(&payload, "session_id")?;
    let name = text(&payload, "hook_event_name")?;
    let tool = text(&payload, "tool_name");

    let (kind, intensity) = match agent {
        Agent::ClaudeCode => claude_code(name, tool, text(&payload, "notification_type"))?,
        Agent::Codex => codex(name, tool)?,
    };

    let mut event = CompanionEvent::new(
        // The id only has to be unique enough to tell two events apart; the
        // source id is what the runtime groups by.
        format!("{}:{session}:{name}:{timestamp}", agent.id()),
        format!("{}:{session}", agent.id()),
        timestamp,
        kind,
        intensity,
        None,
    );
    event.context = Some(UserContext::Working);
    Some(event)
}

fn claude_code(
    name: &str,
    tool: Option<&str>,
    notification: Option<&str>,
) -> Option<(CompanionEventKind, f64)> {
    Some(match name {
        "SessionStart" => (CompanionEventKind::ActivityStarted, 0.35),
        "UserPromptSubmit" => (CompanionEventKind::ActivityStarted, 0.55),
        "PreToolUse" => {
            if is_inspecting(tool) {
                (CompanionEventKind::Inspecting, 0.45)
            } else {
                (CompanionEventKind::HighIntensity, 0.72)
            }
        }
        "PostToolUse" => (CompanionEventKind::Positive, 0.08),
        "PostToolUseFailure" => (CompanionEventKind::Setback, 0.65),
        "PermissionRequest" => (CompanionEventKind::AttentionRequired, 0.95),
        "Notification" => match notification {
            Some("permission_prompt" | "idle_prompt") => {
                (CompanionEventKind::AttentionRequired, 0.8)
            }
            _ => return None,
        },
        "Stop" => (CompanionEventKind::Achievement, 0.55),
        "StopFailure" => (CompanionEventKind::Negative, 0.75),
        "SessionEnd" => (CompanionEventKind::ActivityEnded, 0.1),
        _ => return None,
    })
}

fn codex(name: &str, tool: Option<&str>) -> Option<(CompanionEventKind, f64)> {
    Some(match name {
        "SessionStart" => (CompanionEventKind::ActivityStarted, 0.35),
        "UserPromptSubmit" => (CompanionEventKind::ActivityStarted, 0.55),
        "PreToolUse" => {
            if is_inspecting(tool) {
                (CompanionEventKind::Inspecting, 0.45)
            } else {
                (CompanionEventKind::HighIntensity, 0.72)
            }
        }
        "PostToolUse" => (CompanionEventKind::Positive, 0.08),
        "PermissionRequest" => (CompanionEventKind::AttentionRequired, 0.95),
        "Stop" => (CompanionEventKind::Achievement, 0.55),
        "SessionEnd" => (CompanionEventKind::ActivityEnded, 0.1),
        // Petdex shows `running` for a subagent starting: it is work under way,
        // not the opening of a turn.
        "SubagentStart" => (CompanionEventKind::HighIntensity, 0.3),
        "SubagentStop" => (CompanionEventKind::Positive, 0.12),
        // A compaction is bookkeeping. The pet has nothing to say about it.
        "PreCompact" | "PostCompact" => return None,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(json: &str) -> Vec<u8> {
        json.as_bytes().to_vec()
    }

    /// The intensities and kinds are the macOS normalizer's, and a pet tuned
    /// against those numbers reacts differently if they drift.
    #[test]
    fn the_claude_code_vocabulary_matches_the_swift_original() {
        let cases: [(&str, CompanionEventKind, f64); 6] = [
            ("SessionStart", CompanionEventKind::ActivityStarted, 0.35),
            ("UserPromptSubmit", CompanionEventKind::ActivityStarted, 0.55),
            ("PostToolUse", CompanionEventKind::Positive, 0.08),
            ("PostToolUseFailure", CompanionEventKind::Setback, 0.65),
            ("PermissionRequest", CompanionEventKind::AttentionRequired, 0.95),
            ("Stop", CompanionEventKind::Achievement, 0.55),
        ];
        for (name, kind, intensity) in cases {
            let json = format!(r#"{{"session_id":"s","hook_event_name":"{name}"}}"#);
            let produced = event(Agent::ClaudeCode, &body(&json), 1.0)
                .unwrap_or_else(|| panic!("{name} produced nothing"));
            assert_eq!(produced.kind, kind, "{name}");
            assert_eq!(produced.intensity, intensity, "{name}");
            assert_eq!(produced.source_id, "claude-code:s");
        }
    }

    /// Reading is drawn differently from doing, and the split is on the tool
    /// name alone.
    #[test]
    fn a_read_is_inspecting_and_everything_else_is_work() {
        let read = event(
            Agent::Codex,
            &body(r#"{"session_id":"s","hook_event_name":"PreToolUse","tool_name":"Read"}"#),
            1.0,
        )
        .expect("read");
        assert_eq!(read.kind, CompanionEventKind::Inspecting);

        let edit = event(
            Agent::Codex,
            &body(r#"{"session_id":"s","hook_event_name":"PreToolUse","tool_name":"Edit"}"#),
            1.0,
        )
        .expect("edit");
        assert_eq!(edit.kind, CompanionEventKind::HighIntensity);
    }

    /// A notification the pet has no answer for must not become an event, or
    /// every one of them would wake it.
    #[test]
    fn an_unanswered_notification_is_dropped() {
        let json = r#"{"session_id":"s","hook_event_name":"Notification","notification_type":"other"}"#;
        assert!(event(Agent::ClaudeCode, &body(json), 1.0).is_none());
        let compact = r#"{"session_id":"s","hook_event_name":"PreCompact"}"#;
        assert!(event(Agent::Codex, &body(compact), 1.0).is_none());
    }

    /// Malformed input is the normal case for anything listening on a socket.
    #[test]
    fn rubbish_produces_nothing_rather_than_panicking() {
        assert!(event(Agent::Codex, &body("not json"), 1.0).is_none());
        assert!(event(Agent::Codex, &body("{}"), 1.0).is_none());
        assert!(event(Agent::Codex, &body(r#"{"session_id":"s"}"#), 1.0).is_none());
    }
}
