// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation

/// Who answers a Codex approval request: the user, or Codex's own reviewer.
///
/// The hook does not say. Codex fires `PermissionRequest` *before* it routes
/// the request, and under auto-review almost none of them ever reach the user
/// -- so a pet that asks on every one of them spends the whole run asking for
/// an approval nobody is waiting on (openai/codex #23465 and #28833 ask for the
/// missing field; `docs/requests.md` B8 is what it looked like here).
///
/// The answer is in the session's own record. Every turn opens with a
/// `turn_context` line, and that line carries `approvals_reviewer`.
///
/// **This is the one place the module opens a transcript, and it reads one
/// value from it.** Only lines that announce themselves as `turn_context` are
/// parsed at all; of those, the turn id and the reviewer are read and the rest
/// is dropped with the line. Nothing is stored, and no path outside Codex's own
/// home directory is opened -- the path arrives in a request, and a request is
/// not a reason to read an arbitrary file.
///
/// What this cannot know: a reviewer that gives up hands the question to the
/// user, and nothing announces that either. A session under auto-review is
/// therefore never asked about, including the rare time it should be.
///
/// The Rust side is `rust/roamling-agent/src/reviewer.rs`, rule for rule.
public enum CodexApprovalReviewer {
    /// A session that has run for days. Past this the file is not read and the
    /// request is treated as the user's, which is the safe way to be wrong.
    static let largestRecord: UInt64 = 256 * 1024 * 1024

    /// Whether Codex will answer this request itself. `false` whenever that
    /// cannot be established.
    public static func answersItself(transcriptPath: String?, turnID: String?) -> Bool {
        guard let transcriptPath, let home = codexHome() else { return false }
        return answersItself(
            record: URL(fileURLWithPath: transcriptPath),
            turnID: turnID,
            home: home
        )
    }

    /// The same question with the home directory named, which is how the
    /// tests ask it without touching the real one.
    public static func answersItself(record: URL, turnID: String?, home: URL) -> Bool {
        guard let record = inside(record, home: home) else { return false }
        if let turnID, let known = cache.answer(path: record.path, turnID: turnID) {
            return known
        }
        let named = reviewer(of: record, turnID: turnID)
        let answer = named == "auto_review" || named == "guardian_subagent"
        if let turnID {
            cache.remember(path: record.path, turnID: turnID, answer: answer)
        }
        return answer
    }

    /// `CODEX_HOME` when set, as Codex itself resolves it, else `~/.codex`.
    private static func codexHome() -> URL? {
        let environment = ProcessInfo.processInfo.environment
        if let home = environment["CODEX_HOME"], !home.isEmpty {
            return URL(fileURLWithPath: home)
        }
        guard let home = environment["HOME"], !home.isEmpty else { return nil }
        return URL(fileURLWithPath: home).appendingPathComponent(".codex")
    }

    /// The record, resolved, if it is a `.jsonl` under `home`. Resolving first
    /// is what makes `..` and links answer for where they lead.
    private static func inside(_ record: URL, home: URL) -> URL? {
        guard record.pathExtension == "jsonl" else { return nil }
        let record = record.standardizedFileURL.resolvingSymlinksInPath()
        let home = home.standardizedFileURL.resolvingSymlinksInPath()
        guard FileManager.default.fileExists(atPath: record.path) else { return nil }
        let root = home.path.hasSuffix("/") ? home.path : home.path + "/"
        return record.path.hasPrefix(root) ? record : nil
    }

    /// The reviewer of `turnID`, or of the latest turn when the id is unknown
    /// or not found.
    ///
    /// The turn that asks is the newest one, so its opening line is near the
    /// end of a record that may be tens of megabytes of everything said before
    /// it. The end is read first, in widening windows, and the whole file only
    /// if the turn opened that long ago.
    private static func reviewer(of record: URL, turnID: String?) -> String? {
        guard let handle = try? FileHandle(forReadingFrom: record) else { return nil }
        defer { try? handle.close() }
        guard let length = try? handle.seekToEnd(), length <= largestRecord else { return nil }

        for window in [UInt64(1) << 20, UInt64(16) << 20, UInt64.max] {
            let start = length > window ? length - window : 0
            guard (try? handle.seek(toOffset: start)) != nil,
                  let data = try? handle.readToEnd()
            else { return nil }
            var lines = data.split(separator: UInt8(ascii: "\n"), omittingEmptySubsequences: true)
            if start > 0, !lines.isEmpty {
                // The window opens mid-line. That line belongs to the wider one.
                lines.removeFirst()
            }
            let found = latestReviewer(in: lines, turnID: turnID)
            if let exact = found.exact { return exact }
            // A window that holds some other turn's opening has not answered
            // for this one. Only the whole file may fall back to the latest.
            if start == 0 { return found.latest }
            if turnID == nil, let latest = found.latest { return latest }
        }
        return nil
    }

    private static let marker = Data("\"turn_context\"".utf8)

    private static func latestReviewer(
        in lines: [Data.SubSequence],
        turnID: String?
    ) -> (exact: String?, latest: String?) {
        var latest: String?
        for line in lines {
            // Everything that is not a turn opening is skipped unparsed.
            // Messages and tool output are most of the file and none of this
            // type's business.
            guard line.range(of: marker) != nil,
                  let record = try? JSONSerialization.jsonObject(with: Data(line)) as? [String: Any],
                  record["type"] as? String == "turn_context",
                  let context = record["payload"] as? [String: Any],
                  let reviewer = context["approvals_reviewer"] as? String
            else { continue }
            if let turnID, context["turn_id"] as? String == turnID {
                return (reviewer, latest)
            }
            latest = reviewer
        }
        return (nil, latest)
    }

    /// The last answer, so the second approval request of a turn does not read
    /// the file again. One slot: an agent asks in bursts within a turn.
    private static let cache = Cache()

    private final class Cache: @unchecked Sendable {
        private let lock = NSLock()
        private var last: (path: String, turnID: String, answer: Bool)?

        func answer(path: String, turnID: String) -> Bool? {
            lock.lock()
            defer { lock.unlock() }
            guard let last, last.path == path, last.turnID == turnID else { return nil }
            return last.answer
        }

        func remember(path: String, turnID: String, answer: Bool) {
            lock.lock()
            defer { lock.unlock() }
            last = (path, turnID, answer)
        }
    }
}
