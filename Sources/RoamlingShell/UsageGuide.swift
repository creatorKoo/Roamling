// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation

public struct UsageGuide: Sendable {
    public static let seenKey = "roamling.guideSeenRevision"
    public let revision: Int
    public let basics: [String]
    public let changes: [Change]
    public struct Change: Sendable {
        public let revision: Int
        public let key: String
    }
    public struct Page: Sendable {
        public let revision: Int
        public let isBasics: Bool
        public let keys: [String]
        public var title: String { localized(isBasics ? "guide.title" : "guide.updates.title") }
        public var body: String {
            let modifier = localized("guide.modifier.mac")
            let sections = keys.map {
                localized($0 + ".title") + "\n" + localizedFormat($0 + ".body", modifier)
            }
            return ([localized("guide.intro")] + sections + [localized("guide.footer")])
                .joined(separator: "\n\n")
        }
    }

    public init?(text: String) {
        var revision = 0
        var basics: [String] = []
        var changes: [Change] = []
        for line in text.split(separator: "\n") {
            let line = line.trimmingCharacters(in: .whitespacesAndNewlines)
            if line.isEmpty || line.hasPrefix("#") { continue }
            let pair = line.split(separator: "|", omittingEmptySubsequences: false)
            guard pair.count == 2, !pair[1].isEmpty else { return nil }
            if pair[0] == "revision" { revision = Int(pair[1]) ?? 0 }
            else if pair[0] == "basic" { basics.append(String(pair[1])) }
            else if let number = Int(pair[0]), number > 0 {
                changes.append(Change(revision: number, key: String(pair[1])))
            } else { return nil }
        }
        guard revision > 0, !basics.isEmpty,
              changes.allSatisfy({ $0.revision <= revision }) else { return nil }
        self.revision = revision
        self.basics = basics
        self.changes = changes
    }

    public static func bundled() -> UsageGuide? {
        guard let url = shellResourceBundle.url(forResource: "UsageGuide", withExtension: "txt"),
              let text = try? String(contentsOf: url, encoding: .utf8) else { return nil }
        return UsageGuide(text: text)
    }

    public func page(seen: Int, manual: Bool = false) -> Page? {
        if manual || seen <= 0 { return Page(revision: revision, isBasics: true, keys: basics) }
        guard seen < revision else { return nil }
        let keys = changes.filter { $0.revision > seen }.map(\.key)
        return keys.isEmpty ? nil : Page(revision: revision, isBasics: false, keys: keys)
    }
}
