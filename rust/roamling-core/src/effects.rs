// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Small, bounded visual particles. No platform types and no behavior RNG.
use crate::geometry::WorldPoint;

const MAX_PARTICLES: usize = 6;

/// A filled polygon in pet-width units relative to the pet's centre.
/// Renderers only fill polygons; adding a shape does not add platform logic.
#[derive(Debug, Clone, PartialEq)]
pub struct EffectFrame {
    pub points: Vec<WorldPoint>,
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub opacity: f64,
}

#[derive(Debug, Clone, Copy)]
enum Shape { Heart }

#[derive(Debug, Clone, Copy)]
struct EffectDefinition {
    shape: Shape,
    colour: [u8; 3],
    lifetime: f64,
    start_size: f64,
    end_size: f64,
    rise_speed: f64,
}

const HEART: EffectDefinition = EffectDefinition {
    shape: Shape::Heart,
    colour: [244, 112, 151],
    lifetime: 1.2,
    start_size: 0.09,
    end_size: 0.16,
    rise_speed: 0.60,
};

#[derive(Debug, Clone)]
struct Particle {
    definition: EffectDefinition,
    age: f64,
    x: f64,
    drift: f64,
}

#[derive(Debug, Clone, Default)]
pub struct EffectSystem {
    particles: Vec<Particle>,
    touching: bool,
    until_next: f64,
    sequence: usize,
}

impl EffectSystem {
    pub fn clear(&mut self) {
        self.particles.clear();
        self.touching = false;
        self.until_next = 0.0;
    }

    pub fn is_active(&self) -> bool { !self.particles.is_empty() }

    /// Motion is normalized stroke intensity, not cursor distance.
    pub fn update(&mut self, dt: f64, petting: bool, motion: f64) {
        if !dt.is_finite() || dt < 0.0 { return; }
        let dt = dt.min(0.1);
        for particle in &mut self.particles { particle.age += dt; }
        self.particles.retain(|p| p.age < p.definition.lifetime);
        if !petting {
            self.touching = false;
            self.until_next = 0.0;
            return;
        }
        if !self.touching {
            self.touching = true;
            self.until_next = 0.35;
            return;
        }
        self.until_next -= dt;
        if self.until_next <= 1e-9 {
            self.emit(HEART);
            let strength = if motion.is_finite() { motion.clamp(0.0, 1.0) } else { 0.0 };
            self.until_next += 0.7 + (0.2 - 0.7) * strength;
        }
    }

    fn emit(&mut self, definition: EffectDefinition) {
        if self.particles.len() >= MAX_PARTICLES { return; }
        let phase = self.sequence % 5;
        self.sequence = self.sequence.wrapping_add(1);
        let x = [-0.16, 0.13, -0.04, 0.20, -0.21][phase];
        self.particles.push(Particle {
            definition, age: 0.0, x,
            drift: if phase % 2 == 0 { -0.04 } else { 0.04 },
        });
    }

    pub fn frames(&self) -> Vec<EffectFrame> {
        self.particles.iter().map(|particle| {
            let d = particle.definition;
            let progress = (particle.age / d.lifetime).clamp(0.0, 1.0);
            let size = d.start_size + (d.end_size - d.start_size) * progress;
            let x = particle.x + particle.drift * particle.age;
            let y = -0.30 - d.rise_speed * particle.age;
            let points = match d.shape {
                Shape::Heart => (0..48).map(|i| {
                    let angle = std::f64::consts::TAU * i as f64 / 48.0;
                    let px = angle.sin().powi(3) / 2.0;
                    let py = -(13.0 * angle.cos() - 5.0 * (2.0 * angle).cos()
                        - 2.0 * (3.0 * angle).cos() - (4.0 * angle).cos()) / 32.0;
                    WorldPoint::new(x + px * size, y + py * size)
                }).collect(),
            };
            EffectFrame {
                points, red: d.colour[0], green: d.colour[1], blue: d.colour[2],
                opacity: (particle.age / 0.12).min(1.0) * (1.0 - progress).min(0.6) / 0.6,
            }
        }).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stroking_emits_more_but_particles_stay_bounded_and_expire() {
        for hz in [30, 60] {
            let mut slow = EffectSystem::default();
            let mut fast = EffectSystem::default();
            for _ in 0..hz * 5 {
                slow.update(1.0 / hz as f64, true, 0.0);
                fast.update(1.0 / hz as f64, true, 1.0);
                assert!(fast.frames().len() <= MAX_PARTICLES);
                for frame in fast.frames() {
                    assert!((0.0..=1.0).contains(&frame.opacity));
                    assert!(frame.points.iter().all(|p| p.x.abs() < 1.0 && p.y > -1.5 && p.y < 0.5));
                }
            }
            assert!(fast.sequence > slow.sequence * 2);
            for _ in 0..hz * 2 { fast.update(1.0 / hz as f64, false, 0.0); }
            assert!(!fast.is_active());
            slow.clear();
            assert!(!slow.is_active());
        }
    }

    #[test]
    fn brief_contact_and_non_pet_activity_have_no_hearts() {
        let mut effects = EffectSystem::default();
        for _ in 0..120 { effects.update(1.0 / 30.0, false, 1.0); }
        assert!(!effects.is_active());
        for _ in 0..6 { effects.update(1.0 / 30.0, true, 1.0); }
        assert!(!effects.is_active());
        effects.update(1.0 / 30.0, false, 0.0);
        effects.update(1.0 / 30.0, true, 0.0);
        assert!(!effects.is_active());
    }
}
