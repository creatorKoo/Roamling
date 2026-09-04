// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Pet packages on disk: finding them, and loading one.
//!
//! Ported from `PetCatalog`, `PetManifest` and `PetLoader`. The three are one
//! file here because they are one job -- a directory becomes a `PetAsset` -- and
//! the split on the Swift side is a module-per-type habit rather than a seam.
//!
//! **The package contract is not ours.** `pet.json` and `spritesheet.webp` are
//! Petdex's, and Roamling reads them without asking for anything extra. What
//! Petdex has no word for -- sleeping, being carried, watching the cursor --
//! lives in an optional `roamling.json` on a sheet of its own, addressed by
//! continuing the frame index past the end of the package grid. A package
//! without that file is the compatibility path and must keep working.

use crate::{PetAsset, PetImage};
use roamling_core::{standard_tracks, PetAnimationFrame, PetAnimationTrack};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

/// A sheet larger than this is not a pet, it is a mistake or an attack. The
/// number is `PetLoader.maximumEncodedBytes`.
const MAXIMUM_ENCODED_BYTES: u64 = 32 * 1_024 * 1_024;
const MAXIMUM_FRAMES: usize = 256;
const ROAMLING_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Deserialize)]
struct FrameManifest {
    width: usize,
    height: usize,
    columns: usize,
    rows: usize,
}

#[derive(Debug, Deserialize)]
struct AnimationManifest {
    frames: Vec<i64>,
    fps: Option<f64>,
    #[serde(rename = "loop")]
    looping: Option<bool>,
    fallback: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    id: String,
    display_name: String,
    #[allow(dead_code)]
    description: Option<String>,
    sprite_version_number: Option<u32>,
    spritesheet_path: String,
    frame: Option<FrameManifest>,
    animations: Option<BTreeMap<String, AnimationManifest>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExtensionGrid {
    columns: usize,
    rows: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RoamlingManifest {
    schema_version: u32,
    spritesheet_path: Option<String>,
    frame: Option<ExtensionGrid>,
    #[serde(default)]
    behaviors: BTreeMap<String, String>,
    animations: Option<BTreeMap<String, AnimationManifest>>,
}

/// What the menu lists: enough to name a package without decoding its sheet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PetDescriptor {
    pub id: String,
    pub display_name: String,
    pub package: PathBuf,
    pub sprite_version_number: u32,
}

/// Where packages are looked for, in order.
///
/// `$ROAMLING_PET_PATH` first, then Roamling's own folder, then the agents'.
/// The agent folders are dotfiles under home on every platform because that is
/// where the agents themselves put them.
pub fn default_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(override_path) = std::env::var_os("ROAMLING_PET_PATH") {
        if !override_path.is_empty() {
            roots.push(PathBuf::from(override_path));
        }
    }
    if let Some(folder) = user_pet_folder() {
        roots.push(folder);
    }
    if let Some(home) = home() {
        roots.push(home.join(".codex").join("pets"));
        roots.push(home.join(".petdex").join("pets"));
    }
    roots
}

fn home() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

/// The folder Roamling tells the user to drop packages into: the first root it
/// searches that belongs to Roamling rather than to an agent.
pub fn user_pet_folder() -> Option<PathBuf> {
    if let Some(app_data) = std::env::var_os("APPDATA").filter(|value| !value.is_empty()) {
        return Some(PathBuf::from(app_data).join("Roamling").join("Pets"));
    }
    home().map(|home| home.join("Roamling").join("Pets"))
}

/// Every package under the roots, named, with duplicates dropped.
///
/// A root that is itself a package counts as one, which is what makes
/// `ROAMLING_PET_PATH` able to point straight at a package being worked on.
pub fn discover(roots: &[PathBuf]) -> Vec<PetDescriptor> {
    let mut seen_paths: Vec<PathBuf> = Vec::new();
    let mut seen_ids: Vec<String> = Vec::new();
    let mut found = Vec::new();

    for root in roots {
        let mut packages: Vec<PathBuf> = if root.join("pet.json").is_file() {
            vec![root.clone()]
        } else {
            let Ok(entries) = std::fs::read_dir(root) else {
                continue;
            };
            entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.is_dir())
                .collect()
        };
        packages.sort();

        for package in packages {
            if seen_paths.contains(&package) {
                continue;
            }
            seen_paths.push(package.clone());
            let Some(manifest) = read_manifest(&package).ok() else {
                continue;
            };
            if seen_ids.contains(&manifest.id) {
                continue;
            }
            seen_ids.push(manifest.id.clone());
            found.push(PetDescriptor {
                id: manifest.id,
                display_name: manifest.display_name,
                package,
                sprite_version_number: manifest.sprite_version_number.unwrap_or(1),
            });
        }
    }

    found.sort_by(|left, right| {
        left.display_name
            .to_lowercase()
            .cmp(&right.display_name.to_lowercase())
    });
    found
}

fn read_manifest(package: &Path) -> Result<Manifest, String> {
    let path = package.join("pet.json");
    let raw = std::fs::read(&path).map_err(|_| format!("missing pet.json at {}", path.display()))?;
    let manifest: Manifest =
        serde_json::from_slice(&raw).map_err(|error| format!("invalid pet.json: {error}"))?;
    if manifest.id.trim().is_empty() || manifest.display_name.trim().is_empty() {
        return Err("id and displayName must not be empty".to_string());
    }
    Ok(manifest)
}

struct Layout {
    frame_width: usize,
    frame_height: usize,
    columns: usize,
    rows: usize,
}

/// What a load produced, and what it had to ignore along the way.
///
/// A bad animation entry is reported and skipped rather than failing the load:
/// one unusable name should not cost the user the whole pet.
pub struct Loaded {
    pub asset: PetAsset,
    pub warnings: Vec<String>,
}

pub fn load(package: &Path) -> Result<Loaded, String> {
    let manifest = read_manifest(package)?;
    let sprite = safe_asset_path(&manifest.spritesheet_path, package)?;
    let atlas = decode(&sprite)?;
    let layout = resolve_layout(&manifest, &atlas)?;

    let mut warnings = Vec::new();
    let mut tracks = standard_tracks(layout.columns);
    let base_frames = layout.columns * layout.rows;
    install(
        manifest.animations.as_ref(),
        &mut tracks,
        base_frames,
        &mut warnings,
    );

    let mut behavior_mappings = BTreeMap::new();
    let mut extension_atlas = None;
    let mut extension_columns = 0;
    let mut extension_rows = 0;

    let extension_path = package.join("roamling.json");
    if extension_path.is_file() {
        match read_extension(&extension_path, package, &layout) {
            Ok(extension) => {
                behavior_mappings = extension.behaviors;
                extension_columns = extension.columns;
                extension_rows = extension.rows;
                extension_atlas = extension.atlas;
                // Installed after the package's own tracks, so an extension can
                // add what Petdex has no word for and correct what it does.
                install(
                    extension.animations.as_ref(),
                    &mut tracks,
                    base_frames + extension_columns * extension_rows,
                    &mut warnings,
                );
            }
            Err(error) => warnings.push(format!("Ignored roamling.json: {error}")),
        }
    }

    Ok(Loaded {
        asset: PetAsset {
            display_name: manifest.display_name,
            atlas,
            extension_atlas,
            frame_width: layout.frame_width,
            frame_height: layout.frame_height,
            columns: layout.columns,
            rows: layout.rows,
            extension_columns,
            extension_rows,
            tracks,
            behavior_mappings,
        },
        warnings,
    })
}

struct Extension {
    behaviors: BTreeMap<String, String>,
    animations: Option<BTreeMap<String, AnimationManifest>>,
    atlas: Option<PetImage>,
    columns: usize,
    rows: usize,
}

fn read_extension(path: &Path, package: &Path, layout: &Layout) -> Result<Extension, String> {
    let raw = std::fs::read(path).map_err(|error| error.to_string())?;
    let manifest: RoamlingManifest =
        serde_json::from_slice(&raw).map_err(|error| error.to_string())?;
    if manifest.schema_version != ROAMLING_SCHEMA_VERSION {
        return Err(format!(
            "unsupported roamling.json schemaVersion {}; expected {ROAMLING_SCHEMA_VERSION}",
            manifest.schema_version
        ));
    }

    let mut extension = Extension {
        behaviors: manifest.behaviors,
        animations: manifest.animations,
        atlas: None,
        columns: 0,
        rows: 0,
    };
    let Some(sheet_path) = manifest.spritesheet_path else {
        // A file that only remaps behaviours onto frames the package already
        // has needs no sheet of its own.
        return Ok(extension);
    };

    let grid = manifest
        .frame
        .filter(|grid| grid.columns > 0 && grid.rows > 0)
        .ok_or("extension sheet needs a frame grid")?;
    if grid.columns * grid.rows > MAXIMUM_FRAMES {
        return Err(format!(
            "extension grid is larger than {MAXIMUM_FRAMES} cells"
        ));
    }

    let sheet = decode(&safe_asset_path(&sheet_path, package)?)?;
    // The extension sheet shares the package's cell size, so its pixel size
    // follows from the grid rather than being declared again: a pet drawn at
    // two scales is not one pet.
    let expected = (
        grid.columns * layout.frame_width,
        grid.rows * layout.frame_height,
    );
    if (sheet.width, sheet.height) != expected {
        return Err(format!(
            "extension sheet is {}x{}, expected {}x{}",
            sheet.width, sheet.height, expected.0, expected.1
        ));
    }
    extension.columns = grid.columns;
    extension.rows = grid.rows;
    extension.atlas = Some(sheet);
    Ok(extension)
}

fn decode(path: &Path) -> Result<PetImage, String> {
    let size = std::fs::metadata(path)
        .map_err(|_| format!("missing spritesheet at {}", path.display()))?
        .len();
    if size > MAXIMUM_ENCODED_BYTES {
        return Err(format!("spritesheet is too large ({size} bytes)"));
    }
    let raw = std::fs::read(path).map_err(|error| error.to_string())?;
    PetImage::decode(&raw).ok_or_else(|| {
        format!(
            "could not decode the PNG/WebP spritesheet at {}",
            path.display()
        )
    })
}

fn install(
    animations: Option<&BTreeMap<String, AnimationManifest>>,
    tracks: &mut BTreeMap<String, PetAnimationTrack>,
    addressable: usize,
    warnings: &mut Vec<String>,
) {
    let Some(animations) = animations else { return };
    for (name, definition) in animations {
        if name.is_empty() {
            warnings.push("Ignored a custom animation with an empty name".to_string());
            continue;
        }
        let addressable = addressable as i64;
        if definition.frames.len() > MAXIMUM_FRAMES
            || !definition
                .frames
                .iter()
                .all(|index| *index >= 0 && *index < addressable)
        {
            warnings.push(format!(
                "Ignored animation '{name}' because a frame index is out of range"
            ));
            continue;
        }
        let fps = definition.fps.unwrap_or(12.0);
        if !(fps > 0.0 && fps <= 60.0) {
            warnings.push(format!(
                "Ignored animation '{name}' because fps must be in 0...60"
            ));
            continue;
        }
        let mut track = PetAnimationTrack::new(
            name,
            definition
                .frames
                .iter()
                .map(|index| PetAnimationFrame::new(*index as usize, 1.0 / fps))
                .collect(),
            definition.looping.unwrap_or(true),
        );
        track.fallback = definition.fallback.clone();
        tracks.insert(name.clone(), track);
    }
}

/// A path from the manifest, resolved inside the package and nowhere else.
///
/// The manifest is a file the user downloaded. An absolute path, a `~`, or a
/// `..` in it would let a package name any file on the machine, and the answer
/// is to refuse rather than to sanitise.
fn safe_asset_path(path: &str, package: &Path) -> Result<PathBuf, String> {
    let unsafe_path = || format!("spritesheetPath must stay inside the pet package: {path}");
    if path.is_empty() || path.starts_with('/') || path.starts_with('~') || path.starts_with('\\') {
        return Err(unsafe_path());
    }
    let relative = Path::new(path);
    // Windows adds two forms Unix does not have: `C:\...` is a prefix, and a
    // leading separator is a root. Both escape the package.
    if relative.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(unsafe_path());
    }
    let candidate = package.join(relative);
    // A symlink inside the package could still point out of it, so the check is
    // on the resolved path. An unresolvable path is a missing file, which the
    // caller reports.
    if let (Ok(root), Ok(resolved)) = (package.canonicalize(), candidate.canonicalize()) {
        if !resolved.starts_with(&root) {
            return Err(unsafe_path());
        }
    }
    Ok(candidate)
}

fn resolve_layout(manifest: &Manifest, atlas: &PetImage) -> Result<Layout, String> {
    let layout = match &manifest.frame {
        Some(frame) => {
            if frame.width == 0 || frame.height == 0 || frame.columns == 0 || frame.rows == 0 {
                return Err("custom frame values must be positive".to_string());
            }
            Layout {
                frame_width: frame.width,
                frame_height: frame.height,
                columns: frame.columns,
                rows: frame.rows,
            }
        }
        // v1 is the nine-row Petdex contract; v2 adds the two directional look
        // rows. Anything else is a package from a future we cannot read.
        None => match manifest.sprite_version_number.unwrap_or(1) {
            1 => Layout {
                frame_width: 192,
                frame_height: 208,
                columns: 8,
                rows: 9,
            },
            2 => Layout {
                frame_width: 192,
                frame_height: 208,
                columns: 8,
                rows: 11,
            },
            version => {
                return Err(format!(
                    "unsupported spriteVersionNumber {version}; expected 1 or 2"
                ))
            }
        },
    };

    if layout.columns * layout.rows > MAXIMUM_FRAMES {
        return Err(format!("atlas exceeds {MAXIMUM_FRAMES} frames"));
    }
    let expected = (
        layout.frame_width * layout.columns,
        layout.frame_height * layout.rows,
    );
    if (atlas.width, atlas.height) != expected {
        return Err(format!(
            "expected {}x{}, got {}x{}",
            expected.0, expected.1, atlas.width, atlas.height
        ));
    }
    if layout.rows < 9 || layout.columns < 8 {
        return Err("standard animations require at least an 8x9 grid".to_string());
    }
    Ok(layout)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The manifest is a file the user downloaded, so a path in it must not be
    /// able to name anything outside the package.
    #[test]
    fn a_path_cannot_escape_the_package() {
        let package = Path::new("C:/pets/mochi");
        for path in [
            "",
            "/etc/passwd",
            "~/secrets",
            "../../windows/system32/config/sam",
            "sub/../../out.webp",
            "C:/windows/notepad.exe",
            "\\\\server\\share\\thing.webp",
        ] {
            assert!(
                safe_asset_path(path, package).is_err(),
                "{path} was allowed"
            );
        }
        assert!(safe_asset_path("spritesheet.webp", package).is_ok());
        assert!(safe_asset_path("art/extra.webp", package).is_ok());
    }

    fn animations(json: &str) -> BTreeMap<String, AnimationManifest> {
        serde_json::from_str(json).expect("test json")
    }

    /// One unusable entry costs its own track and nothing more.
    #[test]
    fn a_bad_animation_is_skipped_rather_than_failing_the_pet() {
        let mut tracks = standard_tracks(8);
        let before = tracks.len();
        let mut warnings = Vec::new();
        install(
            Some(&animations(
                r#"{
                    "good": {"frames": [0, 1, 2], "fps": 8},
                    "past the end": {"frames": [0, 999]},
                    "negative": {"frames": [-1]},
                    "too fast": {"frames": [0], "fps": 120},
                    "stopped": {"frames": [0], "fps": 0}
                }"#,
            )),
            &mut tracks,
            72,
            &mut warnings,
        );
        assert_eq!(tracks.len(), before + 1, "only 'good' should have landed");
        assert_eq!(warnings.len(), 4);
        let good = &tracks["good"];
        assert_eq!(good.frames.len(), 3);
        assert!((good.frames[0].duration - 0.125).abs() < 1e-12, "fps 8");
        assert!(good.loops, "loop defaults to true");
    }

    /// The v1 and v2 grids are the Petdex contract, and the sheet has to match
    /// the one it declares.
    #[test]
    fn the_declared_layout_has_to_match_the_sheet() {
        let manifest = |version: u32| Manifest {
            id: "x".into(),
            display_name: "X".into(),
            description: None,
            sprite_version_number: Some(version),
            spritesheet_path: "spritesheet.webp".into(),
            frame: None,
            animations: None,
        };
        let sheet = |width: usize, height: usize| PetImage {
            width,
            height,
            pixels: Vec::new(),
        };

        assert!(resolve_layout(&manifest(1), &sheet(1536, 1872)).is_ok());
        assert!(resolve_layout(&manifest(2), &sheet(1536, 2288)).is_ok());
        assert!(resolve_layout(&manifest(1), &sheet(1536, 2288)).is_err());
        assert!(resolve_layout(&manifest(3), &sheet(1536, 1872)).is_err());
    }

    /// A whole package, written to a temp folder and read back.
    ///
    /// The sheets are the shipped mochi-v3 ones, which are byte for byte the
    /// files in `~/.codex/pets/mochi-v3` -- so this exercises the real contract
    /// rather than a stand-in: a v1 nine-row package with a `roamling.json`
    /// carrying an extension sheet, behaviour mappings and extra tracks.
    #[test]
    fn a_written_package_loads_back_whole() {
        let root = std::env::temp_dir().join(format!("roamling-pkg-{}", std::process::id()));
        let package = root.join("mochi-v3");
        std::fs::create_dir_all(&package).expect("temp dir");
        std::fs::write(package.join("spritesheet.webp"), crate::STANDARD).expect("sheet");
        std::fs::write(package.join("roamling.webp"), crate::EXTENSION).expect("extension");
        std::fs::write(
            package.join("pet.json"),
            br#"{
                "id": "mochi-v3",
                "displayName": "Mochi",
                "description": "test fixture",
                "spriteVersionNumber": 1,
                "spritesheetPath": "spritesheet.webp"
            }"#,
        )
        .expect("manifest");
        std::fs::write(
            package.join("roamling.json"),
            br#"{
                "schemaVersion": 1,
                "spritesheetPath": "roamling.webp",
                "frame": {"columns": 8, "rows": 3},
                "behaviors": {"sleep": "sleeping"},
                "animations": {
                    "sleeping": {"frames": [72, 73, 74], "fps": 1.5},
                    "reaches past the extension": {"frames": [96]}
                }
            }"#,
        )
        .expect("extension manifest");

        let found = discover(&[root.clone()]);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].display_name, "Mochi");
        assert_eq!(found[0].sprite_version_number, 1);

        let loaded = load(&package).expect("the package did not load");
        assert_eq!(loaded.asset.columns, 8);
        assert_eq!(loaded.asset.rows, 9);
        assert_eq!(loaded.asset.extension_rows, 3);
        assert_eq!(
            loaded.asset.behavior_mappings.get("sleep").map(String::as_str),
            Some("sleeping")
        );
        // The extension track landed, and its frames address the second sheet
        // by continuing past the end of the package grid.
        let sleeping = &loaded.asset.tracks["sleeping"];
        assert_eq!(sleeping.frames.len(), 3);
        let rect = loaded
            .asset
            .frame_rect(sleeping.frames[0].index)
            .expect("frame 72 is the extension sheet's first cell");
        assert!(matches!(rect.sheet, crate::Sheet::Extension));
        assert_eq!((rect.x, rect.y), (0, 0));
        // 96 is past 72 + 24, so it is out of range and only costs its own
        // track -- the pet still loaded.
        assert!(!loaded.asset.tracks.contains_key("reaches past the extension"));
        assert_eq!(loaded.warnings.len(), 1, "{:?}", loaded.warnings);

        let _ = std::fs::remove_dir_all(&root);
    }

    /// The shipped mascot's own package, if it is on this machine. Skipped
    /// rather than failed when it is not: not every checkout has one.
    #[test]
    fn a_real_package_loads_if_one_is_installed() {
        let roots = default_roots();
        let found = discover(&roots);
        let Some(descriptor) = found.first() else {
            return;
        };
        let loaded = load(&descriptor.package)
            .unwrap_or_else(|error| panic!("{} failed: {error}", descriptor.package.display()));
        assert!(!loaded.asset.tracks.is_empty());
        assert_eq!(loaded.asset.display_name, descriptor.display_name);
        assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    }
}
