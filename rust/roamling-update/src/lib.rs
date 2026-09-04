// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Is there a new version, and is it ours?
//!
//! Everything here is a decision, so both platforms share it -- the same split
//! the rest of the project uses. What is left for the shell is the two things
//! that touch the machine: fetching bytes, and replacing a file.
//!
//! ## Why there is a signature at all
//!
//! An updater without one is a remote code execution path with extra steps.
//! HTTPS authenticates the *transport*; it says nothing about a release host
//! that has been compromised or a publishing token that leaked. So the bytes
//! are signed with a key that never touches the server, and the public half is
//! compiled into the app. This is the same reason Sparkle carries EdDSA, and it
//! is independent of Authenticode -- a code-signing certificate answers "should
//! Windows warn the user", not "did this come from us".
//!
//! Two signatures, because they answer different questions:
//!
//! - **The manifest**, so the version numbers and URLs cannot be edited. Without
//!   this, whoever serves the feed could claim 9.9.9 and point it at a genuine
//!   but ancient artifact, and the signature on that artifact would check out.
//! - **Each artifact**, so the bytes that arrive are the bytes we published.
//!
//! Ed25519 hashes the message internally, so neither needs a separate digest.

use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// The public half of the release key, as 64 hex characters.
///
/// Set for real when the key is generated -- `roamling-appcast keygen` prints
/// what to paste here. Until then it is all zeroes, and `public_key` reports
/// that as "not configured" rather than as a key: an updater that cannot check
/// a signature must refuse to update, not update anyway.
pub const PUBLIC_KEY_HEX: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

/// Which artifact in the manifest belongs to the machine this is running on.
pub const PLATFORM: &str = if cfg!(target_os = "windows") {
    "windows-x86_64"
} else if cfg!(target_arch = "aarch64") {
    "macos-arm64"
} else {
    "macos-x86_64"
};

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    /// The key in this build is the placeholder, so nothing can be verified.
    NoPublicKey,
    MalformedSignature,
    /// The bytes are not what was signed. Nothing further should happen.
    BadSignature,
    MalformedManifest(String),
    /// The feed is newer than this build knows how to read.
    UnsupportedSchema(u32),
    NoArtifactForPlatform(String),
    /// The artifact is not the size the manifest promised, which is worth
    /// saying before spending time on a signature check.
    WrongSize { expected: u64, got: u64 },
}

impl fmt::Display for Error {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoPublicKey => write!(out, "this build has no release key, so it cannot update"),
            Self::MalformedSignature => write!(out, "the signature is not 64 bytes of hex"),
            Self::BadSignature => write!(out, "the signature does not match the bytes"),
            Self::MalformedManifest(why) => write!(out, "the manifest could not be read: {why}"),
            Self::UnsupportedSchema(version) => {
                write!(out, "the feed is schema {version}, which this build cannot read")
            }
            Self::NoArtifactForPlatform(platform) => {
                write!(out, "the release has nothing for {platform}")
            }
            Self::WrongSize { expected, got } => {
                write!(out, "expected {expected} bytes, got {got}")
            }
        }
    }
}

// ------------------------------------------------------------------- version

/// `major.minor.patch`, and nothing else.
///
/// Deliberately not a full semver implementation: the only question ever asked
/// is "is the feed newer than what is running", and a pre-release ordering
/// nobody publishes is a rule that can only be got wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Version {
    /// This build's own version, from the crate metadata the workspace shares.
    pub fn current() -> Self {
        Self::parse(env!("CARGO_PKG_VERSION")).unwrap_or(Self {
            major: 0,
            minor: 0,
            patch: 0,
        })
    }

    pub fn parse(text: &str) -> Option<Self> {
        // A leading `v` is what a git tag looks like, and the feed is generated
        // from tags. Accepting it here costs nothing and saves a release that
        // silently never updates anyone.
        let text = text.trim().strip_prefix('v').unwrap_or(text.trim());
        let mut parts = text.split('.');
        let mut next = || parts.next()?.parse::<u32>().ok();
        let version = Self {
            major: next()?,
            minor: next()?,
            patch: next()?,
        };
        if parts.next().is_some() {
            return None;
        }
        Some(version)
    }
}

impl fmt::Display for Version {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

// ------------------------------------------------------------------ manifest

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    pub url: String,
    pub size: u64,
    /// Ed25519 over the artifact's bytes, as 128 hex characters.
    pub signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Appcast {
    pub schema_version: u32,
    pub version: String,
    pub published: String,
    /// Where the release notes are. Never fetched by the app -- it is a link the
    /// user may open, and rendering remote HTML in a desktop pet is not on the
    /// table.
    pub notes: String,
    pub platforms: BTreeMap<String, Artifact>,
}

/// What should happen, given a feed and what is running.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    UpToDate,
    /// Worth downloading. Carries the artifact so the caller does not have to
    /// look up the platform a second time.
    Update {
        version: Version,
        artifact: Artifact,
    },
}

/// Reads a signed feed, or refuses.
///
/// `signature` is the detached `appcast.json.sig`, over the exact bytes of
/// `manifest`. Verified *before* parsing, so malformed JSON from an unsigned
/// source never reaches the parser.
pub fn read_manifest(manifest: &[u8], signature: &str) -> Result<Appcast, Error> {
    read_manifest_with(&public_key().ok_or(Error::NoPublicKey)?, manifest, signature)
}

/// The same, against a key that is not the shipped one. This is what lets the
/// tests exercise the real path rather than a second copy of it.
pub fn read_manifest_with(
    key: &VerifyingKey,
    manifest: &[u8],
    signature: &str,
) -> Result<Appcast, Error> {
    verify_with(key, manifest, signature)?;
    let appcast: Appcast =
        serde_json::from_slice(manifest).map_err(|error| Error::MalformedManifest(error.to_string()))?;
    if appcast.schema_version != SCHEMA_VERSION {
        return Err(Error::UnsupportedSchema(appcast.schema_version));
    }
    Ok(appcast)
}

/// Whether this feed is offering something newer, for this platform.
///
/// Strictly newer: equal is up to date, and older is refused rather than
/// installed. A feed that offers a downgrade is either a mistake or an attack,
/// and there is no version of "roll the user back silently" that is correct.
pub fn decide(appcast: &Appcast, current: Version, platform: &str) -> Result<Decision, Error> {
    let offered = Version::parse(&appcast.version)
        .ok_or_else(|| Error::MalformedManifest(format!("version '{}'", appcast.version)))?;
    if offered <= current {
        return Ok(Decision::UpToDate);
    }
    let artifact = appcast
        .platforms
        .get(platform)
        .ok_or_else(|| Error::NoArtifactForPlatform(platform.to_string()))?;
    Ok(Decision::Update {
        version: offered,
        artifact: artifact.clone(),
    })
}

/// The downloaded bytes are the ones that were published, or they are not used.
pub fn verify_artifact(bytes: &[u8], artifact: &Artifact) -> Result<(), Error> {
    verify_artifact_with(&public_key().ok_or(Error::NoPublicKey)?, bytes, artifact)
}

pub fn verify_artifact_with(
    key: &VerifyingKey,
    bytes: &[u8],
    artifact: &Artifact,
) -> Result<(), Error> {
    if bytes.len() as u64 != artifact.size {
        return Err(Error::WrongSize {
            expected: artifact.size,
            got: bytes.len() as u64,
        });
    }
    verify_with(key, bytes, &artifact.signature)
}

/// The release key, or `None` when this build was never given one.
pub fn public_key() -> Option<VerifyingKey> {
    let bytes: [u8; 32] = decode_hex(PUBLIC_KEY_HEX)?.try_into().ok()?;
    if bytes.iter().all(|byte| *byte == 0) {
        return None;
    }
    VerifyingKey::from_bytes(&bytes).ok()
}

fn verify_with(key: &VerifyingKey, message: &[u8], signature: &str) -> Result<(), Error> {
    let raw: [u8; 64] = decode_hex(signature)
        .ok_or(Error::MalformedSignature)?
        .try_into()
        .map_err(|_| Error::MalformedSignature)?;
    // `verify_strict` rejects the small-order public keys and the malleable
    // encodings that plain `verify` accepts. There is no reason to be lenient
    // about a signature on an executable.
    key.verify_strict(message, &Signature::from_bytes(&raw))
        .map_err(|_| Error::BadSignature)
}

pub fn decode_hex(text: &str) -> Option<Vec<u8>> {
    let text = text.trim();
    if text.len() % 2 != 0 {
        return None;
    }
    let digits: Vec<u8> = text.bytes().collect();
    digits
        .chunks_exact(2)
        .map(|pair| {
            let high = (pair[0] as char).to_digit(16)?;
            let low = (pair[1] as char).to_digit(16)?;
            Some((high * 16 + low) as u8)
        })
        .collect()
}

pub fn encode_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from_digit((byte >> 4) as u32, 16).unwrap_or('0'));
        out.push(char::from_digit((byte & 0x0F) as u32, 16).unwrap_or('0'));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    /// A key for the tests only. `public_key` reads a compiled-in constant that
    /// is the placeholder until a release key exists, so the tests go through
    /// the `_with` half of each function -- the same code, a different key.
    fn key() -> SigningKey {
        SigningKey::from_bytes(&[7u8; 32])
    }

    fn signed(message: &[u8]) -> String {
        encode_hex(&key().sign(message).to_bytes())
    }

    fn check(message: &[u8], signature: &str) -> Result<(), Error> {
        verify_with(&key().verifying_key(), message, signature)
    }

    #[test]
    fn a_version_parses_with_or_without_the_tag_prefix() {
        assert_eq!(Version::parse("0.2.13"), Some(Version { major: 0, minor: 2, patch: 13 }));
        assert_eq!(Version::parse("v1.0.0"), Some(Version { major: 1, minor: 0, patch: 0 }));
        assert_eq!(Version::parse(" 1.0.0 "), Some(Version { major: 1, minor: 0, patch: 0 }));
        for bad in ["1.0", "1.0.0.0", "1.0.x", "", "latest"] {
            assert_eq!(Version::parse(bad), None, "{bad} was accepted");
        }
    }

    /// The ordering the whole feature rests on.
    #[test]
    fn versions_order_by_number_not_by_text() {
        let parse = |text| Version::parse(text).expect(text);
        assert!(parse("0.10.0") > parse("0.9.0"), "textual ordering would say otherwise");
        assert!(parse("1.0.0") > parse("0.99.99"));
        assert!(parse("0.1.2") > parse("0.1.1"));
        assert_eq!(parse("0.1.0"), parse("v0.1.0"));
    }

    fn appcast(version: &str) -> Appcast {
        let mut platforms = BTreeMap::new();
        platforms.insert(
            "windows-x86_64".to_string(),
            Artifact {
                url: "https://example.invalid/roamling.exe".into(),
                size: 3,
                signature: signed(b"abc"),
            },
        );
        Appcast {
            schema_version: SCHEMA_VERSION,
            version: version.into(),
            published: "2026-09-04".into(),
            notes: "https://example.invalid/notes".into(),
            platforms,
        }
    }

    /// Newer updates, equal does not, and **older does not** -- a feed offering
    /// a downgrade is a mistake or an attack, and there is no correct way to
    /// roll a user back without asking.
    #[test]
    fn only_a_strictly_newer_version_updates() {
        let current = Version::parse("0.2.0").expect("current");
        let platform = "windows-x86_64";

        assert_eq!(
            decide(&appcast("0.2.0"), current, platform),
            Ok(Decision::UpToDate)
        );
        assert_eq!(
            decide(&appcast("0.1.9"), current, platform),
            Ok(Decision::UpToDate)
        );
        match decide(&appcast("0.2.1"), current, platform) {
            Ok(Decision::Update { version, .. }) => assert_eq!(version.to_string(), "0.2.1"),
            other => panic!("expected an update, got {other:?}"),
        }
    }

    /// A release built for the other platform is not an update for this one.
    #[test]
    fn a_release_without_this_platform_is_an_error_not_an_update() {
        let mut feed = appcast("9.9.9");
        feed.platforms.remove("windows-x86_64");
        assert_eq!(
            decide(&feed, Version::parse("0.1.0").expect("v"), "windows-x86_64"),
            Err(Error::NoArtifactForPlatform("windows-x86_64".into()))
        );
    }

    /// The size is checked first so a truncated download says so, rather than
    /// arriving as a mysterious signature failure.
    #[test]
    fn a_short_download_is_reported_as_short() {
        let artifact = Artifact {
            url: "https://example.invalid/x".into(),
            size: 10,
            signature: signed(b"abc"),
        };
        assert_eq!(
            verify_artifact_with(&key().verifying_key(), b"abc", &artifact),
            Err(Error::WrongSize { expected: 10, got: 3 })
        );
    }

    /// The point of the whole crate: bytes that were not signed do not pass,
    /// and neither does a signature that is not a signature.
    #[test]
    fn only_the_signed_bytes_verify() {
        let signature = signed(b"the real release");
        assert_eq!(check(b"the real release", &signature), Ok(()));
        assert_eq!(check(b"the real releasf", &signature), Err(Error::BadSignature));
        assert_eq!(check(b"", &signature), Err(Error::BadSignature));

        // A signature by a different key.
        let other = encode_hex(&SigningKey::from_bytes(&[9u8; 32]).sign(b"the real release").to_bytes());
        assert_eq!(check(b"the real release", &other), Err(Error::BadSignature));

        for malformed in ["", "zz", &"ab".repeat(63), &"ab".repeat(65)] {
            assert_eq!(
                check(b"the real release", malformed),
                Err(Error::MalformedSignature),
                "{malformed:?} was accepted as a signature"
            );
        }
    }

    /// A build that was never given a key must refuse to update rather than
    /// update without checking. The shipped constant is the placeholder until a
    /// release key exists, so this also pins that it is still recognised as one.
    #[test]
    fn a_build_with_no_key_cannot_update() {
        if PUBLIC_KEY_HEX.chars().all(|c| c == '0') {
            assert!(public_key().is_none());
            assert_eq!(read_manifest(b"{}", &"00".repeat(64)), Err(Error::NoPublicKey));
        } else {
            assert!(public_key().is_some(), "the shipped key is not a valid one");
        }
    }

    /// Round-trip, because a manifest is generated by one tool and read by
    /// another and the field names have to survive the trip.
    #[test]
    fn a_manifest_survives_being_written_and_read() {
        let feed = appcast("1.2.3");
        let json = serde_json::to_vec_pretty(&feed).expect("write");
        let read: Appcast = serde_json::from_slice(&json).expect("read");
        assert_eq!(read, feed);
        // camelCase on the wire, because the macOS side reads the same file.
        let text = String::from_utf8(json).expect("utf8");
        assert!(text.contains("\"schemaVersion\""), "{text}");
    }

    /// The whole flow the app performs, against bytes produced the way the
    /// release tool produces them: serialise once, sign those exact bytes, and
    /// read them back. Signing a re-serialisation would sign something the
    /// reader never sees, and this is what would catch that.
    #[test]
    fn a_signed_feed_reads_back_and_a_tampered_one_does_not() {
        let feed = appcast("1.2.3");
        let json = serde_json::to_vec_pretty(&feed).expect("write");
        let signature = signed(&json);
        let public = key().verifying_key();

        let read = read_manifest_with(&public, &json, &signature).expect("the feed did not read");
        assert_eq!(read.version, "1.2.3");
        match decide(&read, Version::parse("1.0.0").expect("v"), "windows-x86_64") {
            Ok(Decision::Update { artifact, .. }) => {
                assert_eq!(verify_artifact_with(&public, b"abc", &artifact), Ok(()));
                assert_eq!(
                    verify_artifact_with(&public, b"xyz", &artifact),
                    Err(Error::BadSignature),
                    "an artifact that is not ours must not pass"
                );
            }
            other => panic!("expected an update, got {other:?}"),
        }

        // The attack the manifest signature exists to stop: claim a version the
        // feed never published, keeping a genuine artifact and its signature.
        let lie = String::from_utf8(json.clone())
            .expect("utf8")
            .replace("1.2.3", "9.9.9");
        assert_eq!(
            read_manifest_with(&public, lie.as_bytes(), &signature),
            Err(Error::BadSignature)
        );
    }

    /// A feed from a future schema is refused rather than read optimistically.
    #[test]
    fn an_unknown_schema_is_refused() {
        let mut feed = appcast("1.2.3");
        feed.schema_version = SCHEMA_VERSION + 1;
        let json = serde_json::to_vec_pretty(&feed).expect("write");
        assert_eq!(
            read_manifest_with(&key().verifying_key(), &json, &signed(&json)),
            Err(Error::UnsupportedSchema(SCHEMA_VERSION + 1))
        );
    }

    #[test]
    fn hex_round_trips_and_rejects_rubbish() {
        assert_eq!(encode_hex(&[0x00, 0x0f, 0xff]), "000fff");
        assert_eq!(decode_hex("000fff"), Some(vec![0x00, 0x0f, 0xff]));
        assert_eq!(decode_hex("00f"), None);
        assert_eq!(decode_hex("00gg"), None);
    }
}
