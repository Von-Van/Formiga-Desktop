//! The keepsake tree: the fourth cell of the village atlas, and the eight places a colony can
//! hang a find on one of them.
//!
//! The village has two trees, one at each end, bookending the houses. Both are drawn from this
//! one cell — there is one village atlas and one texture — and the inward one is drawn mirrored,
//! so the pair reads as two trees rather than as the same tree twice. The tree belongs to the
//! village rather than to the wallpaper, so it is drawn from the shelter genome the houses are
//! drawn from — the same palette, the same dark single-pixel outline, the same flat shading and
//! the same ground shadow — and it varies with the colony seed exactly as a house does: the trunk
//! leans, the canopy's lobes shift, and the leaf specks land differently.
//!
//! `TRINKET_ANCHORS` is the fixed, ordered set of eight places something can sit on one tree, and
//! the two trees together have sixteen hooks: `hook_place` says where each is. What hangs on them
//! is `formiga_core::hung_keepsakes` — the keepsakes the owner chose, or with nothing chosen the
//! original sixteen finds on the hooks numbered after them and later finds on whichever hooks are
//! still empty. The overlay draws one 16x16 quad from the colony's own trinket atlas centred on
//! each anchor, so a colony that has found three things has three of them. The drawing here is what makes those quads
//! read as hung rather than stuck on: every anchor has a cord drawn down to it out of the leaves
//! above, ending a pixel inside where its keepsake's own drawing begins.

use crate::{Canvas, PALETTES, Rgba};
use formiga_core::{ShelterGenome, TreeEnd};

/// The tree's own cell, which its keepsake anchors are measured in. It stays the size it always
/// was when the houses grew a quarter in 0.61.0, and stands in the middle of its larger village
/// cell on the same ground line the houses stand on, three pixels above the foot of both.
pub const TREE_CELL: u32 = 64;

/// Where the tree's own cell sits inside its larger cell of the village atlas: across the middle,
/// and down on the ground line the houses stand on. Add it to an anchor to find that anchor in
/// the village cell.
pub const TREE_INSET: (i32, i32) = (
    (crate::SHELTER_SIZE - TREE_CELL) as i32 / 2,
    (crate::SHELTER_SIZE - TREE_CELL) as i32,
);
/// The ground line inside the cell, shared with `draw_dwelling`.
const GROUND_Y: i32 = 61;
/// The middle of the cell.
const CENTRE_X: i32 = 32;

/// Where one keepsake sits on the tree, in tree-local pixels: the centre of its 16x16 quad,
/// measured from the top-left of the tree's own cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrinketAnchor {
    pub x: i32,
    pub y: i32,
    /// True when this one hangs from the branches, false when it is set down among the roots.
    pub hanging: bool,
}

impl TrinketAnchor {
    /// The same place on a tree that is drawn mirrored — the inward bookend. The anchors come in
    /// mirrored pairs about the cell's own middle, so this is always another anchor's spot and
    /// always has a cord of its own hanging over it.
    pub const fn mirrored(self) -> Self {
        Self {
            x: TREE_CELL as i32 - 1 - self.x,
            ..self
        }
    }
}

const fn hung(x: i32, y: i32) -> TrinketAnchor {
    TrinketAnchor {
        x,
        y,
        hanging: true,
    }
}

/// The eight places on one tree, in the order a tree fills up. **All eight hang from the
/// branches** — the highest and most central first, then down and outward — because **the roots
/// belong to the belongings**: the colony keeps its pillows, lamps and cups scattered on the
/// ground around each trunk, and a keepsake down there would land on one of them. Eight rather
/// than sixteen because the village has two trees now, and filling them in this order means a
/// colony's earliest finds go up where they are seen.
///
/// They come in mirrored pairs about the cell's middle — 27 with 36, 19 with 44, and so on — so
/// the inward tree, which is the same cell drawn mirrored, still has a cord over every one of
/// them, at the same height.
///
/// The lowest pair still sits clear of where a belonging's own drawing reaches, so the two kinds
/// of thing mingle at the foot of the tree without covering each other. Every anchor's quad stays
/// inside the tree's cell and inside the ground the village reserves for a tree, and no two sit
/// within `ANCHOR_CLEARANCE` of each other, so a full tree still shows its crown and its trunk
/// rather than one bright smudge.
pub const TRINKET_ANCHORS: [TrinketAnchor; formiga_core::TRINKETS_PER_TREE as usize] = [
    hung(27, 15),
    hung(36, 15),
    hung(19, 24),
    hung(44, 24),
    hung(25, 33),
    hung(38, 33),
    hung(13, 38),
    hung(50, 38),
];

/// Where hook `hook` is: which of the village's two trees, and the anchor on that tree as it is
/// actually drawn there. Hooks 0..8 are the outward tree's anchors in the order a tree fills up,
/// and 8..16 the inward tree's, mirrored. With nothing chosen by hand the original sixteen finds
/// each keep the hook numbered after them, so they hang exactly where they always have.
pub fn hook_place(hook: usize) -> Option<(TreeEnd, TrinketAnchor)> {
    if hook >= formiga_core::TREE_HOOKS {
        return None;
    }
    let end = TreeEnd::of_hook(hook);
    let anchor = TRINKET_ANCHORS[hook % TRINKET_ANCHORS.len()];
    Some(match end {
        TreeEnd::Outward => (end, anchor),
        TreeEnd::Inward => (end, anchor.mirrored()),
    })
}

/// Every keepsake hanging in the two trees, with where it hangs: the colony's choice if it made
/// one, and otherwise the trees filling themselves as finds come in. A variant this build has no
/// artwork for — a save from a later version — is simply not drawn.
pub fn hung_trinkets(
    home: &formiga_core::ColonyHome,
    scrapbook: &[formiga_core::ScrapbookRecord],
) -> Vec<(u8, TreeEnd, TrinketAnchor)> {
    formiga_core::hung_keepsakes(home.tree_keepsakes.as_ref(), scrapbook)
        .iter()
        .enumerate()
        .filter_map(|(hook, variant)| {
            let variant = (*variant)?;
            let (end, anchor) = hook_place(hook)?;
            Some((variant, end, anchor))
        })
        .collect()
}

/// The closest two anchors may sit, in tree pixels. A keepsake draws eleven to fourteen pixels
/// wide inside its quad, so at this distance neighbours overlap at the edges — which is what a
/// tree somebody has been hanging things on for months looks like — without either one losing
/// its own outline.
pub const ANCHOR_CLEARANCE: i32 = 9;

/// How far up a cord runs out of the leaves, and where it stops relative to its anchor. It ends
/// one pixel inside the keepsake's own top so the two always meet.
const CORD_TOP: i32 = 8;
const CORD_BOTTOM: i32 = 5;

pub struct KeepsakeTreeRenderer;

impl KeepsakeTreeRenderer {
    /// One tree in its own cell, for the review sheets and for anything that wants it alone.
    pub fn render(genome: &ShelterGenome) -> Canvas {
        let mut canvas = Canvas::new(TREE_CELL, TREE_CELL);
        draw_tree(&mut canvas, genome, CENTRE_X, GROUND_Y);
        canvas
    }
}

/// Draws the tree into `canvas`, centred on `cx` and standing on `bottom`.
pub(crate) fn draw_tree(canvas: &mut Canvas, genome: &ShelterGenome, cx: i32, bottom: i32) {
    let palette = PALETTES[genome.palette_index as usize % PALETTES.len()];
    let accent = PALETTES[genome.accent_index as usize % PALETTES.len()];
    // Two pixels of deterministic character: which way the trunk leans, and how far the crown
    // sits off the middle. Both are small enough that every anchor stays under leaves.
    let lean = ((genome.detail_seed >> 3) % 3) as i32 - 1;
    let tilt = ((genome.detail_seed >> 11) % 3) as i32 - 1;

    // A soft ground shadow, matching the one every dwelling stands on. The colony's belongings
    // are drawn over it, so the yard reads as ground the tree is standing on.
    canvas.fill_ellipse(cx, bottom, 20, 3, Rgba::new(20, 24, 26, 95));

    // Roots: three humps either side of the trunk, drawn before it so the trunk sits on them.
    for side in [-1, 1] {
        for (reach, rise) in [(6, 2), (12, 1), (18, 0)] {
            let x = cx + side * reach;
            canvas.fill_ellipse(x, bottom - 1 - rise, 5, 2, palette.outline);
            canvas.fill_ellipse(x, bottom - 2 - rise, 4, 1, palette.shadow);
        }
    }

    // Trunk: a tapered column out of the roots and up into the crown. Dark all the way, with one
    // lit edge, so the tree reads as bark under leaves rather than as a stick of the same stuff.
    for y in (bottom - 38)..=(bottom - 1) {
        let up = bottom - y;
        let half = 5 - up / 12;
        let drift = cx + lean * up / 18;
        canvas.fill_rect(drift - half, y, half * 2 + 1, 1, palette.outline);
        canvas.fill_rect(drift - half + 1, y, half * 2 - 1, 1, palette.shadow);
        canvas.fill_rect(drift - half + 1, y, 1, 1, palette.coat);
    }

    // Limbs: two up into the crown, and two lower ones reaching out past it, which is what gives
    // the lowest row of keepsakes something to hang off.
    for (side, from, to, reach) in [
        (-1, 30, 38, 12),
        (1, 30, 38, 12),
        (-1, 24, 30, 18),
        (1, 24, 30, 18),
    ] {
        let (x0, y0) = (cx + lean, bottom - from);
        let (x1, y1) = (cx + side * reach, bottom - to);
        canvas.line(x0, y0, x1, y1, 1, palette.outline);
        canvas.line(x0, y0 - 1, x1, y1 - 1, 1, palette.shadow);
    }

    // The crown: a tall lobe over two wide ones, with a tuft out on each low limb. Separate
    // masses rather than one oval, so the shape has notches for things to hang in, and painted in
    // the accent the shelter's own roof is painted from.
    let crown: [(i32, i32, i32, i32); 7] = [
        (0, -46, 14, 12),
        (-13, -38, 12, 10),
        (13, -38, 12, 10),
        (-9, -31, 8, 7),
        (9, -31, 8, 7),
        (-18, -26, 8, 7),
        (18, -26, 8, 7),
    ];
    for (ox, oy, rx, ry) in crown {
        canvas.fill_ellipse(cx + ox + tilt, bottom + oy + tilt, rx, ry, palette.outline);
    }
    for (ox, oy, rx, ry) in crown {
        canvas.fill_ellipse(
            cx + ox + tilt,
            bottom + oy + tilt,
            rx - 1,
            ry - 1,
            accent.coat,
        );
    }
    // Flat shading: lit along the top left of the crown, in shadow under each mass below it.
    canvas.fill_ellipse(cx - 6 + tilt, bottom - 53 + tilt, 8, 3, accent.highlight);
    canvas.fill_ellipse(cx - 14 + tilt, bottom - 44 + tilt, 6, 2, accent.highlight);
    for side in [-1, 1] {
        canvas.fill_ellipse(
            cx + side * 14 + tilt,
            bottom - 31 + tilt,
            7,
            3,
            accent.shadow,
        );
        canvas.fill_ellipse(
            cx + side * 18 + tilt,
            bottom - 21 + tilt,
            5,
            2,
            accent.shadow,
        );
    }

    // A handful of leaf specks, in the accent the houses use for their own details.
    for index in 0..7_u32 {
        let byte = ((genome.detail_seed >> (index * 7)) & 0x3f) as i32;
        let x = cx - 20 + byte.rem_euclid(41);
        let y = bottom - 56 + (byte / 5).rem_euclid(32);
        if canvas.get(x, y).a > 0 {
            canvas.set(x, y, accent.accent);
        }
    }

    // The cords. Each anchor gets a short one out of the leaves down to where its keepsake's own
    // drawing begins, so a quad dropped on the anchor always meets something. The anchors are
    // mirrored pairs, so the mirrored tree keeps a cord over every one of them too.
    let lift = bottom - GROUND_Y;
    for anchor in TRINKET_ANCHORS.iter().filter(|anchor| anchor.hanging) {
        let x = cx - CENTRE_X + anchor.x;
        let y = anchor.y + lift;
        canvas.line(x, y - CORD_TOP, x, y - CORD_BOTTOM, 1, palette.outline);
        canvas.set(x, y - CORD_TOP, palette.shadow);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use formiga_core::ShelterStyle;
    use std::collections::BTreeSet;

    fn genome(seed: u64) -> ShelterGenome {
        ShelterGenome {
            style: ShelterStyle::LeafHouse,
            palette_index: 2,
            accent_index: 5,
            width: 38,
            height: 32,
            detail_seed: seed,
        }
    }

    /// Every anchor's quad has to land inside the tree's own cell and inside the ground the
    /// village reserves for a tree, two keepsakes have to stay far enough apart to read as two,
    /// and none of them may come down into the yard the belongings are scattered over. The order
    /// is part of the contract: a colony used to seeing its gem high on the left keeps seeing it
    /// there.
    #[test]
    fn the_anchors_stay_in_the_branches_keep_their_distance_and_never_move() {
        assert_eq!(
            TRINKET_ANCHORS.len() * 2,
            formiga_core::TREE_HOOKS,
            "two trees, half the hooks each"
        );
        assert_eq!(
            TRINKET_ANCHORS.len(),
            usize::from(formiga_core::TRINKETS_PER_TREE)
        );
        let half = crate::TRINKET_CELL as i32 / 2;
        let lot = formiga_core::TREE_WIDTH as i32 / 2;
        let mut seen = BTreeSet::new();
        for (index, anchor) in TRINKET_ANCHORS.iter().enumerate() {
            assert!(
                anchor.x - half >= 0
                    && anchor.x + half <= TREE_CELL as i32
                    && anchor.y - half >= 0
                    && anchor.y + half <= TREE_CELL as i32,
                "anchor {index} hangs off the tree's own cell"
            );
            assert!(
                (anchor.x - CENTRE_X).abs() + half <= lot,
                "anchor {index} hangs past the {} the village reserves for the tree",
                formiga_core::TREE_WIDTH
            );
            assert!(
                anchor.hanging && anchor.y + half <= LOWEST_KEEPSAKE_ROW,
                "anchor {index} has come down into the belongings' yard"
            );
            assert!(seen.insert((anchor.x, anchor.y)), "anchor {index} repeats");
            for (other, second) in TRINKET_ANCHORS.iter().enumerate().skip(index + 1) {
                let distance = (anchor.x - second.x).pow(2) + (anchor.y - second.y).pow(2);
                assert!(
                    distance >= ANCHOR_CLEARANCE * ANCHOR_CLEARANCE,
                    "anchors {index} and {other} sit on top of each other"
                );
            }
        }
        assert_eq!(
            (TRINKET_ANCHORS[0].x, TRINKET_ANCHORS[0].y),
            (27, 15),
            "the first find hangs high in the crown"
        );
        assert_eq!(
            (TRINKET_ANCHORS[7].x, TRINKET_ANCHORS[7].y),
            (50, 38),
            "the eighth hangs out on the lowest branch"
        );
        // The pairs are what lets the inward tree be this same cell drawn mirrored: every anchor
        // mirrors onto another one, at the same height, so a cord is waiting there.
        for (index, anchor) in TRINKET_ANCHORS.iter().enumerate() {
            let mirrored = anchor.mirrored();
            assert_eq!(mirrored.mirrored(), *anchor, "anchor {index} mirrors twice");
            assert!(
                TRINKET_ANCHORS.contains(&mirrored),
                "anchor {index} mirrors onto {mirrored:?}, which nothing hangs from"
            );
        }
    }

    /// Every hook has one place of its own: the first eight on the outward tree by the door and
    /// the next eight at the far end, each on an anchor as it is drawn there. The original sixteen
    /// finds, which each hung in a fixed place of their own before hooks, keep those places.
    #[test]
    fn each_hook_has_one_tree_and_one_anchor_of_its_own() {
        let mut seen = BTreeSet::new();
        for hook in 0..formiga_core::TREE_HOOKS {
            let (end, anchor) = hook_place(hook).expect("every hook has a place");
            assert_eq!(end, formiga_core::TreeEnd::of_hook(hook));
            assert_eq!(
                end,
                if hook < 8 {
                    formiga_core::TreeEnd::Outward
                } else {
                    formiga_core::TreeEnd::Inward
                }
            );
            assert!(
                TRINKET_ANCHORS.contains(&anchor),
                "hook {hook} hangs off a place no tree has"
            );
            assert!(
                seen.insert((end, anchor.x, anchor.y)),
                "hook {hook} shares a place with another"
            );
        }
        assert_eq!(hook_place(formiga_core::TREE_HOOKS), None);
        // Where the first sixteen finds hung before there were hooks: variant `n` on the outward
        // tree's anchor `n` for the everyday eight, and on the inward tree's mirrored anchor
        // `n - 8` for the rest.
        for variant in 0..16_usize {
            let before = if variant < 8 {
                (formiga_core::TreeEnd::Outward, TRINKET_ANCHORS[variant])
            } else {
                (
                    formiga_core::TreeEnd::Inward,
                    TRINKET_ANCHORS[variant - 8].mirrored(),
                )
            };
            assert_eq!(hook_place(variant), Some(before), "variant {variant} moved");
        }
    }

    /// How far down a keepsake may reach before it is in among the belongings rather than in the
    /// branches. A belonging's own drawing tops out about here.
    const LOWEST_KEEPSAKE_ROW: i32 = 50;

    /// The tree is drawn from the colony's own seed like everything else in the village: the same
    /// genome always gives the same tree, a different one gives a different tree, and whichever
    /// way the seed leans it the tree stays on its own ground with a cord over every anchor.
    #[test]
    fn the_tree_is_deterministic_and_stays_on_its_lot() {
        let first = KeepsakeTreeRenderer::render(&genome(0x1234));
        assert_eq!(first, KeepsakeTreeRenderer::render(&genome(0x1234)));
        assert_ne!(first, KeepsakeTreeRenderer::render(&genome(0x9f2c)));
        assert_eq!((first.width(), first.height()), (TREE_CELL, TREE_CELL));
        // The two seeds that lean the trunk and tilt the crown the furthest either way.
        for seed in [0_u64, u64::MAX] {
            let canvas = KeepsakeTreeRenderer::render(&genome(seed));
            let (x0, y0, x1, y1) = canvas.alpha_bounds().expect("a tree is drawn");
            let half = formiga_core::TREE_WIDTH as i32 / 2;
            assert!(
                x0 as i32 >= CENTRE_X - half && x1 as i32 <= CENTRE_X + half,
                "seed {seed} draws {x0}..{x1}, past the {} the village reserves",
                formiga_core::TREE_WIDTH
            );
            assert!(y0 > 0, "seed {seed} runs off the top of its cell");
            assert!(
                (GROUND_Y - 4..=GROUND_Y + 3).contains(&(y1 as i32)),
                "seed {seed} floats or sinks: it ends at {y1}"
            );
            for (index, anchor) in TRINKET_ANCHORS.iter().enumerate() {
                let strung = (anchor.y - CORD_TOP..=anchor.y - CORD_BOTTOM)
                    .all(|y| canvas.get(anchor.x, y).a > 0);
                assert!(strung, "seed {seed}: anchor {index} hangs from nothing");
            }
        }
    }
}
