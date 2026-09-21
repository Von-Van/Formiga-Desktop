use crate::Rgba;

#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub outline: Rgba,
    pub shadow: Rgba,
    pub coat: Rgba,
    pub highlight: Rgba,
    pub accent: Rgba,
    pub eye: Rgba,
}

/// Image colors are softened once at atlas construction, with fixed dark facial contrast.
pub fn palette_for(genome: &formiga_core::AppearanceGenome) -> Palette {
    let Some(design) = genome.design else {
        return PALETTES[genome.palette_index as usize % PALETTES.len()];
    };
    if design.classic.coat > 0 {
        return candy_palette(design.coat, design.accent);
    }
    let soften = |rgb: [u8; 3]| rgb.map(|c| ((u16::from(c) * 3 + 220) / 4) as u8);
    let rgb = soften(design.coat);
    let rgba = |v: [u8; 3]| Rgba::new(v[0], v[1], v[2], 255);
    Palette {
        outline: c(0x302b3b),
        shadow: rgba(rgb.map(|c| (u16::from(c) * 3 / 4) as u8)),
        coat: rgba(rgb),
        highlight: rgba(rgb.map(|c| ((u16::from(c) + 255) / 2) as u8)),
        accent: rgba(soften(design.accent)),
        eye: c(0x201b29),
    }
}

/// Candy colours, built the way the original hand-made palettes are: the coat and accent at full
/// strength, a shade that keeps the coat's hue but drops most of its saturation, a bright tinted
/// highlight, and an outline and eyes that are near-black tinted toward the coat instead of one
/// fixed ink. Lightness is held inside the band those palettes use, so a dark coat from a reference
/// image still leaves the eyes readable against it.
fn candy_palette(coat: [u8; 3], accent: [u8; 3]) -> Palette {
    let rgba = |value: [u8; 3]| Rgba::new(value[0], value[1], value[2], 255);
    let (hue, saturation, lightness) = to_hsl(rgba(coat));
    let lightness = lightness.clamp(0.55, 0.78);
    let (accent_hue, accent_saturation, accent_lightness) = to_hsl(rgba(accent));
    let ink = (saturation * 0.4).min(0.3);
    Palette {
        outline: rgba(from_hsl(hue, ink, 0.19)),
        shadow: rgba(from_hsl(
            hue,
            (saturation * 0.5).min(0.45),
            (lightness - 0.24).clamp(0.36, 0.52),
        )),
        coat: rgba(from_hsl(hue, saturation, lightness)),
        highlight: rgba(from_hsl(hue, (saturation * 1.4 + 0.15).min(1.0), 0.86)),
        accent: rgba(from_hsl(
            accent_hue,
            accent_saturation,
            accent_lightness.clamp(0.62, 0.82),
        )),
        eye: rgba(from_hsl(hue, ink, 0.11)),
    }
}

/// Toys, snacks, drinkware, and found trinkets read as separate belongings rather than
/// another patch of coat. The item hue is rotated away from the creature's own, saturation is
/// lifted, and lightness is pushed to whichever side has more room, so a prop stays legible
/// against the body carrying it. The dark outline is shared so the pixel art still matches.
pub fn prop_palette(palette: Palette, seed: u64) -> Palette {
    let (hue, _, lightness) = to_hsl(palette.coat);
    // A near-complementary rotation, nudged per creature so props are not all one hue.
    let spin = 150.0 + (seed % 61) as f32;
    let hue = (hue + spin) % 360.0;
    let saturation = 0.62 + ((seed >> 8) % 3) as f32 * 0.06;
    // Separate the item from the coat it is held against, in the roomier direction.
    let target = if lightness > 0.5 {
        (lightness - 0.30).max(0.34)
    } else {
        (lightness + 0.30).min(0.78)
    };
    let rgba = |value: [u8; 3]| Rgba::new(value[0], value[1], value[2], 255);
    Palette {
        outline: palette.outline,
        shadow: rgba(from_hsl(hue, saturation, (target - 0.16).max(0.12))),
        coat: rgba(from_hsl(hue, saturation, target)),
        highlight: rgba(from_hsl(hue, saturation * 0.7, (target + 0.22).min(0.92))),
        accent: rgba(from_hsl(hue, saturation, target)),
        eye: palette.eye,
    }
}

pub(crate) fn to_hsl(color: Rgba) -> (f32, f32, f32) {
    let (r, g, b) = (
        f32::from(color.r) / 255.0,
        f32::from(color.g) / 255.0,
        f32::from(color.b) / 255.0,
    );
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let lightness = (max + min) / 2.0;
    let delta = max - min;
    if delta <= f32::EPSILON {
        return (0.0, 0.0, lightness);
    }
    let saturation = delta / (1.0 - (2.0 * lightness - 1.0).abs()).max(f32::EPSILON);
    let hue = if max == r {
        60.0 * (((g - b) / delta) % 6.0)
    } else if max == g {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    };
    ((hue + 360.0) % 360.0, saturation.clamp(0.0, 1.0), lightness)
}

pub(crate) fn from_hsl(hue: f32, saturation: f32, lightness: f32) -> [u8; 3] {
    let saturation = saturation.clamp(0.0, 1.0);
    let lightness = lightness.clamp(0.0, 1.0);
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let section = (hue % 360.0) / 60.0;
    let second = chroma * (1.0 - (section % 2.0 - 1.0).abs());
    let (r, g, b) = match section as u32 {
        0 => (chroma, second, 0.0),
        1 => (second, chroma, 0.0),
        2 => (0.0, chroma, second),
        3 => (0.0, second, chroma),
        4 => (second, 0.0, chroma),
        _ => (chroma, 0.0, second),
    };
    let base = lightness - chroma / 2.0;
    [r, g, b].map(|channel| ((channel + base).clamp(0.0, 1.0) * 255.0).round() as u8)
}

const fn c(hex: u32) -> Rgba {
    Rgba::new((hex >> 16) as u8, (hex >> 8) as u8, hex as u8, 255)
}

pub const PALETTES: [Palette; 12] = [
    Palette {
        outline: c(0x30243b),
        shadow: c(0x8d5b7b),
        coat: c(0xe98ab5),
        highlight: c(0xffc7dd),
        accent: c(0xffe081),
        eye: c(0x201827),
    },
    Palette {
        outline: c(0x24313b),
        shadow: c(0x4a7d8c),
        coat: c(0x75c9c8),
        highlight: c(0xc7f3e8),
        accent: c(0xffcf70),
        eye: c(0x172029),
    },
    Palette {
        outline: c(0x3a2d24),
        shadow: c(0xb56f45),
        coat: c(0xf2a65a),
        highlight: c(0xffd59a),
        accent: c(0x7dcfb6),
        eye: c(0x211914),
    },
    Palette {
        outline: c(0x28243d),
        shadow: c(0x6557a5),
        coat: c(0x9b8de3),
        highlight: c(0xd9d1ff),
        accent: c(0xff9f9f),
        eye: c(0x171426),
    },
    Palette {
        outline: c(0x283427),
        shadow: c(0x688f4e),
        coat: c(0xa7cb65),
        highlight: c(0xe1efaa),
        accent: c(0xf3a86b),
        eye: c(0x182017),
    },
    Palette {
        outline: c(0x3b2927),
        shadow: c(0x9c5d5a),
        coat: c(0xe8837b),
        highlight: c(0xffc1ae),
        accent: c(0x7ec8e3),
        eye: c(0x251817),
    },
    Palette {
        outline: c(0x233342),
        shadow: c(0x477fa8),
        coat: c(0x65b5df),
        highlight: c(0xbde8f6),
        accent: c(0xf5d76e),
        eye: c(0x14202a),
    },
    Palette {
        outline: c(0x35283d),
        shadow: c(0x87549d),
        coat: c(0xca78d1),
        highlight: c(0xf0b9ed),
        accent: c(0x84d6b4),
        eye: c(0x201726),
    },
    Palette {
        outline: c(0x3c3520),
        shadow: c(0xa38b42),
        coat: c(0xe0c65a),
        highlight: c(0xffeda0),
        accent: c(0xef7d77),
        eye: c(0x252014),
    },
    Palette {
        outline: c(0x2d3033),
        shadow: c(0x737b80),
        coat: c(0xaab3b7),
        highlight: c(0xe5ecec),
        accent: c(0xf49d6e),
        eye: c(0x1b1d1e),
    },
    Palette {
        outline: c(0x3b2a30),
        shadow: c(0x9c586b),
        coat: c(0xda7894),
        highlight: c(0xffb8c7),
        accent: c(0x94d3ac),
        eye: c(0x23181c),
    },
    Palette {
        outline: c(0x26352f),
        shadow: c(0x4f8b72),
        coat: c(0x70c49b),
        highlight: c(0xb9efd2),
        accent: c(0xbe87d9),
        eye: c(0x16221d),
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Candy colours come at full strength from anywhere, a reference photo included, so the
    /// palette itself has to keep the eyes dark against the coat and the outline darker still.
    #[test]
    fn candy_colours_keep_a_dark_readable_face_on_any_coat() {
        let lightness = |color: Rgba| to_hsl(color).2;
        let mut coats = vec![[0, 0, 0], [255, 255, 255], [128, 128, 128]];
        for r in (0..=255).step_by(51) {
            for g in (0..=255).step_by(51) {
                for b in (0..=255).step_by(51) {
                    coats.push([r as u8, g as u8, b as u8]);
                }
            }
        }
        for coat in &coats {
            for accent in [[0, 0, 0], [255, 255, 255], [255, 224, 129]] {
                let palette = candy_palette(*coat, accent);
                assert!(
                    lightness(palette.coat) - lightness(palette.eye) >= 0.4,
                    "{coat:?}: eyes too close to the coat"
                );
                assert!(lightness(palette.outline) < lightness(palette.shadow));
                assert!(lightness(palette.shadow) < lightness(palette.coat));
                assert!(lightness(palette.coat) < lightness(palette.highlight));
                assert!(
                    lightness(palette.accent) - lightness(palette.eye) >= 0.4,
                    "{accent:?}: an accent cheek or beak has to show on the face"
                );
            }
        }
    }

    #[test]
    fn belongings_stay_distinct_from_the_creature_carrying_them() {
        let distance = |a: Rgba, b: Rgba| {
            let channel = |x: u8, y: u8| (f32::from(x) - f32::from(y)).powi(2);
            (channel(a.r, b.r) + channel(a.g, b.g) + channel(a.b, b.b)).sqrt()
        };
        for palette in PALETTES {
            for seed in 0..64_u64 {
                let prop = prop_palette(palette, seed);
                assert_eq!(
                    prop.accent,
                    prop_palette(palette, seed).accent,
                    "props are deterministic"
                );
                assert!(
                    distance(prop.accent, palette.coat) > 90.0,
                    "a prop should not read as another patch of coat"
                );
                assert!(
                    distance(prop.accent, palette.highlight) > 60.0,
                    "a prop should not read as a highlight"
                );
                // The shared dark outline keeps props matching the pixel-art style.
                assert_eq!(prop.outline, palette.outline);
            }
        }
    }
}
