// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation

/// A short history of what changed, kept in memory so the pet can be asked why
/// it is doing something.
///
/// The behaviour that matters is deliberately not "what is true now" but "what
/// changed and when". A sampled trace of a pet that is standing still is a wall
/// of identical lines that says nothing, and the one time it mattered the
/// answer was in a gap between lines rather than in the lines. Callers record
/// on every tick and only transitions survive, so a quiet stretch reads as
/// quiet instead of scrolling past.
///
/// It holds coordinates, scores, state names and timings. Nothing derived from
/// the screen beyond an emptiness score belongs here, and neither does anything
/// naming what the user is working on: the capture path takes care never to
/// keep an image, and a diagnostics buffer must not undo that from behind.
public struct DiagnosticsLog: Sendable {
    public struct Entry: Equatable, Sendable {
        public let timestamp: TimeInterval
        public let category: String
        public let message: String

        public init(timestamp: TimeInterval, category: String, message: String) {
            self.timestamp = timestamp
            self.category = category
            self.message = message
        }
    }

    public let capacity: Int
    /// How far back `text` reaches. On a quiet day `capacity` holds two days of
    /// naps, and someone asking why the pet just did something pasted all of it
    /// (`docs/requests.md` R28). The buffer itself keeps what it kept.
    public let window: TimeInterval
    public private(set) var entries: [Entry] = []
    /// The last message seen per category, which is what makes a per-tick
    /// caller produce a transition log rather than a sampled one.
    private var latest: [String: String] = [:]

    public init(capacity: Int = 2_000, window: TimeInterval = 30 * 60) {
        self.capacity = max(1, capacity)
        self.window = window
    }

    /// Records only when `message` differs from this category's previous one.
    @discardableResult
    public mutating func record(
        _ category: String,
        _ message: String,
        at timestamp: TimeInterval
    ) -> Bool {
        guard latest[category] != message else { return false }
        latest[category] = message
        entries.append(Entry(timestamp: timestamp, category: category, message: message))
        if entries.count > capacity {
            entries.removeFirst(entries.count - capacity)
        }
        return true
    }

    /// The last `window` seconds before `now` (or before the last entry), under
    /// what each category last said before them -- without those lines a pet
    /// that has slept for hours reads as an empty log. Elapsed seconds count
    /// from the start of the window, so the lines carried in from before it are
    /// negative. A log younger than the window counts from its first entry, as
    /// it always has: absolute uptime is a large number that says nothing on
    /// its own and the question is always how long something lasted.
    public func text(now: TimeInterval? = nil) -> String {
        guard let newest = entries.last else { return "(no entries)" }
        let cutoff = (now ?? newest.timestamp) - window
        var carried: [Entry] = []
        for entry in entries.reversed() where entry.timestamp < cutoff {
            if !carried.contains(where: { $0.category == entry.category }) {
                carried.append(entry)
            }
        }
        let shown = carried.reversed() + entries.filter { $0.timestamp >= cutoff }
        guard let first = shown.first, let last = shown.last else { return "(no entries)" }
        let origin = carried.isEmpty ? first.timestamp : cutoff
        var lines = shown.map { entry in
            String(
                format: "%8.1f  %-9@ %@",
                entry.timestamp - origin,
                entry.category,
                entry.message
            )
        }
        if let now, now > last.timestamp {
            lines.append(
                String(format: "%8.1f  %-9@ %@", now - origin, "now", "-")
            )
        }
        return lines.joined(separator: "\n")
    }
}
