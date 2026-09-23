// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Getting the new version onto the disk, and into the running pet.
//!
//! The decisions -- is it newer, is it ours -- are `roamling-update`'s, and
//! macOS shares them. What is here is what touches this machine: fetching
//! bytes, putting them beside the executable, swapping them in, and starting
//! the new one.
//!
//! ## Replacing a running executable
//!
//! Windows will not let the file backing a running process be deleted or
//! overwritten. It *will* let it be renamed: the lock is on the file's contents,
//! not on its name. So a check writes `roamling.exe.new` (`stage`), and when the
//! pet is idle the swap (`install`) is
//!
//! ```text
//! roamling.exe     -> roamling.exe.old  the running process keeps running
//! roamling.exe.new -> roamling.exe      and starts this one on its way out
//! ```
//!
//! followed at once by `relaunch`. The new copy waits for the old one to be
//! gone (`wait_for_previous_instance`) and deletes the leftover. Swap and
//! restart go together on both platforms because on macOS a process whose
//! bundle was swapped loses the screen (`docs/windows.md` "자동 업데이트").
//! This is the same lock that makes `scripts/run.ps1` stop the pet before
//! building.
//!
//! ## Never annoying
//!
//! Nothing here shows a window on its own. An automatic check that finds
//! nothing says nothing; one that finds something stages it and restarts the
//! pet when it is idle. Only a check the user asked for reports back.

use roamling_update::{Appcast, Decision, Version};
use std::path::PathBuf;
use std::sync::mpsc::Sender;

use windows::core::PCWSTR;
use windows::Win32::Networking::WinHttp::*;

/// `/releases/latest/download/` always redirects to the newest release's asset,
/// so the feed needs no GitHub Pages site and no branch of its own -- the same
/// CI step that uploads the build uploads this.
const FEED: &str = "https://github.com/creatorKoo/Roamling/releases/latest/download/appcast.json";
const FEED_SIGNATURE: &str =
    "https://github.com/creatorKoo/Roamling/releases/latest/download/appcast.json.sig";

/// Twenty-four hours. A desktop pet checking more often than that is spending
/// the user's battery to find out nothing, which `docs/battery.md` would have
/// something to say about.
pub const INTERVAL: f64 = 24.0 * 60.0 * 60.0;

/// A manifest bigger than this is not our manifest, and an artifact bigger than
/// this is not our 2.5 MB executable. Both are read into memory to be verified
/// before anything is written, so both need a ceiling.
const MAXIMUM_FEED_BYTES: usize = 64 * 1_024;
const MAXIMUM_ARTIFACT_BYTES: usize = 64 * 1_024 * 1_024;

#[derive(Debug)]
pub enum Outcome {
    UpToDate,
    /// Downloaded, verified and written beside the executable. `install` puts
    /// it in place.
    Staged(Version),
    Failed(String),
}

/// What came back, and whether anyone is waiting to hear about it.
#[derive(Debug)]
pub struct Report {
    pub outcome: Outcome,
    /// True when the user picked "Check for Updates" and is owed an answer.
    pub asked: bool,
}

/// Runs the whole check on a thread of its own.
///
/// Network calls block, and this loop has a pet to draw. The result comes back
/// through the channel and the tick picks it up, which is the same shape the
/// agent endpoints use.
pub fn check(events: Sender<Report>, asked: bool) {
    let _ = std::thread::Builder::new()
        .name("roamling-update".into())
        .spawn(move || {
            let outcome = match run() {
                Ok(outcome) => outcome,
                Err(error) => Outcome::Failed(error),
            };
            let _ = events.send(Report { outcome, asked });
        });
}

fn run() -> Result<Outcome, String> {
    let manifest = fetch(FEED, MAXIMUM_FEED_BYTES)?;
    let signature = fetch(FEED_SIGNATURE, MAXIMUM_FEED_BYTES)?;
    let signature = String::from_utf8(signature).map_err(|_| "the signature is not text".to_string())?;

    let appcast: Appcast =
        roamling_update::read_manifest(&manifest, signature.trim()).map_err(|e| e.to_string())?;
    let decision = roamling_update::decide(&appcast, Version::current(), roamling_update::PLATFORM)
        .map_err(|e| e.to_string())?;

    let (version, artifact) = match decision {
        Decision::UpToDate => return Ok(Outcome::UpToDate),
        Decision::Update { version, artifact } => (version, artifact),
    };

    let bytes = fetch(&artifact.url, MAXIMUM_ARTIFACT_BYTES)?;
    // Verified in memory. Nothing unverified is ever written next to the
    // executable, let alone put in its place.
    roamling_update::verify_artifact(&bytes, &artifact).map_err(|e| e.to_string())?;
    stage(&bytes)?;
    Ok(Outcome::Staged(version))
}

/// Where the running executable is, and the two names used around it.
fn paths() -> Result<(PathBuf, PathBuf, PathBuf), String> {
    let current = std::env::current_exe().map_err(|error| error.to_string())?;
    let mut staged = current.clone().into_os_string();
    staged.push(".new");
    let mut previous = current.clone().into_os_string();
    previous.push(".old");
    Ok((current, PathBuf::from(staged), PathBuf::from(previous)))
}

/// Writes verified bytes beside the running executable. Nothing the running
/// process uses is touched.
fn stage(bytes: &[u8]) -> Result<(), String> {
    let (_, staged, _) = paths()?;
    std::fs::write(&staged, bytes).map_err(|error| format!("{}: {error}", staged.display()))
}

/// Puts the staged executable in place of the running one. Right before
/// `relaunch`, or at quit.
pub fn install() -> Result<(), String> {
    let (current, staged, previous) = paths()?;
    install_at(&current, &staged, &previous)
}

/// The swap itself, against named paths so a test can drive it somewhere other
/// than over the executable it is running from.
fn install_at(
    current: &std::path::Path,
    staged: &std::path::Path,
    previous: &std::path::Path,
) -> Result<(), String> {
    if !staged.exists() {
        return Err(format!("{} is gone", staged.display()));
    }
    // A leftover from an interrupted attempt, or from a launch that could not
    // clean up. Either way it is in the way.
    let _ = std::fs::remove_file(previous);

    // Renaming the running executable is allowed; deleting it is not.
    if let Err(error) = std::fs::rename(current, previous) {
        let _ = std::fs::remove_file(staged);
        return Err(format!("{}: {error}", current.display()));
    }
    if let Err(error) = std::fs::rename(staged, current) {
        // Put the old one back rather than leaving the user with no executable
        // at all -- this is the one failure that would matter.
        let _ = std::fs::rename(previous, current);
        let _ = std::fs::remove_file(staged);
        return Err(format!("{}: {error}", current.display()));
    }
    Ok(())
}

const AFTER_FLAG: &str = "--after";

/// Starts the executable that `install` just put in place, telling it which
/// process to wait for. The caller then quits the ordinary way.
pub fn relaunch() -> Result<(), String> {
    let (current, _, _) = paths()?;
    std::process::Command::new(&current)
        .arg(AFTER_FLAG)
        .arg(std::process::id().to_string())
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("{}: {error}", current.display()))
}

/// The other half of `relaunch`, and it has to come before the single-instance
/// claim: the copy being replaced still holds that mutex, and a new one that
/// found it taken would quietly exit -- no pet at all. Bounded, so a copy that
/// will not quit cannot keep the pet away forever.
pub fn wait_for_previous_instance() {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE};

    let arguments: Vec<String> = std::env::args().collect();
    let Some(pid) = previous_instance(&arguments) else {
        return;
    };
    unsafe {
        // Already gone is the usual case by the time this runs, and also fine.
        if let Ok(process) = OpenProcess(PROCESS_SYNCHRONIZE, false, pid) {
            let _ = WaitForSingleObject(process, 10_000);
            let _ = CloseHandle(process);
        }
    }
}

fn previous_instance(arguments: &[String]) -> Option<u32> {
    let flag = arguments.iter().position(|argument| argument == AFTER_FLAG)?;
    arguments.get(flag + 1)?.parse().ok()
}

/// Clears what a swap left behind, and a staged file that never got swapped in
/// (the process was killed first; the next check fetches it again). Called once
/// at startup.
///
/// Failure is ignored on purpose: another copy may still be running from it,
/// and a file that could not be deleted this launch will be deleted the next.
pub fn clean_up() {
    if let Ok((_, staged, previous)) = paths() {
        let _ = std::fs::remove_file(previous);
        let _ = std::fs::remove_file(staged);
    }
}


// ---------------------------------------------------------------------- http

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// `https://host/path`, split. Only https, because an updater that will speak
/// plaintext has a downgrade attack built into it -- the signature would still
/// catch tampering, but there is no reason to allow the attempt.
fn split_url(url: &str) -> Result<(String, String), String> {
    let rest = url
        .strip_prefix("https://")
        .ok_or_else(|| format!("not an https URL: {url}"))?;
    let (host, path) = match rest.split_once('/') {
        Some((host, path)) => (host, format!("/{path}")),
        None => (rest, "/".to_string()),
    };
    if host.is_empty() {
        return Err(format!("no host in {url}"));
    }
    Ok((host.to_string(), path))
}

/// Closes a WinHTTP handle when it goes out of scope, including on the error
/// paths -- of which this function has many.
struct Handle(*mut std::ffi::c_void);

impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { let _ = WinHttpCloseHandle(self.0); }
        }
    }
}

fn fetch(url: &str, limit: usize) -> Result<Vec<u8>, String> {
    let (host, path) = split_url(url)?;
    let agent = wide(concat!("Roamling/", env!("CARGO_PKG_VERSION")));
    let host_wide = wide(&host);
    let path_wide = wide(&path);
    let verb = wide("GET");

    unsafe {
        let session = Handle(WinHttpOpen(
            PCWSTR(agent.as_ptr()),
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
            PCWSTR::null(),
            PCWSTR::null(),
            0,
        ));
        if session.0.is_null() {
            return Err("could not open an HTTP session".into());
        }

        let connection = Handle(WinHttpConnect(
            session.0,
            PCWSTR(host_wide.as_ptr()),
            INTERNET_DEFAULT_HTTPS_PORT as u16,
            0,
        ));
        if connection.0.is_null() {
            return Err(format!("could not connect to {host}"));
        }

        let request = Handle(WinHttpOpenRequest(
            connection.0,
            PCWSTR(verb.as_ptr()),
            PCWSTR(path_wide.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            std::ptr::null_mut(),
            WINHTTP_FLAG_SECURE,
        ));
        if request.0.is_null() {
            return Err(format!("could not open a request for {url}"));
        }

        WinHttpSendRequest(request.0, None, None, 0, 0, 0)
            .map_err(|error| format!("{url}: {error}"))?;
        WinHttpReceiveResponse(request.0, std::ptr::null_mut())
            .map_err(|error| format!("{url}: {error}"))?;

        let status = status_code(request.0)?;
        if status != 200 {
            return Err(format!("{url}: HTTP {status}"));
        }

        let mut body = Vec::new();
        loop {
            let mut available = 0u32;
            WinHttpQueryDataAvailable(request.0, &mut available)
                .map_err(|error| format!("{url}: {error}"))?;
            if available == 0 {
                break;
            }
            let wanted = available as usize;
            if body.len() + wanted > limit {
                return Err(format!("{url}: larger than {limit} bytes"));
            }
            let start = body.len();
            body.resize(start + wanted, 0);
            let mut read = 0u32;
            WinHttpReadData(
                request.0,
                body[start..].as_mut_ptr() as *mut std::ffi::c_void,
                available,
                &mut read,
            )
            .map_err(|error| format!("{url}: {error}"))?;
            body.truncate(start + read as usize);
            if read == 0 {
                break;
            }
        }
        Ok(body)
    }
}

fn status_code(request: *mut std::ffi::c_void) -> Result<u32, String> {
    let mut status = 0u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    unsafe {
        WinHttpQueryHeaders(
            request,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            PCWSTR::null(),
            Some(&mut status as *mut u32 as *mut std::ffi::c_void),
            &mut size,
            std::ptr::null_mut(),
        )
        .map_err(|error| format!("no status line: {error}"))?;
    }
    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_https_urls_are_accepted() {
        assert_eq!(
            split_url("https://example.com/a/b.json"),
            Ok(("example.com".into(), "/a/b.json".into()))
        );
        assert_eq!(
            split_url("https://example.com"),
            Ok(("example.com".into(), "/".into()))
        );
        assert!(split_url("http://example.com/x").is_err());
        assert!(split_url("ftp://example.com/x").is_err());
        assert!(split_url("https:///x").is_err());
    }

    /// The three names have to be siblings of the running executable, or the
    /// swap would rename it somewhere the next launch does not look.
    #[test]
    fn the_staging_names_sit_beside_the_executable() {
        let (current, staged, previous) = paths().expect("paths");
        assert_eq!(staged.parent(), current.parent());
        assert_eq!(previous.parent(), current.parent());
        let name = |path: &PathBuf| path.file_name().map(|n| n.to_string_lossy().into_owned());
        let current_name = name(&current).expect("name");
        assert_eq!(name(&staged), Some(format!("{current_name}.new")));
        assert_eq!(name(&previous), Some(format!("{current_name}.old")));
    }

    /// The feed URLs are the ones the release workflow uploads to, and both
    /// have to be https or `fetch` refuses them.
    #[test]
    fn the_feed_urls_parse() {
        assert!(split_url(FEED).is_ok());
        assert!(split_url(FEED_SIGNATURE).is_ok());
        assert!(FEED_SIGNATURE.starts_with(FEED.trim_end_matches(".json")));
    }

    /// A copy started by `relaunch` has to find the id it was handed, and a
    /// copy started any other way must not wait for anything.
    #[test]
    fn the_new_copy_reads_which_process_to_wait_for() {
        let args = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(previous_instance(&args(&["roamling.exe", "--after", "4242"])), Some(4242));
        assert_eq!(previous_instance(&args(&["roamling.exe"])), None);
        assert_eq!(previous_instance(&args(&["roamling.exe", "--after"])), None);
        assert_eq!(previous_instance(&args(&["roamling.exe", "--after", "soon"])), None);
    }

    fn scratch(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("roamling-update-{}-{name}", std::process::id()));
        let _ = std::fs::create_dir_all(&root);
        root
    }

    /// The swap, on real files. The point is that the old bytes survive under
    /// the `.old` name -- that is what makes a rollback possible and what stops
    /// a half-finished update from leaving nothing to run.
    #[test]
    fn the_swap_replaces_the_file_and_keeps_the_old_one() {
        let root = scratch("swap");
        let (current, staged, previous) = (
            root.join("roamling.exe"),
            root.join("roamling.exe.new"),
            root.join("roamling.exe.old"),
        );
        std::fs::write(&current, b"old version").expect("seed");

        std::fs::write(&staged, b"new version").expect("stage");
        install_at(&current, &staged, &previous).expect("the swap failed");
        assert_eq!(std::fs::read(&current).expect("current"), b"new version");
        assert_eq!(std::fs::read(&previous).expect("previous"), b"old version");
        assert!(!staged.exists(), "the staging file should be gone");

        // A second update finds the previous `.old` in the way and replaces it.
        std::fs::write(&staged, b"newer still").expect("stage");
        install_at(&current, &staged, &previous).expect("the second swap failed");
        assert_eq!(std::fs::read(&current).expect("current"), b"newer still");
        assert_eq!(std::fs::read(&previous).expect("previous"), b"new version");

        // Nothing staged is nothing to do, and the running file stays put.
        install_at(&current, &staged, &previous).expect_err("there was nothing to install");
        assert_eq!(std::fs::read(&current).expect("current"), b"newer still");

        let _ = std::fs::remove_dir_all(&root);
    }

    /// The failure that would actually hurt: the second rename does not happen
    /// and the user is left with nothing to run. It has to roll back.
    #[test]
    fn a_failed_swap_puts_the_old_file_back() {
        use std::os::windows::fs::OpenOptionsExt;

        let root = scratch("rollback");
        let (current, staged, previous) = (
            root.join("roamling.exe"),
            root.join("roamling.exe.new"),
            root.join("roamling.exe.old"),
        );
        std::fs::write(&current, b"old version").expect("seed");
        std::fs::write(&staged, b"new version").expect("stage");
        // Held open with no sharing, the staged file cannot be renamed. The
        // first rename (the running file aside) still succeeds, so this is the
        // second one failing -- the case that would leave nothing to run.
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&staged)
            .expect("lock");

        let error = install_at(&current, &staged, &previous)
            .expect_err("renaming a locked file should not succeed");
        drop(lock);
        assert!(!error.is_empty());
        assert_eq!(
            std::fs::read(&current).expect("current"),
            b"old version",
            "the file that was there must still be there"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    /// A real fetch over the real internet, including a cross-host redirect --
    /// which is exactly what the release asset URLs do. Ignored by default so a
    /// machine with no network does not fail the suite; run it with
    /// `cargo test -p roamling-win -- --ignored` when touching the HTTP path.
    #[test]
    #[ignore = "needs the network"]
    fn a_real_https_get_comes_back_with_bytes() {
        let body = fetch(
            "https://raw.githubusercontent.com/rust-lang/rust/master/LICENSE-MIT",
            MAXIMUM_FEED_BYTES,
        )
        .expect("the fetch failed");
        assert!(body.len() > 500, "got {} bytes", body.len());
        assert!(String::from_utf8_lossy(&body).contains("Permission is hereby granted"));

        // And something that is not there is an error rather than empty bytes.
        let missing = fetch(
            "https://raw.githubusercontent.com/rust-lang/rust/master/no-such-file-here",
            MAXIMUM_FEED_BYTES,
        );
        assert!(missing.is_err(), "a 404 came back as success");
    }
}
