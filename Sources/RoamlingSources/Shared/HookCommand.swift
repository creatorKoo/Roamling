// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation

/// How an agent's hook reaches Roamling.
///
/// The string here is written into the user's own config, so it has to be a
/// command their shell will run -- which makes it the one place in this module
/// where the host operating system shows through. Both installers share it so
/// the two configs cannot drift.
public enum HookCommand {
    /// Windows has shipped `curl.exe` in System32 since 10 build 1803, and it
    /// is the real curl rather than an alias, so only the path differs.
    public static var curlPath: String {
        #if os(Windows)
        "curl.exe"
        #else
        "/usr/bin/curl"
        #endif
    }

    /// Discards output and never fails: a closed companion must not surface a
    /// hook error, which a native `http` handler used to do on every session
    /// that outlived the app.
    private static var silencer: String {
        #if os(Windows)
        ">NUL 2>&1"
        #else
        ">/dev/null 2>&1 || true"
        #endif
    }

    /// Quoting differs: cmd.exe does not treat single quotes as grouping.
    private static func quoted(_ value: String) -> String {
        #if os(Windows)
        "\"\(value)\""
        #else
        "'\(value)'"
        #endif
    }

    /// Forwards the hook's stdin, unread, to the authenticated loopback
    /// receiver. Nothing about the payload is inspected on the way.
    public static func forwardStandardInput(
        to endpoint: URL,
        tokenHeader: String,
        token: String,
        marker: String
    ) -> String {
        "\(curlPath) --silent --connect-timeout 0.15 --max-time 0.3 "
            + "--request POST --header \(quoted("Content-Type: application/json")) "
            + "--header \(quoted("\(tokenHeader): \(token)")) --data-binary @- "
            + "\(quoted(endpoint.absoluteString)) \(silencer) # \(marker)"
    }
}
