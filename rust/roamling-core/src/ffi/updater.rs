// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only


// -------------------------------------------------------------- the updater

/// What the feed says, once it has been read and trusted.
///
/// The decisions belong to `roamling-update` and are shared with Windows; what
/// crosses here is only the answer, because the fetching and the swapping are
/// the platform's and macOS does them in Swift.
#[derive(uniffi::Record)]
pub struct FfiUpdate {
    pub version: String,
    pub url: String,
    pub size: u64,
    /// Ed25519 over the artifact's bytes, as 128 hex characters.
    pub signature: String,
    /// Where the release notes are. A link the user may open; never fetched.
    pub notes: String,
}

/// The outcome of reading a feed. An error and an update are both optional and
/// never both present: a feed that could not be trusted has no answer in it.
#[derive(uniffi::Record)]
pub struct FfiUpdateCheck {
    pub error: Option<String>,
    pub update: Option<FfiUpdate>,
}

/// Which artifact in the feed belongs to this machine.
#[uniffi::export]
pub fn update_platform() -> String {
    roamling_update::PLATFORM.to_string()
}

#[uniffi::export]
pub fn update_current_version() -> String {
    roamling_update::Version::current().to_string()
}

/// Reads the feed, checks its signature, and says whether there is anything
/// newer for this machine.
///
/// The signature is checked before the JSON is parsed, so malformed bytes from
/// an untrusted source never reach the parser.
#[uniffi::export]
pub fn update_check(manifest: Vec<u8>, signature: String) -> FfiUpdateCheck {
    let appcast = match roamling_update::read_manifest(&manifest, signature.trim()) {
        Ok(appcast) => appcast,
        Err(error) => {
            return FfiUpdateCheck {
                error: Some(error.to_string()),
                update: None,
            }
        }
    };
    match roamling_update::decide(
        &appcast,
        roamling_update::Version::current(),
        roamling_update::PLATFORM,
    ) {
        Ok(roamling_update::Decision::UpToDate) => FfiUpdateCheck {
            error: None,
            update: None,
        },
        Ok(roamling_update::Decision::Update { version, artifact }) => FfiUpdateCheck {
            error: None,
            update: Some(FfiUpdate {
                version: version.to_string(),
                url: artifact.url,
                size: artifact.size,
                signature: artifact.signature,
                notes: appcast.notes,
            }),
        },
        Err(error) => FfiUpdateCheck {
            error: Some(error.to_string()),
            update: None,
        },
    }
}

/// Whether these bytes are the ones the feed described. Nothing unverified is
/// ever unpacked, let alone put in place of the running app.
#[uniffi::export]
pub fn update_verify(bytes: Vec<u8>, size: u64, signature: String) -> Option<String> {
    let artifact = roamling_update::Artifact {
        url: String::new(),
        size,
        signature,
    };
    match roamling_update::verify_artifact(&bytes, &artifact) {
        Ok(()) => None,
        Err(error) => Some(error.to_string()),
    }
}

/// Sheet bytes in, RGBA8 premultiplied out -- the one thing the pet layer
/// cannot do with arithmetic alone.
///
/// It used to be a platform capability, on the reasoning that macOS answers
/// WebP and PNG through ImageIO for free and Windows does not. Both answer
/// through this now, which is what W2b was for: two decoders meant two sets of
/// bytes, and they differed -- CoreGraphics rounds when it premultiplies and
/// this did not, so 1.66% of the standard sheet came out a channel darker on
/// Windows. Measured byte for byte against ImageIO on nine sheets, PNG and
/// WebP, built-in and packaged, after the rounding was fixed.
///
/// Returns nil for a file this cannot read, which the caller already had to
/// handle -- a pet package can name a sheet that is missing or malformed.
#[uniffi::export]
pub fn decode_pet_image(bytes: Vec<u8>) -> Option<FfiPetImage> {
    crate::PetImage::decode(&bytes).map(|image| FfiPetImage {
        width: image.width as u32,
        height: image.height as u32,
        pixels: image.pixels,
    })
}

#[derive(uniffi::Record)]
pub struct FfiPetImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}
