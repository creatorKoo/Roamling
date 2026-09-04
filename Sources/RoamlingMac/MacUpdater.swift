// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation
import RoamlingCoreRs
import RoamlingShell

/// Getting the new version onto the disk.
///
/// The decisions -- is it newer, is it ours -- are `roamling-update`'s, and
/// Windows shares them. What is here is the three things that touch this
/// machine: fetching bytes, unpacking them, and replacing a bundle that is
/// currently running.
///
/// ## Replacing a running app
///
/// macOS lets it happen, unlike Windows. A running process holds the
/// executable's inode rather than its path, so the bundle underneath it can be
/// moved aside and a new one put in its place; the process keeps running from
/// the copy it already opened, and the next launch gets the new version. The
/// old bundle is deleted immediately for the same reason -- deleting a file
/// someone has open is only a rename to the kernel.
///
/// ## Never annoying
///
/// Nothing here shows a window on its own. A background check that finds
/// nothing says nothing. Only a check the user asked for reports back.
@MainActor
public final class MacUpdater {
    /// `/releases/latest/download/` always redirects to the newest release's
    /// asset, so the feed needs no site of its own -- the same CI step that
    /// uploads the build uploads this.
    private static let feed = URL(
        string: "https://github.com/creatorKoo/Roamling/releases/latest/download/appcast.json"
    )!
    private static let feedSignature = URL(
        string: "https://github.com/creatorKoo/Roamling/releases/latest/download/appcast.json.sig"
    )!

    /// A manifest bigger than this is not our manifest, and an artifact bigger
    /// than this is not our 8 MB app. Both are read into memory to be verified
    /// before anything is written, so both need a ceiling.
    private static let maximumFeedBytes = 64 * 1_024
    private static let maximumArtifactBytes = 128 * 1_024 * 1_024

    public enum Outcome: Equatable, Sendable {
        case upToDate(current: String)
        /// Downloaded, verified and swapped in. Takes effect on the next launch.
        case staged(version: String)
        case failed(String)
    }

    /// The version already staged, if a check has found one this session. The
    /// menu shows it instead of offering another check.
    public private(set) var staged: String?
    public private(set) var isChecking = false

    private let session: URLSession

    public init(session: URLSession = .shared) {
        self.session = session
    }

    /// - Parameter asked: whether the user asked. A background check that finds
    ///   nothing is silent; one the user asked for always answers.
    public func check(asked: Bool, then report: @escaping @MainActor (Outcome) -> Void) {
        guard !isChecking else { return }
        isChecking = true
        Task { [weak self] in
            let outcome = await Self.run()
            guard let self else { return }
            self.isChecking = false
            if case let .staged(version) = outcome { self.staged = version }
            switch outcome {
            case .upToDate where !asked:
                // Silence is the whole point of a background check.
                break
            case .failed where !asked:
                break
            default:
                report(outcome)
            }
        }
    }

    private static func run() async -> Outcome {
        let current = updateCurrentVersion()
        do {
            let manifest = try await fetch(feed, limit: maximumFeedBytes)
            let signature = try await fetch(feedSignature, limit: maximumFeedBytes)
            guard let signatureText = String(data: signature, encoding: .utf8) else {
                return .failed("the signature is not text")
            }

            let answer = updateCheck(manifest: manifest, signature: signatureText)
            if let error = answer.error { return .failed(error) }
            guard let update = answer.update else { return .upToDate(current: current) }
            guard let url = URL(string: update.url) else {
                return .failed("the feed names a url that is not one")
            }

            let bytes = try await fetch(url, limit: maximumArtifactBytes)
            // Verified in memory. Nothing unverified is ever written next to
            // the app, let alone put in its place.
            if let error = updateVerify(
                bytes: bytes, size: update.size, signature: update.signature
            ) {
                return .failed(error)
            }
            try stage(bytes)
            return .staged(version: update.version)
        } catch {
            return .failed(error.localizedDescription)
        }
    }

    private static func fetch(_ url: URL, limit: Int) async throws -> Data {
        var request = URLRequest(url: url)
        request.timeoutInterval = 30
        let (data, response) = try await URLSession.shared.data(for: request)
        if let http = response as? HTTPURLResponse, !(200..<300).contains(http.statusCode) {
            throw Failure("\(url.lastPathComponent): HTTP \(http.statusCode)")
        }
        guard data.count <= limit else {
            throw Failure("\(url.lastPathComponent) is larger than it could honestly be")
        }
        return data
    }

    /// Puts a verified archive in place of the running app.
    private static func stage(_ archive: Data) throws {
        let bundle = Bundle.main.bundleURL
        let parent = bundle.deletingLastPathComponent()
        let work = parent.appendingPathComponent(".Roamling-update", isDirectory: true)
        let manager = FileManager.default

        try? manager.removeItem(at: work)
        try manager.createDirectory(at: work, withIntermediateDirectories: true)
        defer { try? manager.removeItem(at: work) }

        let archiveURL = work.appendingPathComponent("Roamling.zip")
        try archive.write(to: archiveURL)

        // `ditto` rather than an unzip library: it is what wrote the archive,
        // and it keeps the extended attributes and internal symlinks a signed
        // bundle needs. Anything that drops them produces an app macOS refuses.
        try run("/usr/bin/ditto", ["-x", "-k", archiveURL.path, work.path])

        let unpacked = work.appendingPathComponent("Roamling.app", isDirectory: true)
        guard manager.fileExists(atPath: unpacked.path) else {
            throw Failure("the archive did not contain Roamling.app")
        }
        // Signed by us, and intact. The Ed25519 signature already said these
        // are our bytes; this says macOS will agree to run them.
        try run("/usr/bin/codesign", ["--verify", "--deep", "--strict", unpacked.path])

        // The swap. A running process holds the old executable's inode, so
        // moving the bundle out from under it is safe and it keeps running.
        let outgoing = work.appendingPathComponent("outgoing.app", isDirectory: true)
        try manager.moveItem(at: bundle, to: outgoing)
        do {
            try manager.moveItem(at: unpacked, to: bundle)
        } catch {
            // Put it back rather than leave the machine with no app at all.
            try? manager.moveItem(at: outgoing, to: bundle)
            throw error
        }
    }

    private static func run(_ path: String, _ arguments: [String]) throws {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: path)
        process.arguments = arguments
        let errors = Pipe()
        process.standardError = errors
        process.standardOutput = Pipe()
        try process.run()
        let detail = errors.fileHandleForReading.readDataToEndOfFile()
        process.waitUntilExit()
        guard process.terminationStatus == 0 else {
            let message = String(data: detail, encoding: .utf8)?
                .trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
            throw Failure(
                message.isEmpty
                    ? "\((path as NSString).lastPathComponent) failed"
                    : message
            )
        }
    }

    private struct Failure: Error, LocalizedError {
        let message: String
        init(_ message: String) { self.message = message }
        var errorDescription: String? { message }
    }
}
