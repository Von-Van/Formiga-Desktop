//! What a find is made of. A thing that has a colour of its own in the world — an acorn, a clover
//! leaf, a brass buckle, a teacup's china — is drawn in that colour whichever colony finds it, so
//! it can be told at a glance for what it is. The colony's own colour is kept for what is dyed,
//! painted or printed: a ribbon, a button, a bead, a scrap of wool. Those two are [`Ink::Dye`] and
//! [`Ink::Accent`], read from the [`TrinketInk`] the colony's sheet chose to stand clear of every
//! coat in it.
//!
//! Every ramp runs darkest to lightest — edge, shadow, base, light, shine — with its shadows
//! leaning cool and its lights warm, as everything is lit from the upper left. The shared dark
//! outline is drawn round the whole find afterwards; `edge` is for lines and creases inside it.

use super::TrinketInk;
use crate::Rgba;
use crate::paint::{Ramp, mix};

/// What one letter of a find's drawing is painted in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Ink {
    /// A material with its own colour.
    Natural(Ramp),
    /// The colony's colour for this find.
    Dye,
    /// The colony's contrasting detail colour for this find.
    Accent,
}

impl Ink {
    /// The five tones, edge to shine, this ink paints in for a find drawn in `ink`.
    pub(crate) fn tones(self, ink: TrinketInk) -> [Rgba; 5] {
        let white = Rgba::new(255, 252, 240, 255);
        match self {
            Self::Natural(ramp) => [ramp.edge, ramp.shadow, ramp.base, ramp.light, ramp.shine],
            Self::Dye => [
                mix(ink.deep, ink.outline, 0.45),
                ink.deep,
                ink.body,
                ink.light,
                mix(ink.light, white, 0.55),
            ],
            Self::Accent => [
                mix(ink.accent, ink.outline, 0.55),
                mix(ink.accent, ink.outline, 0.28),
                ink.accent,
                mix(ink.accent, white, 0.38),
                mix(ink.accent, white, 0.72),
            ],
        }
    }
}

pub(crate) const fn n(edge: u32, shadow: u32, base: u32, light: u32, shine: u32) -> Ink {
    Ink::Natural(Ramp::new(edge, shadow, base, light, shine))
}

pub(crate) const DYE: Ink = Ink::Dye;
pub(crate) const ACCENT: Ink = Ink::Accent;

// Wood and the things trees drop.
/// Oak: furniture, a broom handle, a toy block.
pub(crate) const OAK: Ink = n(0x4a2c1c, 0x7a4a2a, 0xa86c3c, 0xc99358, 0xe6c08a);
/// Pale wood: a clothes peg, a spool, a lolly stick.
pub(crate) const PINE: Ink = n(0x7a5a34, 0xb08a54, 0xd6b47a, 0xead09c, 0xf8ecc8);
/// Bark, twigs and sticks.
pub(crate) const BARK: Ink = n(0x3a2618, 0x5c3c26, 0x7e5636, 0x9e744c, 0xc09c72);
/// An acorn's nut, and a walnut's shell.
pub(crate) const NUT: Ink = n(0x5a2e14, 0x8e4c1e, 0xbe7230, 0xdc9a4c, 0xf2c47c);
/// An acorn's cup, a pinecone's scales.
pub(crate) const CUP: Ink = n(0x3e3020, 0x6a5434, 0x8e7650, 0xae9870, 0xcebe98);
/// Cork and cardboard.
pub(crate) const CORK: Ink = n(0x6a4422, 0x9a6a3a, 0xc29458, 0xdab47c, 0xeed6a8);

// Growing things.
/// A leaf, a clover, a stalk.
pub(crate) const LEAF: Ink = n(0x1e4a2a, 0x2f6e36, 0x4a9a44, 0x74be58, 0xaedc80);
/// Spring green: a sprout, a pea pod, a carrot's top.
pub(crate) const SPROUT: Ink = n(0x2a5a20, 0x468a2c, 0x6cb63a, 0x9cd858, 0xd2f08e);
/// Moss and lichen.
pub(crate) const MOSS: Ink = n(0x2c3e1c, 0x4a6228, 0x6a8634, 0x8ea84a, 0xb8c870);
/// White petals: a daisy, a dandelion clock.
pub(crate) const PETAL: Ink = n(0x8c8aa0, 0xbcbccc, 0xe8e6ec, 0xfaf8f4, 0xffffff);
/// A flower's yellow middle, a buttercup.
pub(crate) const POLLEN: Ink = n(0x8a5a10, 0xc88a1c, 0xf0bc2c, 0xfadc5c, 0xfff2a8);
/// A rose's petal, a cherry blossom.
pub(crate) const ROSE: Ink = n(0x7a2440, 0xb03c5c, 0xdc6480, 0xf094a6, 0xfccad2);
/// Cherries, a radish, a ladybird.
pub(crate) const CHERRY: Ink = n(0x5a0e1c, 0x8e1a28, 0xc42e34, 0xe65a50, 0xfa9c86);
/// A carrot, an ember, marmalade.
pub(crate) const CARROT: Ink = n(0x7a2e0e, 0xb84c14, 0xe8761e, 0xf8a040, 0xfed086);
/// A toadstool's cap.
pub(crate) const TOADSTOOL: Ink = n(0x5e1414, 0x962424, 0xc8382c, 0xe46446, 0xf6a084);
/// A mushroom's stalk, an egg, a pearl.
pub(crate) const IVORY: Ink = n(0x8a7c6a, 0xbcae94, 0xe2d6bc, 0xf4ecd8, 0xfffcf2);

// Made things.
/// Brass and gold: a buckle, a bell, a key, a clock key.
pub(crate) const BRASS: Ink = n(0x5e3c0c, 0x96661a, 0xc89a2c, 0xe8c454, 0xfcec9c);
/// Silver and tin: a spoon, a thimble, a safety pin, a whistle.
pub(crate) const SILVER: Ink = n(0x3a4250, 0x6a7484, 0x9ca6b4, 0xc8d0da, 0xf2f6fa);
/// Copper: a coin, a penny, a kettle.
pub(crate) const COPPER: Ink = n(0x4e2414, 0x86401e, 0xb8642e, 0xda8c4c, 0xf4bc84);
/// Iron: a nail, a key's dark old metal, a stove.
pub(crate) const IRON: Ink = n(0x1e2028, 0x343844, 0x4c5262, 0x6a7284, 0x9aa2b2);
/// Clear glass: a bottle, a jar, a lens. Pale, with its own blue in the shadow.
pub(crate) const GLASS: Ink = n(0x4a7484, 0x82aebc, 0xb6dce2, 0xdcf2f2, 0xffffff);
/// Amber, honey, a glow.
pub(crate) const AMBER: Ink = n(0x6a2e08, 0xa85a10, 0xdc8a1c, 0xf4b23c, 0xfde28a);
/// China: a teacup, an egg cup, a domino. Cool white.
pub(crate) const CHINA: Ink = n(0x6e7488, 0xa6acbe, 0xdadfe8, 0xf2f4f8, 0xffffff);
/// Paper and card: a postcard, a recipe card, a ticket.
pub(crate) const PAPER: Ink = n(0x8a7a5e, 0xc4b494, 0xeadfc6, 0xf8f0de, 0xfffcf4);
/// Ink and print on paper, a pencil's lead, a domino's pips.
pub(crate) const PRINT: Ink = n(0x14141c, 0x2a2a36, 0x40404e, 0x5c5c6c, 0x80808e);
/// Wax: a candle, a crayon's paper, soap.
pub(crate) const WAX: Ink = n(0x8a6e52, 0xc8aa84, 0xf0dcb8, 0xfaf0d8, 0xfffcf0);
/// A flame.
pub(crate) const FLAME: Ink = n(0x8a2a0a, 0xd8561a, 0xf89a2a, 0xfcd25a, 0xfff6c8);
/// Pebbles and stone, slate, chalk's grey.
pub(crate) const STONE: Ink = n(0x3c3a40, 0x625e66, 0x8c8890, 0xb0acb2, 0xd8d4d8);
/// A warm pebble, sand, a brick's mortar.
pub(crate) const SAND: Ink = n(0x5e4a36, 0x8c7458, 0xb49c7c, 0xd0bc9c, 0xece0c8);
/// Brick and terracotta.
pub(crate) const BRICK: Ink = n(0x501c14, 0x84301e, 0xb24a2c, 0xcc6e48, 0xe6a080);
/// Shell and mother of pearl.
pub(crate) const SHELL: Ink = n(0x7a5a5a, 0xb8908a, 0xe2c0b2, 0xf4e0d4, 0xfff8f2);
/// Baked things: a biscuit, a crust, a tart's pastry.
pub(crate) const BAKED: Ink = n(0x6a3a14, 0xa86224, 0xd8923c, 0xeeb85e, 0xfadc96);
/// Jam and berries.
pub(crate) const JAM: Ink = n(0x3e0a22, 0x6e1236, 0xa21e46, 0xcc3c5a, 0xec7c88);
/// Sugar, icing, cream, snow.
pub(crate) const SUGAR: Ink = n(0x8c94aa, 0xc2cada, 0xeef0f6, 0xfafcff, 0xffffff);
/// A feather's brown, an owl's.
pub(crate) const FEATHER: Ink = n(0x3a2a20, 0x6a5038, 0x987a58, 0xbea27c, 0xe2ceac);
/// Night sky and deep blue.
pub(crate) const NIGHT: Ink = n(0x10142e, 0x1e2856, 0x2e4084, 0x4a62b0, 0x7e9ad8);
/// Starlight and moonlight: a pale cool gold.
pub(crate) const STARLIGHT: Ink = n(0x7a6a2a, 0xbca848, 0xe8da7c, 0xf8f0b4, 0xfffde8);
/// Moonstone: a milky lilac.
pub(crate) const MOONSTONE: Ink = n(0x5a5478, 0x8c88b0, 0xbcbadc, 0xdedcf2, 0xfaf8ff);
/// Water, a dewdrop, a soap bubble's blue.
pub(crate) const WATER: Ink = n(0x1e4c7a, 0x2e74aa, 0x4ea0d4, 0x86c8ec, 0xd2f0fc);
/// Rubber and an eraser's pink.
pub(crate) const RUBBER: Ink = n(0x7a3a48, 0xb45e6e, 0xde8a96, 0xf0b2b8, 0xfcdcdc);
/// String, twine and wool's natural undyed cream.
pub(crate) const TWINE: Ink = n(0x6a5636, 0x9e865a, 0xc8b282, 0xe0d0a6, 0xf4ead0);
