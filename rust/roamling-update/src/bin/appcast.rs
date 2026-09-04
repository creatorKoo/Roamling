// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Makes the release key, and signs a release with it.
//!
//! Two commands, run from CI or by hand:
//!
//! ```sh
//! roamling-appcast keygen
//! roamling-appcast sign 0.2.0 <notes-url> windows-x86_64=<url>=<file> [more...]
//! roamling-appcast verify <public-key-hex> <platform>=<file> [more...]
//! ```
//!
//! `verify` reads the feed the way the app reads it, against the key the app
//! carries. Running it before publishing is the difference between finding a
//! broken release now and finding it when nobody can update.
//!
//! `sign` writes `appcast.json` and `appcast.json.sig` to the working
//! directory. The secret key comes from `ROAMLING_UPDATE_SECRET_KEY` and is
//! never written anywhere, never printed, and never passed as an argument --
//! an argument would land in the process list and in shell history.

use ed25519_dalek::{Signer, SigningKey};
use roamling_update::{decode_hex, encode_hex, Appcast, Artifact, SCHEMA_VERSION};
use std::collections::BTreeMap;
use std::process::ExitCode;

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let command = arguments.first().map(String::as_str);
    let result = match command {
        Some("keygen") => keygen(),
        Some("sign") => sign(&arguments[1..]),
        Some("verify") => verify(&arguments[1..]),
        _ => Err(usage()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn usage() -> String {
    concat!(
        "roamling-appcast keygen\n",
        "roamling-appcast sign <version> <notes-url> <platform>=<url>=<file> [...]\n",
        "roamling-appcast verify <public-key-hex> <platform>=<file> [...]\n",
        "\n",
        "  The secret key is read from ROAMLING_UPDATE_SECRET_KEY, never from an\n",
        "  argument. Writes appcast.json and appcast.json.sig.\n",
    )
    .to_string()
}

fn keygen() -> Result<(), String> {
    let key = SigningKey::generate(&mut rand_core::OsRng);
    // The secret goes to stdout once and is never stored by this tool. Put it
    // in the CI secret and in whatever the human uses to keep secrets; there is
    // no recovery if it is lost, only a new key and a broken update path for
    // everyone already running an old build.
    println!("secret (ROAMLING_UPDATE_SECRET_KEY, keep this):");
    println!("  {}", encode_hex(&key.to_bytes()));
    println!();
    println!("public (paste into PUBLIC_KEY_HEX in roamling-update/src/lib.rs):");
    println!("  {}", encode_hex(key.verifying_key().as_bytes()));
    Ok(())
}

fn secret_key() -> Result<SigningKey, String> {
    let hex = std::env::var("ROAMLING_UPDATE_SECRET_KEY")
        .map_err(|_| "ROAMLING_UPDATE_SECRET_KEY is not set".to_string())?;
    let bytes: [u8; 32] = decode_hex(&hex)
        .ok_or("ROAMLING_UPDATE_SECRET_KEY is not 64 hex characters")?
        .try_into()
        .map_err(|_| "ROAMLING_UPDATE_SECRET_KEY is not 32 bytes".to_string())?;
    Ok(SigningKey::from_bytes(&bytes))
}

fn sign(arguments: &[String]) -> Result<(), String> {
    let (version, notes) = match arguments {
        [version, notes, rest @ ..] if !rest.is_empty() => (version, notes),
        _ => return Err(usage()),
    };
    roamling_update::Version::parse(version)
        .ok_or_else(|| format!("'{version}' is not major.minor.patch"))?;

    let key = secret_key()?;
    let mut platforms = BTreeMap::new();
    for entry in &arguments[2..] {
        // `platform=url=path`, split from the left twice so a URL keeping its
        // own `=` (a query string) stays intact.
        let (platform, rest) = entry
            .split_once('=')
            .ok_or_else(|| format!("'{entry}' is not platform=url=file"))?;
        let (url, path) = rest
            .rsplit_once('=')
            .ok_or_else(|| format!("'{entry}' is not platform=url=file"))?;

        let bytes = std::fs::read(path).map_err(|error| format!("{path}: {error}"))?;
        let signature = encode_hex(&key.sign(&bytes).to_bytes());
        println!("{platform}  {} bytes  {path}", bytes.len());
        platforms.insert(
            platform.to_string(),
            Artifact {
                url: url.to_string(),
                size: bytes.len() as u64,
                signature,
            },
        );
    }

    let appcast = Appcast {
        schema_version: SCHEMA_VERSION,
        version: version.trim_start_matches('v').to_string(),
        published: today(),
        notes: notes.clone(),
        platforms,
    };
    // Serialised once, then signed over exactly those bytes -- signing a
    // re-serialisation would be signing something the reader never sees.
    let json = serde_json::to_vec_pretty(&appcast).map_err(|error| error.to_string())?;
    let signature = encode_hex(&key.sign(&json).to_bytes());

    std::fs::write("appcast.json", &json).map_err(|error| format!("appcast.json: {error}"))?;
    std::fs::write("appcast.json.sig", &signature)
        .map_err(|error| format!("appcast.json.sig: {error}"))?;
    println!("wrote appcast.json ({} bytes) and appcast.json.sig", json.len());
    Ok(())
}

/// `YYYY-MM-DD`, without a date crate. The feed shows it and nothing parses it.
fn today() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or(0);
    let days = (seconds / 86_400) as i64;

    // Civil-from-days, the standard algorithm, shifted to an era starting
    // 0000-03-01 so leap days land at the end of a cycle.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = if month <= 2 { year + 1 } else { year };
    format!("{year:04}-{month:02}-{day:02}")
}

/// Reads `appcast.json` back the way the app will, and checks the artifacts
/// against it. Takes the public key as an argument rather than using the one
/// compiled in, so a release can be checked against the key that will actually
/// ship -- including before that key has been pasted into the source.
fn verify(arguments: &[String]) -> Result<(), String> {
    let (key_hex, files) = match arguments {
        [key, rest @ ..] => (key, rest),
        _ => return Err(usage()),
    };
    let raw: [u8; 32] = decode_hex(key_hex)
        .ok_or("the public key is not 64 hex characters")?
        .try_into()
        .map_err(|_| "the public key is not 32 bytes".to_string())?;
    let key = ed25519_dalek::VerifyingKey::from_bytes(&raw)
        .map_err(|error| format!("not a public key: {error}"))?;

    let manifest = std::fs::read("appcast.json").map_err(|error| format!("appcast.json: {error}"))?;
    let signature = std::fs::read_to_string("appcast.json.sig")
        .map_err(|error| format!("appcast.json.sig: {error}"))?;

    let appcast = roamling_update::read_manifest_with(&key, &manifest, signature.trim())
        .map_err(|error| format!("appcast.json: {error}"))?;
    println!("appcast.json  version {}  signature ok", appcast.version);

    for entry in files {
        let (platform, path) = entry
            .split_once('=')
            .ok_or_else(|| format!("'{entry}' is not platform=file"))?;
        let artifact = appcast
            .platforms
            .get(platform)
            .ok_or_else(|| format!("the feed has nothing for {platform}"))?;
        let bytes = std::fs::read(path).map_err(|error| format!("{path}: {error}"))?;
        roamling_update::verify_artifact_with(&key, &bytes, artifact)
            .map_err(|error| format!("{platform}: {error}"))?;
        println!(
            "{platform}  {} bytes  signature ok  {}",
            bytes.len(),
            artifact.url
        );
    }
    Ok(())
}
