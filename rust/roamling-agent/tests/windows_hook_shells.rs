// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

#![cfg(windows)]

use roamling_agent::{installer, normalize::Agent};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// Exercise the generated command, including stdin forwarding and a nonempty
// response. An assertion on its text alone cannot detect a shell-created NUL.
fn check_shell(executable: PathBuf, args: &[&str]) {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let folder =
        std::env::temp_dir().join(format!("roamling-hook-{}-{unique}", std::process::id()));
    std::fs::create_dir(&folder).unwrap();
    let payload = b"{\"hook_event_name\":\"PreToolUse\",\"session_id\":\"shell-test\"}";
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();
    let server = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline, "hook never reached the receiver");
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(error) => panic!("accept: {error}"),
            }
        };
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut request = Vec::new();
        let mut buffer = [0; 4096];
        loop {
            let count = stream.read(&mut buffer).unwrap();
            assert_ne!(count, 0, "incomplete hook request");
            request.extend_from_slice(&buffer[..count]);
            if let Some(end) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
                if request.len() >= end + 4 + payload.len() {
                    assert_eq!(&request[end + 4..], payload);
                    break;
                }
            }
        }
        assert!(String::from_utf8_lossy(&request).contains("X-Roamling-Token: test-token"));
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\nresponse")
            .unwrap();
    });
    let command = installer::command(Agent::ClaudeCode, "test-token")
        .replace("127.0.0.1:47831", &address.to_string());
    let run = |command: &str| {
        let mut child = Command::new(&executable)
            .args(args)
            .arg(command)
            .current_dir(&folder)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // Local test traffic must not inherit a developer's proxy.
            .env("NO_PROXY", "127.0.0.1")
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(payload).unwrap();
        child.wait_with_output().unwrap()
    };
    let output = run(&command);
    let served = server.join();
    assert!(output.status.success(), "{executable:?}: {output:?}");
    served.unwrap();
    assert!(output.stdout.is_empty(), "response leaked: {output:?}");
    assert!(output.stderr.is_empty(), "diagnostics leaked: {output:?}");
    assert_eq!(
        std::fs::read_dir(&folder).unwrap().count(),
        0,
        "hook created a file"
    );

    // The receiver is now closed. Curl must still be quiet and leave cwd clean.
    let failed = run(&command);
    assert!(failed.stdout.is_empty(), "{failed:?}");
    assert!(failed.stderr.is_empty(), "{failed:?}");
    assert_eq!(
        std::fs::read_dir(&folder).unwrap().count(),
        0,
        "failed hook created a file"
    );
    std::fs::remove_dir(folder).unwrap();
}

#[test]
fn powershell_hook_forwards_stdin_without_creating_files() {
    check_shell(
        "powershell.exe".into(),
        &["-NoProfile", "-NonInteractive", "-Command"],
    );
}

#[test]
fn git_bash_hook_forwards_stdin_without_creating_files() {
    let bash = std::env::var_os("CLAUDE_CODE_GIT_BASH_PATH")
        .map(PathBuf::from)
        .or_else(|| {
            ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"]
                .iter()
                .filter_map(std::env::var_os)
                .map(|base| PathBuf::from(base).join("Git/bin/bash.exe"))
                .find(|path| path.is_file())
        });
    let Some(bash) = bash else {
        eprintln!("Git Bash not installed; set CLAUDE_CODE_GIT_BASH_PATH to exercise this test");
        return;
    };
    check_shell(bash, &["--noprofile", "--norc", "-c"]);
}
