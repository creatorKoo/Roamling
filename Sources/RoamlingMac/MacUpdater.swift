// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import AppKit
import Foundation
import RoamlingCoreRs
import RoamlingShell

/// Getting the new version onto the disk, and into the running pet.
///
/// The decisions -- is it newer, is it ours -- are `roamling-update`'s, and
/// Windows shares them. What is here is what touches this machine: fetching
/// bytes, unpacking them beside the app, replacing the bundle, and starting it
/// again.
///
/// ## Replacing a running app -- and then restarting it, at once
///
/// A running process holds the executable's inode rather than its path, so the
/// bundle underneath it can be moved aside and a new one put in its place, and
/// the process keeps running. It does not keep *seeing*: from that moment
/// ScreenCaptureKit stops answering it, and the pet sits on text until it is
/// started again (`docs/capture.md` section 2, 2026-09-23). So a check only
/// unpacks (`stage`), and the swap (`install`) happens only right before the
/// restart (`relaunch`) -- or at quit, when there is no process left to go
/// blind. `docs/windows.md` "자동 업데이트" has the whole flow.
///
/// ## Never annoying
///
/// Nothing here shows a window on its own. A background check that finds
/// nothing says nothing, and one that finds something restarts the pet when
/// it is idle. Only a check the user asked for reports back.
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
        /// Downloaded, verified and unpacked beside the app. `install` puts it
        /// in place.
        case staged(version: String)
        case failed(String)
    }

    /// The version unpacked and waiting for `install`, if a check has found
    /// one this session. The menu shows it instead of offering another check.
    public private(set) var staged: String?
    public private(set) var isChecking = false

    private let session: URLSession

    public init(session: URLSession = .shared) {
        self.session = session
    }

    /// Always reports, whatever the answer.
    ///
    /// It used to stay quiet when a background check found nothing, which
    /// conflated two different things: not showing an alert, and not saying
    /// what happened. The caller had already put the menu into its "checking"
    /// state and was relying on this to take it back out, so a silent answer
    /// left that row saying "Checking for updates…" for the rest of the
    /// session -- and since that row replaces the button, there was nothing
    /// left to press. Whether to *interrupt* the user is the caller's decision
    /// and is made against `Outcome`, not here.
    public func check(then report: @escaping @MainActor (Outcome) -> Void) {
        guard !isChecking else { return }
        isChecking = true
        Task { [weak self] in
            // Deliberately off the main actor. Unpacking runs `ditto` and
            // `codesign` and waits for them, and waiting for a process on the
            // thread that draws the pet would stop the pet.
            let outcome = await Self.run()
            await MainActor.run {
                guard let self else { return }
                self.isChecking = false
                if case let .staged(version) = outcome { self.staged = version }
                report(outcome)
            }
        }
    }

    private nonisolated static func run() async -> Outcome {
        let current = updateCurrentVersion()
        // `swift run` has no bundle, and `bundleURL` is then the build
        // directory. Swapping that for an app would be a disaster.
        guard Bundle.main.bundleURL.pathExtension == "app" else {
            return .failed("not running from an app bundle")
        }
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

    private nonisolated static func fetch(_ url: URL, limit: Int) async throws -> Data {
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

    /// Beside the app, so the swap is a rename on one volume.
    private nonisolated static var work: URL {
        Bundle.main.bundleURL.deletingLastPathComponent()
            .appendingPathComponent(".Roamling-update", isDirectory: true)
    }

    private nonisolated static var unpacked: URL {
        work.appendingPathComponent("Roamling.app", isDirectory: true)
    }

    /// Unpacks a verified archive beside the running app. Nothing the running
    /// process uses is touched.
    private nonisolated static func stage(_ archive: Data) throws {
        let manager = FileManager.default
        try? manager.removeItem(at: work)
        try manager.createDirectory(at: work, withIntermediateDirectories: true)
        do {
            let archiveURL = work.appendingPathComponent("Roamling.zip")
            try archive.write(to: archiveURL)

            // `ditto` rather than an unzip library: it is what wrote the archive,
            // and it keeps the extended attributes and internal symlinks a signed
            // bundle needs. Anything that drops them produces an app macOS refuses.
            try run("/usr/bin/ditto", ["-x", "-k", archiveURL.path, work.path])
            try? manager.removeItem(at: archiveURL)

            guard manager.fileExists(atPath: unpacked.path) else {
                throw Failure("the archive did not contain Roamling.app")
            }
            // Signed by us, and intact. The Ed25519 signature already said these
            // are our bytes; this says macOS will agree to run them.
            try run("/usr/bin/codesign", ["--verify", "--deep", "--strict", unpacked.path])
        } catch {
            try? manager.removeItem(at: work)
            throw error
        }
    }

    /// Puts the staged bundle in place of the running one. Call it only on the
    /// way out -- right before `relaunch`, or at quit -- because the process
    /// that stays behind can no longer capture the screen.
    public func install() throws {
        guard staged != nil else { return }
        let manager = FileManager.default
        let bundle = Bundle.main.bundleURL
        defer { try? manager.removeItem(at: Self.work) }
        staged = nil

        guard manager.fileExists(atPath: Self.unpacked.path) else {
            throw Failure("the staged update is gone")
        }
        let outgoing = Self.work.appendingPathComponent("outgoing.app", isDirectory: true)
        try manager.moveItem(at: bundle, to: outgoing)
        do {
            try manager.moveItem(at: Self.unpacked, to: bundle)
        } catch {
            // Put it back rather than leave the machine with no app at all.
            try? manager.moveItem(at: outgoing, to: bundle)
            throw error
        }
    }

    nonisolated static let afterFlag = "--after"

    /// Opens the app again as a new instance and hands it this process's id,
    /// so it waits for this one to be gone (`waitForPreviousInstance`). A plain
    /// `open` would not do: with the bundle id already running, Launch
    /// Services brings the running copy forward instead of starting the new one.
    ///
    /// `finish` gets nil once Launch Services has the new copy, which is when
    /// this one may quit. Quitting earlier could lose the request.
    public func relaunch(then finish: @escaping @Sendable @MainActor (Error?) -> Void) {
        let configuration = NSWorkspace.OpenConfiguration()
        configuration.createsNewApplicationInstance = true
        configuration.activates = false
        configuration.arguments = [Self.afterFlag, String(ProcessInfo.processInfo.processIdentifier)]
        NSWorkspace.shared.openApplication(
            at: Bundle.main.bundleURL,
            configuration: configuration
        ) { _, error in
            Task { @MainActor in finish(error) }
        }
    }

    /// The other half of `relaunch`, run before this copy builds anything the
    /// old one still holds -- the agent ports, the overlay, the saved position.
    /// Bounded: a copy that will not quit must not keep the pet away forever.
    public nonisolated static func waitForPreviousInstance(
        arguments: [String] = CommandLine.arguments,
        timeout: TimeInterval = 10
    ) {
        guard let flag = arguments.firstIndex(of: afterFlag),
              arguments.indices.contains(flag + 1),
              let pid = pid_t(arguments[flag + 1]) else { return }
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline, kill(pid, 0) == 0 || errno == EPERM {
            usleep(50_000)
        }
    }

    /// A version unpacked by a copy that never got to put it in place -- it was
    /// killed, or the machine went down. The next check fetches it again.
    public nonisolated static func discardLeftovers() {
        try? FileManager.default.removeItem(at: work)
    }

    private nonisolated static func run(_ path: String, _ arguments: [String]) throws {
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
