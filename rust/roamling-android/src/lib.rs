// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! One native library contains both UniFFI components. Android owns the OS;
//! the existing PetLoop continues to own all behavior decisions.

use roamling_core::ffi::{normalize_tuning, FfiPetImage, FfiTuning};
use roamling_core::{AnimationResolver, PetAnimationPlayer, RuntimeTuning, PET_CAPABILITIES};
use roamling_pet::{built_in_mochi, PetAsset, Sheet};
use std::sync::{Arc, Mutex};

uniffi::setup_scaffolding!();
roamling_core::uniffi_reexport_scaffolding!();

/// Fetch defaults from the core instead of maintaining a Kotlin copy.
#[uniffi::export]
pub fn default_tuning() -> FfiTuning {
    let tuning = RuntimeTuning::default();
    normalize_tuning(
        tuning.walking_speed,
        tuning.wander_pause,
        tuning.cross_display_wander_chance,
        tuning.pointer_awareness_distance,
        tuning.catch_arm_distance,
        tuning.catch_approach_speed,
        tuning.catch_window,
        tuning.hit_region_scale,
        tuning.gait_cadence,
        tuning.evade_speed_scale,
        tuning.idle_before_rest,
    )
}

/// The decoder already premultiplies RGBA; Kotlin must not multiply it again.
#[derive(uniffi::Object)]
pub struct MascotAtlas {
    asset: PetAsset,
}

#[uniffi::export]
pub fn load_mochi() -> Option<Arc<MascotAtlas>> {
    built_in_mochi().map(|asset| Arc::new(MascotAtlas { asset }))
}

#[derive(Debug, PartialEq, uniffi::Record)]
pub struct AtlasFrame {
    pub extension: bool,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[uniffi::export]
impl MascotAtlas {
    pub fn image(&self, extension: bool) -> Option<FfiPetImage> {
        let sheet = if extension {
            Sheet::Extension
        } else {
            Sheet::Package
        };
        self.asset.sheet(sheet).map(|image| FfiPetImage {
            width: image.width as u32,
            height: image.height as u32,
            pixels: image.pixels.clone(),
        })
    }

    pub fn frame(&self, index: u32) -> Option<AtlasFrame> {
        self.asset
            .frame_rect(index as usize)
            .map(|rect| AtlasFrame {
                extension: rect.sheet == Sheet::Extension,
                x: rect.x as u32,
                y: rect.y as u32,
                width: rect.width as u32,
                height: rect.height as u32,
            })
    }
}

#[derive(uniffi::Object)]
pub struct Player {
    atlas: Arc<MascotAtlas>,
    resolver: AnimationResolver,
    inner: Mutex<PetAnimationPlayer>,
}

#[uniffi::export]
impl Player {
    #[uniffi::constructor]
    pub fn new(atlas: Arc<MascotAtlas>) -> Arc<Self> {
        let resolver = AnimationResolver::new(
            atlas.asset.tracks.clone(),
            atlas.asset.behavior_mappings.clone(),
        );
        let player = PetAnimationPlayer::new(&resolver);
        Arc::new(Self {
            atlas,
            resolver,
            inner: Mutex::new(player),
        })
    }

    /// Same capability wire order as FfiTickOutput. Invalid input changes nothing.
    pub fn advance(&self, capability: u8, delta_time: f64) -> Option<AtlasFrame> {
        let capability = *PET_CAPABILITIES.get(capability as usize)?;
        if !delta_time.is_finite() || delta_time < 0.0 {
            return None;
        }
        let mut player = self.inner.lock().unwrap();
        player.set_capability(&self.resolver, capability);
        player.update(delta_time);
        self.atlas.frame(player.current_frame_index() as u32)
    }

    /// Let the shared catch lifecycle use the resolved asset's real timing.
    pub fn duration(&self, capability: u8) -> f64 {
        PET_CAPABILITIES
            .get(capability as usize)
            .and_then(|capability| self.resolver.resolve(*capability))
            .map_or(0.0, |track| track.frames.iter().map(|frame| frame.duration).sum())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_capabilities_follow_the_existing_player_on_both_sheets() {
        let atlas = load_mochi().unwrap();
        let bridge = Player::new(atlas.clone());
        let resolver = AnimationResolver::new(
            atlas.asset.tracks.clone(),
            atlas.asset.behavior_mappings.clone(),
        );
        let mut reference = PetAnimationPlayer::new(&resolver);
        let mut saw_extension = false;
        for (wire, capability) in PET_CAPABILITIES.iter().enumerate() {
            reference.set_capability(&resolver, *capability);
            for delta in [0.0, 0.05, 0.1, 0.3, 1.2, 0.2] {
                reference.update(delta);
                let actual = bridge.advance(wire as u8, delta).unwrap();
                assert_eq!(
                    Some(actual),
                    atlas.frame(reference.current_frame_index() as u32)
                );
                let rect = atlas.frame(reference.current_frame_index() as u32).unwrap();
                saw_extension |= rect.extension;
                let image = atlas.image(rect.extension).unwrap();
                assert!(rect.x + rect.width <= image.width);
                assert!(rect.y + rect.height <= image.height);
            }
        }
        assert!(saw_extension);
    }

    #[test]
    fn invalid_wire_input_does_not_poison_the_next_frame() {
        let atlas = load_mochi().unwrap();
        let player = Player::new(atlas.clone());
        let before = player.advance(0, 0.0);
        assert!(player.advance(255, 0.1).is_none());
        assert!(player.advance(0, f64::NAN).is_none());
        assert!(player.advance(0, f64::INFINITY).is_none());
        assert!(player.advance(0, -1.0).is_none());
        assert_eq!(before, player.advance(0, 0.0));
        assert!(atlas.frame(u32::MAX).is_none());
        for extension in [false, true] {
            let image = atlas.image(extension).unwrap();
            assert_eq!(
                image.pixels.len(),
                (image.width * image.height * 4) as usize
            );
            assert!(image
                .pixels
                .chunks_exact(4)
                .all(|p| p[..3].iter().all(|c| *c <= p[3])));
        }
    }
}
