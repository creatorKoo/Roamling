// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Writing the hook into the user's own config, and taking it back out.
//!
//! Ported from `ClaudeCodeHookInstaller` and `CodexHookInstaller`, which have
//! the same shape as each other, so this is one implementation with the agent
//! as a parameter.
//!
//! **It is the user's file.** Everything not ours is read, kept and written
//! back untouched, a backup is taken the first time, and removal takes out only
//! handlers that carry our marker. Nothing here runs without the user asking
//! for it from the menu.

use crate::normalize::{Agent, TOKEN_HEADER};
use serde_json::{json, Map, Value};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    NotInstalled,
    Installed,
    /// Ours is there but not as this version writes it -- an older token, an
    /// older command, or a duplicate. Reinstalling repairs it.
    NeedsRepair,
}

impl Agent {
    fn marker(self) -> &'static str {
        match self {
            Self::ClaudeCode => "roamling-claude-code-hook",
            Self::Codex => "roamling-codex-hook",
        }
    }

    /// Where the agent keeps its hooks, under the user's home.
    pub fn config_path(self) -> Option<PathBuf> {
        let home = std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from)?;
        Some(match self {
            Self::ClaudeCode => home.join(".claude").join("settings.json"),
            Self::Codex => home.join(".codex").join("hooks.json"),
        })
    }

    fn endpoint(self) -> String {
        format!("http://127.0.0.1:{}{}", self.port(), self.path())
    }

    /// Which events get a handler. Claude Code's list is every event it emits;
    /// Codex installs the subset its 0.147.0 registry actually fires.
    fn installed_events(self) -> &'static [&'static str] {
        match self {
            Self::ClaudeCode => &[
                "SessionStart",
                "UserPromptSubmit",
                "PreToolUse",
                "PostToolUse",
                "PostToolUseFailure",
                "PermissionRequest",
                "Notification",
                "Stop",
                "StopFailure",
                "SessionEnd",
            ],
            Self::Codex => &[
                "SessionStart",
                "UserPromptSubmit",
                "PreToolUse",
                "PostToolUse",
                "PermissionRequest",
                "Stop",
                "SessionEnd",
            ],
        }
    }
}

/// Forwards the hook's stdin, unread, to the authenticated loopback receiver.
///
/// Ported from `HookCommand`, whose Windows branches were already written on
/// the macOS side. Windows has shipped the real `curl.exe` in System32 since 10
/// build 1803, so only the path and the quoting differ. Failure is swallowed:
/// a closed companion must never surface a hook error, which a native `http`
/// handler used to do on every session that outlived the app.
pub fn command(agent: Agent, token: &str) -> String {
    format!(
        "curl.exe --silent --connect-timeout 0.15 --max-time 0.3 \
         --request POST --header \"Content-Type: application/json\" \
         --header \"{TOKEN_HEADER}: {token}\" --data-binary @- \
         \"{}\" >NUL 2>&1 # {}",
        agent.endpoint(),
        agent.marker()
    )
}

fn handler(agent: Agent, token: &str) -> Value {
    json!({ "type": "command", "command": command(agent, token), "timeout": 2 })
}

fn is_current(agent: Agent, token: &str, entry: &Value) -> bool {
    entry.get("type").and_then(Value::as_str) == Some("command")
        && entry.get("command").and_then(Value::as_str) == Some(command(agent, token).as_str())
        && entry.get("timeout").and_then(Value::as_i64) == Some(2)
}

/// Also matches the legacy `http` handler, so an install from an older version
/// can be repaired or removed rather than stranded in the user's settings.
fn is_ours(agent: Agent, entry: &Value) -> bool {
    match entry.get("type").and_then(Value::as_str) {
        Some("command") => entry
            .get("command")
            .and_then(Value::as_str)
            .is_some_and(|command| {
                command.contains(agent.marker())
                    || (command.contains(agent.path()) && command.contains(TOKEN_HEADER))
            }),
        Some("http") => entry.get("url").and_then(Value::as_str).is_some_and(|url| {
            url == agent.endpoint()
                || (url.starts_with("http://127.0.0.1:") && url.ends_with(agent.path()))
        }),
        _ => false,
    }
}

fn read_root(path: &std::path::Path) -> Result<Map<String, Value>, String> {
    if !path.exists() {
        return Ok(Map::new());
    }
    let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    if text.trim().is_empty() {
        return Ok(Map::new());
    }
    match serde_json::from_str::<Value>(&text) {
        Ok(Value::Object(map)) => Ok(map),
        Ok(_) => Err("the config is not a JSON object".into()),
        Err(error) => Err(error.to_string()),
    }
}

/// Every handler of ours, across every event, so "is anything installed" and
/// "is all of it current" are separate questions.
fn ours_for(agent: Agent, hooks: &Map<String, Value>, event: &str) -> Vec<Value> {
    hooks
        .get(event)
        .and_then(Value::as_array)
        .map(|groups| {
            groups
                .iter()
                .filter_map(|group| group.get("hooks")?.as_array())
                .flatten()
                .filter(|entry| is_ours(agent, entry))
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

pub fn status(agent: Agent, token: &str) -> Status {
    let Some(path) = agent.config_path() else {
        return Status::NotInstalled;
    };
    let Ok(root) = read_root(&path) else {
        return Status::NeedsRepair;
    };
    let Some(hooks) = root.get("hooks").and_then(Value::as_object) else {
        return Status::NotInstalled;
    };
    let events = agent.installed_events();
    if events.iter().all(|event| ours_for(agent, hooks, event).is_empty()) {
        return Status::NotInstalled;
    }
    let current = events.iter().all(|event| {
        let found = ours_for(agent, hooks, event);
        found.len() == 1 && found.iter().all(|entry| is_current(agent, token, entry))
    });
    if current {
        Status::Installed
    } else {
        Status::NeedsRepair
    }
}

/// Drop every handler of ours, leaving everything else exactly as it was --
/// including groups that hold other people's handlers alongside ours.
fn strip(agent: Agent, root: &mut Map<String, Value>) {
    let Some(Value::Object(hooks)) = root.get_mut("hooks") else {
        return;
    };
    let names: Vec<String> = hooks.keys().cloned().collect();
    for name in names {
        let Some(Value::Array(groups)) = hooks.get_mut(&name) else {
            continue;
        };
        for group in groups.iter_mut() {
            if let Some(Value::Array(entries)) = group.get_mut("hooks") {
                entries.retain(|entry| !is_ours(agent, entry));
            }
        }
        // A group we emptied was ours alone; one that still holds something is
        // somebody else's and stays.
        groups.retain(|group| {
            group
                .get("hooks")
                .and_then(Value::as_array)
                .is_none_or(|entries| !entries.is_empty())
        });
        if groups.is_empty() {
            hooks.remove(&name);
        }
    }
}

fn backup_and_write(path: &std::path::Path, root: &Map<String, Value>) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    // Once, and never overwritten: the point is the file as it was before
    // Roamling ever touched it.
    let backup = path.with_extension("roamling-backup");
    if path.exists() && !backup.exists() {
        std::fs::copy(path, &backup).map_err(|error| error.to_string())?;
    }
    let text = serde_json::to_string_pretty(&Value::Object(root.clone()))
        .map_err(|error| error.to_string())?;
    std::fs::write(path, text + "\n").map_err(|error| error.to_string())
}

pub fn install(agent: Agent, token: &str) -> Result<(), String> {
    let path = agent.config_path().ok_or("no home directory")?;
    let mut root = read_root(&path)?;
    strip(agent, &mut root);

    let mut hooks = root
        .get("hooks")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    for event in agent.installed_events() {
        let mut groups = hooks
            .get(*event)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        groups.push(json!({ "hooks": [handler(agent, token)] }));
        hooks.insert((*event).to_string(), Value::Array(groups));
    }
    root.insert("hooks".into(), Value::Object(hooks));
    backup_and_write(&path, &root)
}

pub fn remove(agent: Agent) -> Result<(), String> {
    let path = agent.config_path().ok_or("no home directory")?;
    if !path.exists() {
        return Ok(());
    }
    let mut root = read_root(&path)?;
    strip(agent, &mut root);
    backup_and_write(&path, &root)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hooks_of(root: &Map<String, Value>) -> &Map<String, Value> {
        root.get("hooks").unwrap().as_object().unwrap()
    }

    fn installed(agent: Agent, token: &str) -> Map<String, Value> {
        let mut root = Map::new();
        let mut hooks = Map::new();
        for event in agent.installed_events() {
            hooks.insert(
                (*event).to_string(),
                json!([{ "hooks": [handler(agent, token)] }]),
            );
        }
        root.insert("hooks".into(), Value::Object(hooks));
        root
    }

    /// The whole promise of the installer: it is the user's file, and what is
    /// not ours survives being written back.
    #[test]
    fn removing_ours_leaves_everything_else_alone() {
        let mut root = installed(Agent::Codex, "secret");
        let hooks = root.get_mut("hooks").unwrap().as_object_mut().unwrap();
        hooks.insert(
            "Stop".into(),
            json!([
                { "hooks": [ { "type": "command", "command": "echo mine" } ] },
                { "hooks": [ handler(Agent::Codex, "secret") ] }
            ]),
        );
        hooks.insert("SomebodyElse".into(), json!([{ "hooks": [ { "type": "command", "command": "theirs" } ] }]));
        root.insert("unrelated".into(), json!({ "kept": true }));

        strip(Agent::Codex, &mut root);

        assert_eq!(root.get("unrelated"), Some(&json!({ "kept": true })));
        let hooks = hooks_of(&root);
        assert!(hooks.contains_key("SomebodyElse"), "another tool's hook was dropped");
        let stop = hooks.get("Stop").unwrap().as_array().unwrap();
        assert_eq!(stop.len(), 1, "the user's own Stop handler should remain alone");
        assert_eq!(
            stop[0]["hooks"][0]["command"].as_str(),
            Some("echo mine")
        );
        // Every event that held only ours is gone rather than left empty.
        assert!(!hooks.contains_key("SessionStart"));
    }

    /// An install written by an older version has to be repairable, which means
    /// recognising it even though its command string differs.
    #[test]
    fn an_older_install_is_seen_as_ours() {
        let legacy = json!({ "type": "http", "url": "http://127.0.0.1:47832/v1/hooks/codex" });
        assert!(is_ours(Agent::Codex, &legacy));
        let stale_token = json!({
            "type": "command",
            "command": command(Agent::Codex, "an-older-token"),
            "timeout": 2
        });
        assert!(is_ours(Agent::Codex, &stale_token));
        assert!(!is_current(Agent::Codex, "the-current-token", &stale_token));
    }

    /// Somebody else's hook must never be mistaken for ours, or removal would
    /// delete it.
    #[test]
    fn another_tools_hook_is_not_ours() {
        let theirs = json!({ "type": "command", "command": "curl.exe http://example.com" });
        assert!(!is_ours(Agent::Codex, &theirs));
        let other_port = json!({ "type": "http", "url": "http://192.168.0.2:47832/v1/hooks/codex" });
        assert!(!is_ours(Agent::Codex, &other_port));
    }

    /// The command carries the token and the marker; without both, an install
    /// cannot be recognised later and the receiver would refuse it.
    #[test]
    fn the_command_carries_the_token_and_the_marker() {
        let built = command(Agent::ClaudeCode, "abc123");
        assert!(built.contains("abc123"));
        assert!(built.contains(TOKEN_HEADER));
        assert!(built.contains("roamling-claude-code-hook"));
        assert!(built.contains("127.0.0.1:47831/v1/hooks/claude-code"));
        // Failure has to be swallowed, or a closed companion surfaces an error
        // on every session.
        assert!(built.contains(">NUL 2>&1"));
    }
}
