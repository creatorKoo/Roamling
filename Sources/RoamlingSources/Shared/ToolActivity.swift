// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation

/// Splits a tool call into "looking at things" and "doing things".
///
/// Petdex draws the two differently -- `review` for a read or a search, `running`
/// for everything else (`hook_runner.zig:236`) -- so a pet drawn to that contract
/// has artwork for both and Roamling should ask for the right one.
///
/// Only the tool's *name* is read, and only to match this list. Prompt text,
/// tool arguments, results and transcripts stay out of the model, which is the
/// line the normalizers document. A name like `Read` says no more about the
/// user's work than the hook event itself already does.
enum ToolActivity {
    private static let inspecting: Set<String> = ["read", "grep", "glob"]

    static func isInspecting(_ toolName: String?) -> Bool {
        guard let toolName else { return false }
        return inspecting.contains(toolName.lowercased())
    }
}
