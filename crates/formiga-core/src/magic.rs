//! Magic fruit: what eating one does to a companion for a little while, and how that is drawn.
//!
//! Now and then — once every few days of a colony being out on the desktop — what a companion sits
//! down to eat turns out to be a star fruit, glowing gold. Once it has eaten it, something odd
//! happens to it for a minute or two: it floats a hand's breadth off the ground, every step it
//! takes is a bounce, it can hardly walk for running, or it shimmers with little lights. Which one
//! leans on who it is. Then it wears off, and it is itself again.
//!
//! A [`Spell`] is runtime only, like a [`crate::Presence`]: nothing here is saved, counted or
//! shown anywhere but on the companion itself, and a relaunch simply finds everybody as they were.
//! Everything a spell does to how a companion is drawn is worked out here, as plain numbers from
//! the spell's own clock, so the overlay, its click proxy and the review sheets agree, and none of
//! it can leave a companion anywhere the simulation did not put it: a floating companion's feet
//! are where they always were, only drawn higher, and drawn back down on them whenever anything
//! else wants its body.

use crate::tuning::MAGIC;

/// What a magic fruit does to whoever eats it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MagicKind {
    /// Floats a hand's breadth off the ground, bobbing gently, with a little gold dust falling
    /// from its feet.
    Float,
    /// Every step is a bounce, and standing still it hops on the spot.
    Bounce,
    /// So full of beans it runs wherever it goes, trailing sparks.
    Zoomies,
    /// Little lights twinkle all about it.
    Shimmer,
}

impl MagicKind {
    pub const ALL: [Self; 4] = [Self::Float, Self::Bounce, Self::Zoomies, Self::Shimmer];
}

/// One little light drawn about a companion under a spell, in art pixels from the point it
/// stands on: `x` to the right, `y` up the screen as a negative number, as the frame is laid out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glint {
    pub x: f32,
    pub y: f32,
    /// 0 a speck, 1 a small star, 2 a bright one.
    pub size: u8,
}

/// The most lights one companion shows at once.
pub const MAX_GLINTS: usize = 4;

/// A spell a companion is under.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spell {
    pub kind: MagicKind,
    /// Seconds since it began.
    pub elapsed: f32,
    /// Seconds it lasts before it starts to wear off.
    pub length: f32,
    /// How much of it shows just now, from nothing to all of it: it comes on over a moment, goes
    /// to nothing whenever something else wants the companion's body — a scene, a climb, a hand —
    /// and back again afterwards, and to nothing as it wears off.
    pub strength: f32,
    /// The still, gentle way of it, for a person who has asked for reduced motion: a few lights
    /// that stay where they are, and nothing else.
    pub still: bool,
    /// How far above its feet the companion is drawn this tick, in art pixels, worked out by the
    /// world at the end of every tick.
    pub lift: f32,
    /// Its own little offset for the lights, so two spells never twinkle in step.
    pub phase: f32,
}

impl Spell {
    pub fn new(kind: MagicKind, length: f32, phase: f32, still: bool) -> Self {
        Self {
            kind: if still { MagicKind::Shimmer } else { kind },
            elapsed: 0.0,
            length: length.max(1.0),
            strength: 0.0,
            still,
            lift: 0.0,
            phase,
        }
    }

    /// Whether it has run its length and is wearing off.
    pub fn wearing_off(&self) -> bool {
        self.elapsed >= self.length
    }

    /// Whether it has worn off altogether.
    pub fn over(&self) -> bool {
        self.elapsed >= self.length + MAGIC.wear_off_secs
    }

    /// How much of it there would be now, from its own clock alone: coming on over its first
    /// moment and going over its last.
    pub fn envelope(&self) -> f32 {
        let on = (self.elapsed / MAGIC.come_on_secs).clamp(0.0, 1.0);
        let off = 1.0 - ((self.elapsed - self.length) / MAGIC.wear_off_secs).clamp(0.0, 1.0);
        on.min(off)
    }

    /// Whether anything about it moves, so the overlay has to draw it as often as a walk.
    pub fn moving(&self) -> bool {
        !self.still
    }

    /// How much quicker it walks under the spell: only zoomies hurry it.
    pub fn pace(&self) -> f32 {
        match self.kind {
            MagicKind::Zoomies if !self.still => 1.0 + (MAGIC.zoomies_pace - 1.0) * self.strength,
            _ => 1.0,
        }
    }

    /// How far above its feet the companion is drawn, in art pixels, before `strength`: floating,
    /// or partway through a bounce. `stepping` and `stride` are its presence's: whether its feet
    /// are carrying it, and how many walking frames they have gone through.
    pub fn shape_lift(&self, stepping: bool, stride: f32) -> f32 {
        if self.still {
            return 0.0;
        }
        match self.kind {
            MagicKind::Float => {
                let bob = (self.elapsed / MAGIC.float_bob_secs * std::f32::consts::TAU).sin();
                MAGIC.float_height + MAGIC.float_bob * bob
            }
            MagicKind::Bounce => {
                let phase = self.bounce_phase(stepping, stride);
                if phase < 1.0 {
                    MAGIC.bounce_height * (phase * std::f32::consts::PI).sin()
                } else {
                    0.0
                }
            }
            MagicKind::Zoomies | MagicKind::Shimmer => 0.0,
        }
    }

    /// Where it is in a bounce: 0 to 1 in the air, and above 1 on the ground, counting up to the
    /// next hop. Walking, it hops once every two walking frames; standing, every
    /// `bounce_rest_secs`.
    fn bounce_phase(&self, stepping: bool, stride: f32) -> f32 {
        if stepping {
            // Two frames in the air out of every three, so a walk is mostly hopping.
            let cycle = (stride / 3.0).fract() * 3.0;
            cycle / 2.0
        } else {
            let t = (self.elapsed + self.phase) % MAGIC.bounce_rest_secs;
            t / MAGIC.bounce_air_secs
        }
    }

    /// The lights drawn about it this tick. `facing_right` is the way it is facing, for a trail
    /// that streams out behind it.
    pub fn glints(&self, stepping: bool, stride: f32, facing_right: bool) -> [Option<Glint>; 4] {
        let mut out = [None; MAX_GLINTS];
        let t = self.elapsed + self.phase;
        // Body middle, about half a companion up.
        let mid = -13.0;
        if self.still {
            // Three lights that stay put, about the head and shoulders.
            let fixed = [
                (-11.0, mid - 7.0, 1),
                (12.0, mid - 2.0, 1),
                (-7.0, mid + 8.0, 0),
            ];
            for (slot, (x, y, size)) in out.iter_mut().zip(fixed) {
                *slot = (self.envelope() > 0.3).then_some(Glint { x, y, size });
            }
            return out;
        }
        // The moment it comes on: a ring of lights bursting outward.
        if self.elapsed < MAGIC.burst_secs {
            let p = self.elapsed / MAGIC.burst_secs;
            let radius = 5.0 + 15.0 * p;
            let size = if p < 0.4 {
                2
            } else if p < 0.75 {
                1
            } else {
                0
            };
            for (i, slot) in out.iter_mut().enumerate() {
                let angle = std::f32::consts::FRAC_PI_4 + i as f32 * std::f32::consts::FRAC_PI_2;
                *slot = Some(Glint {
                    x: angle.cos() * radius,
                    y: mid + angle.sin() * radius * 0.85,
                    size,
                });
            }
            return out;
        }
        // Wearing off: a last few lights drifting up and fading.
        if self.wearing_off() {
            let p = ((self.elapsed - self.length) / MAGIC.wear_off_secs).clamp(0.0, 1.0);
            let size = if p < 0.35 {
                1
            } else if p < 0.8 {
                0
            } else {
                return out;
            };
            for (i, slot) in out.iter_mut().take(3).enumerate() {
                let x = [-9.0, 2.0, 10.0][i];
                *slot = Some(Glint {
                    x,
                    y: mid - 6.0 - p * 14.0 - i as f32 * 3.0 - self.lift,
                    size,
                });
            }
            return out;
        }
        if self.strength < 0.35 {
            return out;
        }
        let twinkle = |offset: f32, period: f32| -> u8 {
            let p = ((t / period + offset).fract() * 4.0) as u8;
            [0, 1, 2, 1][usize::from(p.min(3))]
        };
        match self.kind {
            MagicKind::Shimmer => {
                // Four lights round it on a slow turn, each twinkling in its own time.
                let turn = t * MAGIC.shimmer_turns_per_sec * std::f32::consts::TAU;
                for (i, slot) in out.iter_mut().enumerate() {
                    let angle = turn + i as f32 * std::f32::consts::FRAC_PI_2;
                    *slot = Some(Glint {
                        x: angle.cos() * 17.0,
                        y: mid + angle.sin() * 13.0,
                        size: twinkle(i as f32 * 0.31, 1.1),
                    });
                }
            }
            MagicKind::Float => {
                // Gold dust falling from its feet to the ground, two specks at a time.
                for (i, slot) in out.iter_mut().take(2).enumerate() {
                    let p = (t / 1.3 + i as f32 * 0.5).fract();
                    *slot = Some(Glint {
                        x: [-5.0, 6.0][i] + (p * 9.0).sin() * 1.5,
                        y: -self.lift * (1.0 - p),
                        size: if p < 0.5 { 1 } else { 0 },
                    });
                }
                // And one light up by its head, now and then.
                out[2] = (twinkle(0.2, 2.4) > 0).then_some(Glint {
                    x: 12.0,
                    y: mid - 9.0 - self.lift,
                    size: twinkle(0.2, 2.4),
                });
            }
            MagicKind::Bounce => {
                // A little puff either side each time it comes down.
                let phase = self.bounce_phase(stepping, stride);
                let since = phase - 1.0;
                if (0.0..0.5).contains(&since) {
                    let spread = 6.0 + since * 14.0;
                    let size = if since < 0.25 { 1 } else { 0 };
                    out[0] = Some(Glint {
                        x: -spread,
                        y: -1.0 - since * 4.0,
                        size,
                    });
                    out[1] = Some(Glint {
                        x: spread,
                        y: -1.0 - since * 4.0,
                        size,
                    });
                }
                out[2] = (twinkle(0.6, 1.7) == 2).then_some(Glint {
                    x: -10.0,
                    y: mid - 8.0 - self.lift,
                    size: 1,
                });
            }
            MagicKind::Zoomies => {
                // Sparks streaming out behind it.
                let behind = if facing_right { -1.0 } else { 1.0 };
                for (i, slot) in out.iter_mut().take(3).enumerate() {
                    let p = (t * 2.2 + i as f32 / 3.0).fract();
                    *slot = Some(Glint {
                        x: behind * (9.0 + p * 16.0),
                        y: -6.0 - ((i as f32 * 2.3 + t * 3.0).sin() + 1.0) * 4.0,
                        size: if p < 0.3 {
                            2
                        } else if p < 0.7 {
                            1
                        } else {
                            0
                        },
                    });
                }
            }
        }
        out
    }
}

/// The weights a companion's temperament puts on each spell: a calm one tends to float, a playful
/// one to bounce, a lively one to run, and a vain or shy one to shimmer. Everyone can have any of
/// them.
pub fn leanings(personality: &crate::PersonalityGenome, traits: &[crate::Trait; 3]) -> [f32; 4] {
    use crate::Trait::*;
    let has = |any: &[crate::Trait]| traits.iter().any(|t| any.contains(t));
    let calm = 1.0 - personality.activity;
    let mut float = 1.0 + calm * 2.0;
    let mut bounce = 1.0 + personality.playfulness * 2.0;
    let mut zoomies = 1.0 + personality.activity * 2.0;
    let mut shimmer = 1.0;
    if has(&[Mellow, Sleepy, Gentle, Easygoing, Patient, Steady, Lazy]) {
        float += 2.0;
    }
    if has(&[Playful, Silly, Excitable, Spontaneous, Mischievous]) {
        bounce += 2.0;
    }
    if has(&[
        Energetic,
        Restless,
        Competitive,
        Impatient,
        Reckless,
        Impulsive,
    ]) {
        zoomies += 2.0;
    }
    if has(&[
        Vain,
        AttentionSeeking,
        Dramatic,
        Shy,
        EasilyEmbarrassed,
        Eccentric,
    ]) {
        shimmer += 2.5;
    }
    [float, bounce, zoomies, shimmer]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_spell_comes_on_and_wears_off_to_nothing() {
        for kind in MagicKind::ALL {
            let mut spell = Spell::new(kind, 30.0, 0.0, false);
            assert_eq!(spell.envelope(), 0.0);
            spell.elapsed = 10.0;
            assert_eq!(spell.envelope(), 1.0);
            spell.elapsed = 30.0 + MAGIC.wear_off_secs;
            assert_eq!(spell.envelope(), 0.0);
            assert!(spell.over());
        }
    }

    #[test]
    fn the_still_way_never_lifts_or_hurries() {
        let mut spell = Spell::new(MagicKind::Float, 30.0, 0.0, true);
        assert_eq!(spell.kind, MagicKind::Shimmer);
        spell.strength = 1.0;
        for step in 0..200 {
            spell.elapsed = step as f32 * 0.1;
            assert_eq!(spell.shape_lift(true, step as f32), 0.0);
            assert_eq!(spell.pace(), 1.0);
            let first = spell.glints(false, 0.0, true);
            spell.elapsed += 0.05;
            if spell.elapsed > 2.0 && spell.elapsed < 29.0 {
                assert_eq!(
                    first,
                    spell.glints(false, 0.0, true),
                    "still lights do not move"
                );
            }
        }
    }

    #[test]
    fn floating_stays_off_the_ground_and_bouncing_comes_back_to_it() {
        let mut float = Spell::new(MagicKind::Float, 30.0, 0.0, false);
        let mut bounce = Spell::new(MagicKind::Bounce, 30.0, 0.0, false);
        let mut touched = false;
        for step in 0..300 {
            float.elapsed = step as f32 * 0.05;
            bounce.elapsed = step as f32 * 0.05;
            assert!(float.shape_lift(false, 0.0) >= MAGIC.float_height - MAGIC.float_bob - 0.01);
            let lift = bounce.shape_lift(false, 0.0);
            assert!((0.0..=MAGIC.bounce_height + 0.01).contains(&lift));
            touched |= lift == 0.0;
        }
        assert!(touched);
    }
}
