// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation

public struct PetDescriptor: Hashable, Sendable, Identifiable {
    public let id: String
    public let displayName: String
    public let packageURL: URL
    public let spriteVersionNumber: Int

    public init(id: String, displayName: String, packageURL: URL, spriteVersionNumber: Int) {
        self.id = id
        self.displayName = displayName
        self.packageURL = packageURL
        self.spriteVersionNumber = spriteVersionNumber
    }
}

public struct PetCatalog {
    public let roots: [URL]

    public init(roots: [URL] = Self.defaultRoots()) {
        self.roots = roots
    }

    public static func defaultRoots(
        environment: [String: String] = ProcessInfo.processInfo.environment,
        home: URL = FileManager.default.homeDirectoryForCurrentUser
    ) -> [URL] {
        var roots: [URL] = []
        if let override = environment["ROAMLING_PET_PATH"], !override.isEmpty {
            roots.append(URL(fileURLWithPath: override, isDirectory: true))
        }
        roots.append(home.appendingPathComponent("Library/Application Support/Roamling/Pets", isDirectory: true))
        roots.append(home.appendingPathComponent(".codex/pets", isDirectory: true))
        roots.append(home.appendingPathComponent(".petdex/pets", isDirectory: true))
        return roots
    }

    public func discover() -> [PetDescriptor] {
        var seenPaths = Set<String>()
        var seenIDs = Set<String>()
        var result: [PetDescriptor] = []

        for root in roots {
            let packages: [URL]
            if FileManager.default.fileExists(atPath: root.appendingPathComponent("pet.json").path) {
                packages = [root]
            } else {
                packages = (try? FileManager.default.contentsOfDirectory(
                    at: root,
                    includingPropertiesForKeys: [.isDirectoryKey],
                    options: [.skipsHiddenFiles]
                )) ?? []
            }

            for package in packages.sorted(by: { $0.lastPathComponent < $1.lastPathComponent }) {
                let normalized = package.standardizedFileURL.path
                guard seenPaths.insert(normalized).inserted else { continue }
                let manifestURL = package.appendingPathComponent("pet.json")
                guard let data = try? Data(contentsOf: manifestURL),
                      let manifest = try? JSONDecoder().decode(PetManifest.self, from: data),
                      seenIDs.insert(manifest.id).inserted else { continue }
                result.append(PetDescriptor(
                    id: manifest.id,
                    displayName: manifest.displayName,
                    packageURL: package,
                    spriteVersionNumber: manifest.spriteVersionNumber ?? 1
                ))
            }
        }

        return result.sorted {
            $0.displayName.localizedCaseInsensitiveCompare($1.displayName) == .orderedAscending
        }
    }
}
