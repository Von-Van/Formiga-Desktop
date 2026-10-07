//! Painting a sculpted form, piece by piece, from the back of the figure to the front: whatever
//! is behind the body first, then the body and everything on it, then the head and its face.

use super::figure::{Figure, Limb};
use super::motion::Pose;
use super::paint::{
    Frame, HD_OUTLINE, Mask, OUTLINE, Shape, Sheet, far, grain, hash, light, mix, shade,
    smooth_noise, smoothstep, soft,
};
use crate::Rgba;
use formiga_core::forms::{
    Ink, Marking, MarkingKind, Part, PartKind, Plan, Sculpt, Slot, Treatment,
};

/// What a painted region is, which decides how a marking or the underside colour falls on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Region {
    Body,
    Head,
    Neck,
    Leg {
        front: bool,
    },
    Arm,
    Tail,
    Ear,
    Mane,
    Fin,
    Wing,
    Hump,
    /// Snouts and muzzles: masks reach them, nothing else does.
    Snout,
}

#[derive(Clone, Copy)]
struct Inks {
    primary: Rgba,
    secondary: Rgba,
    underside: Rgba,
    accent: Rgba,
    feature: Rgba,
}

struct Painter<'a> {
    sheet: &'a mut Sheet,
    sculpt: &'a Sculpt,
    fig: &'a Figure,
    pose: Pose,
    inks: Inks,
    /// How far each eye sits from the middle of the face, for markings that frame them.
    eye_dx: f32,
    /// Where the top of the snout is, once one is drawn: where a nose horn grows.
    snout_top: (f32, f32),
    /// In high definition, the body once it is painted, and while a near leg is painted, the
    /// body it grows out of: the leg's outline stops at the body and the leg fades into it, so
    /// it reads as part of the creature rather than a peg stuck on.
    body: Option<Mask>,
    melt: Option<Mask>,
}

/// Paint `sculpt` laid out as `fig` in `pose`. `eye_spacing` is the face's own, from 4 to 7.
pub(crate) fn draw(sheet: &mut Sheet, sculpt: &Sculpt, fig: &Figure, pose: Pose, eye_spacing: u8) {
    let coat = sculpt.coat;
    let inks = Inks {
        primary: soft(coat.primary),
        secondary: soft(coat.secondary),
        underside: soft(coat.underside),
        accent: soft(coat.accent),
        feature: soft(coat.feature),
    };
    let mut painter = Painter {
        sheet,
        sculpt,
        fig,
        pose,
        inks,
        eye_dx: f32::from(eye_spacing) * 0.5 + 0.5,
        snout_top: (
            fig.face.0 + fig.head.r * 0.45,
            fig.face.1 + fig.head.r * 0.15,
        ),
        body: None,
        melt: None,
    };
    painter.paint();
}

impl Painter<'_> {
    fn part(&self, slot: Slot) -> Option<Part> {
        self.sculpt.part(slot).copied()
    }

    fn ink(&self, ink: Ink) -> Rgba {
        match ink {
            Ink::Primary => self.inks.primary,
            Ink::Secondary => self.inks.secondary,
            Ink::Underside => self.inks.underside,
            Ink::Accent => self.inks.accent,
            Ink::Feature => self.inks.feature,
        }
    }

    /// A part's size as a multiplier, from about half to about one and a half, at the form's
    /// scale.
    fn k(&self, part: &Part) -> f32 {
        (0.55 + f32::from(part.size) * 0.09) * self.fig.scale
    }

    fn paint(&mut self) {
        let fig = self.fig;
        let plan = fig.plan;
        let flippers = plan == Plan::Crawler
            && self
                .part(Slot::Fins)
                .is_some_and(|p| p.kind == PartKind::FinsFlippers);

        // Behind everything: the far wing, the far fin, the tail, things on the back, the
        // ruff and the far limbs.
        self.wings(false);
        self.fins(false, flippers);
        self.tail();
        self.back(true);
        if let Some(mane) = self.part(Slot::Mane)
            && matches!(mane.kind, PartKind::ManeRuff | PartKind::ManeShaggy)
        {
            self.mane_behind(mane);
        }
        let legs: Vec<Limb> = fig.legs.clone();
        for leg in legs.iter().filter(|leg| !leg.near) {
            if flippers && leg.front {
                continue;
            }
            self.limb(*leg, Region::Leg { front: leg.front });
        }
        for arm in fig.arms.clone().iter().filter(|arm| !arm.near) {
            self.limb(*arm, Region::Arm);
        }

        self.body();
        self.back(false);
        if plan == Plan::Crawler {
            self.shell();
        }
        for leg in legs.iter().filter(|leg| leg.near) {
            if flippers && leg.front {
                continue;
            }
            self.limb(*leg, Region::Leg { front: leg.front });
        }
        self.fins(true, flippers);
        self.wings(true);
        if let Some((a, b, r)) = fig.neck {
            self.region(
                &[Shape::capsule(a, b, r, r * 0.9)],
                Region::Neck,
                Frame::along(a, b, r),
                self.inks.primary,
                true,
                false,
            );
            if let Some(mane) = self.part(Slot::Mane)
                && mane.kind == PartKind::ManeCrest
            {
                self.crest_along(mane, a, b, r);
            }
        }
        self.ears(false);
        self.horns(false);
        self.head();
        self.ears(true);
        self.snout();
        self.horns(true);
        self.tusks();
        if let Some(mane) = self.part(Slot::Mane)
            && (mane.kind == PartKind::ManeTuft
                || (mane.kind == PartKind::ManeCrest && fig.neck.is_none()))
        {
            self.tuft(mane);
        }
        for arm in fig.arms.clone().iter().filter(|arm| arm.near) {
            self.limb(*arm, Region::Arm);
        }
    }

    /// Fill shapes as one region: outline, colour, the underside, shade and light, the coat's
    /// treatment, and its markings. `dim` draws it on the far side, a little in shadow.
    fn region(
        &mut self,
        shapes: &[Shape],
        region: Region,
        frame: Frame,
        base: Rgba,
        outlined: bool,
        dim: bool,
    ) -> Mask {
        let mask = self.sheet.mask(shapes);
        if mask.is_empty() {
            return mask;
        }
        let melt = self.melt.take();
        if let Some(body) = &melt {
            // A near leg: outlined only where it is clear of the body.
            let res = self.sheet.res() as f32;
            for (x, y, d) in mask.measured() {
                if d > -1.0 / res {
                    let clear = smoothstep(-0.3, 0.5, body.distance(x, y).min(4.0));
                    let alpha = ((HD_OUTLINE - d) * res + 0.5).clamp(0.0, 1.0) * clear;
                    self.sheet.paint(x, y, OUTLINE, alpha);
                }
            }
        } else if outlined {
            self.sheet.outline(&mask, OUTLINE);
        }
        let coat = self.sculpt.coat;
        let treatment = coat.treatment;
        let is_coat = base == self.inks.primary;
        if self.sheet.hd() {
            for (x, y) in mask.pixels() {
                let mut alpha = mask.coverage(x, y);
                if let Some(body) = &melt {
                    alpha *= 1.0 - smoothstep(0.7, 1.8, body.depth(x, y));
                }
                // Too little of the pixel to paint: no need to work out its colour.
                if alpha < 0.5 {
                    continue;
                }
                let (fx, fy) = self.sheet.center(x, y);
                let color =
                    self.hd_color(region, frame, base, is_coat, fx, fy, mask.depth(x, y), dim);
                self.sheet.paint(x, y, color, alpha);
            }
            if outlined && matches!(region, Region::Body | Region::Head) {
                self.hd_fringe(&mask, treatment, frame);
            }
            return mask;
        }
        for (x, y) in mask.pixels() {
            let (fx, fy) = self.sheet.center(x, y);
            let (u, v) = frame.at(fx, fy);
            let (mut color, _) = self.pattern(region, frame, base, is_coat, fx, fy);
            let shadow = match region {
                Region::Leg { .. } | Region::Arm => v > 0.35,
                Region::Neck => v < -0.45,
                _ => v > 0.5,
            };
            if shadow {
                color = shade(color);
            }
            // Light from the upper left.
            let shine = (u + 0.4).powi(2) + (v + 0.55).powi(2);
            if treatment == Treatment::Smooth
                && matches!(region, Region::Body | Region::Head)
                && shine < 0.035
            {
                color = light(color);
            }
            if mask.edge(x, y) {
                // Edges keep their colour; texture stays inside them.
            } else if let Some(textured) = texture(treatment, region, u, v, x, y, color) {
                color = textured;
            }
            if dim {
                color = far(color);
            }
            self.sheet.set(x, y, color);
        }
        if outlined && matches!(region, Region::Body | Region::Head) {
            self.fringe(&mask, treatment, base, frame);
        }
        mask
    }

    /// The coat's own colour at a point of a region, in frame pixels: the base, the underside
    /// where it reaches, and the markings over both. Says whether it fell on the underside.
    fn pattern(
        &self,
        region: Region,
        frame: Frame,
        base: Rgba,
        is_coat: bool,
        fx: f32,
        fy: f32,
    ) -> (Rgba, bool) {
        let (u, v) = frame.at(fx, fy);
        let mut color = base;
        let mut under = false;
        if is_coat && self.underside(region, u, v) {
            color = self.inks.underside;
            under = true;
        }
        if is_coat || matches!(region, Region::Ear | Region::Mane | Region::Snout) {
            for marking in &self.sculpt.markings {
                if self.marked(marking, region, u, v, fx, fy, under) {
                    color = soft(marking.color);
                }
            }
        }
        (color, under)
    }

    /// A region's colour at a point in high definition: the coat's pattern, then its shade
    /// underneath in three tones rather than two, a patch of light from the upper left, a line
    /// of shadow just inside the outline on the far side, and the coat's treatment drawn as
    /// fine strands, feathers or folds. Every colour is one of a few flat tones, as pixel art's
    /// are; there is more detail, not a different way of drawing.
    #[allow(clippy::too_many_arguments)]
    fn hd_color(
        &self,
        region: Region,
        frame: Frame,
        base: Rgba,
        is_coat: bool,
        fx: f32,
        fy: f32,
        depth: f32,
        dim: bool,
    ) -> Rgba {
        let (mut color, _) = self.pattern(region, frame, base, is_coat, fx, fy);
        let (u, v) = frame.at(fx, fy);
        // How far past the line Desktop shades from: a half shade either side of it, and the
        // full shade beyond, three tones where a companion has two.
        let past = match region {
            Region::Leg { .. } | Region::Arm => v - 0.35,
            Region::Neck => -0.45 - v,
            _ => v - 0.5,
        };
        let shadow = if past > 0.12 {
            1.0
        } else if past > -0.1 {
            0.5
        } else {
            0.0
        };
        color = mix(color, shade(color), shadow);
        let treatment = self.sculpt.coat.treatment;
        // Light from the upper left: a lit patch on every coat, a bright glint on smooth skin.
        let shine = (u + 0.4).powi(2) + (v + 0.55).powi(2);
        if matches!(region, Region::Body | Region::Head) {
            if treatment == Treatment::Smooth && shine < 0.035 {
                color = light(color);
            } else if shine < 0.1 {
                color = mix(color, light(color), 0.3);
            }
        }
        // Turned away from the light, a line of shadow just inside the outline.
        if depth < 0.55 && v + u * 0.3 > 0.3 && shadow < 1.0 {
            color = mix(color, shade(color), 0.5);
        }
        if depth > 0.7
            && let Some((textured, amount)) = texture_hd(treatment, region, u, v, fx, fy, color)
            && amount >= 0.5
        {
            color = textured;
        }
        if dim { far(color) } else { color }
    }

    /// Ruffled edges for fur: a tuft here and there along the top, and for shaggy fur a fringe
    /// all along the bottom.
    fn fringe(&mut self, mask: &Mask, treatment: Treatment, base: Rgba, frame: Frame) {
        let (every, below) = match treatment {
            Treatment::Fur => (7, false),
            Treatment::Shaggy => (3, true),
            _ => return,
        };
        for (x, y) in mask.pixels() {
            let (fx, fy) = self.sheet.center(x, y);
            let (_, v) = frame.at(fx, fy);
            if v < -0.5 && !mask.has(x, y - 1) && hash(x, y, 11).is_multiple_of(every) {
                self.sheet.set(x, y - 2, OUTLINE);
                self.sheet.set(x, y - 1, self.sheet.get(x, y));
            }
            if below && v > 0.4 && !mask.has(x, y + 1) && (x + y) % 2 == 0 {
                self.sheet.set(x, y + 1, shade(base));
                self.sheet.set(x, y + 2, OUTLINE);
            }
        }
    }

    /// Fur's ruffled edge in high definition: small pointed tufts along the top, each grown
    /// from the coat beneath it with no line where it joins, and for shaggy fur a fringe of
    /// longer ones all along the bottom.
    fn hd_fringe(&mut self, mask: &Mask, treatment: Treatment, frame: Frame) {
        let (spacing, below) = match treatment {
            Treatment::Fur => (4.2_f32, false),
            Treatment::Shaggy => (1.6, true),
            _ => return,
        };
        let res = self.sheet.res();
        // Walk the outline along the body, a tuft every so often where it faces up (or down).
        let along = frame.half_length * 2.0;
        let count = (along / spacing).floor() as i32;
        let (ax, ay) = frame.axis;
        let across = (-ay, ax);
        let mut tufts = Vec::new();
        for i in 0..=count {
            let t = -0.85 + 1.7 * (i as f32 + 0.5) / (count as f32 + 1.0);
            let jitter = ((hash(i, 7, 11) % 100) as f32 / 100.0 - 0.5) * 0.12;
            let t = t + jitter;
            for (side, wanted) in [(-1.0_f32, true), (1.0, below)] {
                if !wanted {
                    continue;
                }
                // From the middle, out across the body to where the mask ends.
                let mid = (
                    frame.cx + ax * t * frame.half_length,
                    frame.cy + ay * t * frame.half_length,
                );
                let mut edge = None;
                let mut s = 0.0;
                while s < frame.half_width * 1.6 + 2.0 {
                    let p = (mid.0 + across.0 * side * s, mid.1 + across.1 * side * s);
                    let px = (
                        (p.0 * res as f32).floor() as i32,
                        (p.1 * res as f32).floor() as i32,
                    );
                    if !mask.has(px.0, px.1) && s > 0.0 {
                        edge = Some(p);
                        break;
                    }
                    s += 0.25;
                }
                let Some(edge) = edge else { continue };
                // A tuft only grows where the edge faces the way it should.
                let out = (across.0 * side, across.1 * side);
                if side < 0.0 && out.1 > -0.55 {
                    continue;
                }
                tufts.push((edge, out, side > 0.0, i));
            }
        }
        // Every tuft is outlined as one, so neighbours run together with no line between them,
        // and each is filled from the coat it grows out of.
        let mut shapes = Vec::new();
        let mut fills = Vec::new();
        for (edge, out, low, i) in tufts {
            let lean = if low { 0.25 } else { -0.55 };
            let dir = (out.0 + lean * out.1.abs(), out.1);
            let norm = (dir.0 * dir.0 + dir.1 * dir.1).sqrt().max(0.01);
            let dir = (dir.0 / norm, dir.1 / norm);
            let len = if low { 1.3 } else { 0.9 } + (hash(i, 3, 5) % 3) as f32 * 0.2;
            let half = if low { 0.7 } else { 0.9 };
            let side = (-dir.1, dir.0);
            let root = (edge.0 - out.0 * 0.9, edge.1 - out.1 * 0.9);
            let tip = (edge.0 + dir.0 * len, edge.1 + dir.1 * len);
            let tuft = Shape::Polygon(vec![
                (root.0 - side.0 * half, root.1 - side.1 * half),
                tip,
                (root.0 + side.0 * half, root.1 + side.1 * half),
            ]);
            // The coat just inside the edge, which the tuft is grown from.
            let from = (
                ((edge.0 - out.0 * 1.4) * res as f32) as i32,
                ((edge.1 - out.1 * 1.4) * res as f32) as i32,
            );
            let coat = self.sheet.get(from.0, from.1);
            if coat.a == 0 {
                continue;
            }
            shapes.push(tuft.clone());
            fills.push((tuft, coat));
        }
        if shapes.is_empty() {
            return;
        }
        let all = self.sheet.mask(&shapes);
        let res_f = res as f32;
        for (x, y, d) in all.measured() {
            if mask.has(x, y) || d < -1.0 / res_f {
                continue;
            }
            self.sheet.paint(
                x,
                y,
                OUTLINE,
                ((HD_OUTLINE - d) * res_f + 0.5).clamp(0.0, 1.0),
            );
        }
        for (tuft, coat) in fills {
            self.sheet.blob(&[tuft], coat);
        }
    }

    /// Whether the underside colour falls here.
    fn underside(&self, region: Region, u: f32, v: f32) -> bool {
        let reach = f32::from(self.sculpt.coat.underside_reach) / 10.0;
        if reach <= 0.0 {
            return false;
        }
        let upright = matches!(self.fig.plan, Plan::Upright | Plan::Percher);
        match region {
            Region::Body if upright => {
                let k = 0.35 + reach * 0.75;
                ((u - 0.2) / (0.55 * k)).powi(2) + ((v - 0.15) / (0.8 * k)).powi(2) < 1.0
            }
            Region::Body => v > 1.0 - reach * 1.1,
            Region::Head => {
                ((u - 0.3) / (0.35 + reach * 0.45)).powi(2)
                    + ((v - 0.6) / (0.2 + reach * 0.35)).powi(2)
                    < 1.0
            }
            _ => false,
        }
    }

    /// Whether `marking` colours this pixel of `region`.
    #[allow(clippy::too_many_arguments)]
    fn marked(
        &self,
        m: &Marking,
        region: Region,
        u: f32,
        v: f32,
        fx: f32,
        fy: f32,
        under: bool,
    ) -> bool {
        let size = 0.6 + f32::from(m.size) * 0.12;
        let amount = f32::from(m.amount) / 10.0;
        let seed = u32::from(m.layout) * 7919 + m.kind as u32 * 31;
        let coat_like = matches!(
            region,
            Region::Body
                | Region::Neck
                | Region::Head
                | Region::Leg { .. }
                | Region::Arm
                | Region::Tail
        );
        match m.kind {
            MarkingKind::Stripes => {
                if under || !coat_like {
                    return false;
                }
                let period = 4.0 * size + 1.0;
                let width = 0.8 + amount * 1.6;
                let offset = (seed % 7) as f32;
                match region {
                    Region::Leg { .. } | Region::Arm => {
                        u < 0.65 && (fy + offset).rem_euclid(period) < width * 0.8
                    }
                    Region::Head => {
                        v < -0.3 && u.abs() < 0.5 && (fx + offset).rem_euclid(period * 0.6) < 1.0
                    }
                    Region::Tail | Region::Neck => {
                        (u * 8.0 * size.recip() + offset).rem_euclid(2.2) < 0.5 + amount * 0.6
                    }
                    _ => {
                        let wobble = (smooth_noise(fx, fy, 3.0, seed) - 0.5) * 2.0;
                        v < 0.6
                            && u > -0.92
                            && (fx + v * 2.0 + wobble + offset).rem_euclid(period) < width
                            && (v < 0.2 || (fx + offset).rem_euclid(period * 2.0) < period)
                    }
                }
            }
            MarkingKind::Spots | MarkingKind::Rosettes => {
                if under || !coat_like || (matches!(region, Region::Leg { .. }) && u > 0.5) {
                    return false;
                }
                let rosette = m.kind == MarkingKind::Rosettes;
                let cell = (if rosette { 4.5 } else { 3.5 }) * size + 1.0;
                let (gx, gy) = ((fx / cell).floor(), (fy / cell).floor());
                let mut hit = false;
                for (dx, dy) in [
                    (0, 0),
                    (1, 0),
                    (0, 1),
                    (1, 1),
                    (-1, 0),
                    (0, -1),
                    (-1, -1),
                    (1, -1),
                    (-1, 1),
                ] {
                    let (cx, cy) = (gx as i32 + dx, gy as i32 + dy);
                    let h = hash(cx, cy, seed);
                    if (h % 100) as f32 >= amount * 100.0 {
                        continue;
                    }
                    let px = (cx as f32 + 0.25 + ((h >> 8) % 50) as f32 / 100.0) * cell;
                    let py = (cy as f32 + 0.25 + ((h >> 16) % 50) as f32 / 100.0) * cell;
                    let r = cell * 0.3 + 0.3;
                    let d = ((fx - px).powi(2) + (fy - py).powi(2)).sqrt();
                    if rosette {
                        if d < r + 0.6 && d >= r - 0.6 && !(h >> 24).is_multiple_of(5) {
                            hit = true;
                        }
                    } else if d < r {
                        hit = true;
                    }
                }
                hit
            }
            MarkingKind::Patches => {
                if under || !coat_like || (matches!(region, Region::Leg { .. }) && u > 0.3) {
                    return false;
                }
                let cell = 4.0 * size + 1.5;
                if region == Region::Head && v > -0.3 {
                    return false;
                }
                let (gx, gy) = ((fx / cell).floor() as i32, (fy / cell).floor() as i32);
                let mut nearest = [f32::MAX, f32::MAX];
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        let h = hash(gx + dx, gy + dy, seed);
                        let px = ((gx + dx) as f32 + ((h >> 4) % 100) as f32 / 100.0) * cell;
                        let py = ((gy + dy) as f32 + ((h >> 12) % 100) as f32 / 100.0) * cell;
                        let d = ((fx - px).powi(2) + (fy - py).powi(2)).sqrt();
                        if d < nearest[0] {
                            nearest = [d, nearest[0]];
                        } else if d < nearest[1] {
                            nearest[1] = d;
                        }
                    }
                }
                nearest[1] - nearest[0] > 0.6 + (1.0 - amount) * 1.6
            }
            MarkingKind::Mottle => {
                if under {
                    return false;
                }
                smooth_noise(fx, fy, 3.0 + size * 2.5, seed) > 0.85 - amount * 0.45
            }
            MarkingKind::EyePatches => {
                if region != Region::Head {
                    return false;
                }
                let (ex, ey) = self.fig.face;
                let rx = 1.6 + size * 0.9 + amount * 0.6;
                let ry = 2.2 + size * 1.1 + amount * 0.6;
                [-1.0_f32, 1.0].into_iter().any(|side| {
                    let cx = ex + side * self.eye_dx;
                    let cy = ey - 0.5;
                    let (dx, dy) = (fx - cx, fy - cy);
                    // Drooping toward the outside, like a panda's.
                    let tilt = side * 0.5;
                    let (sin, cos) = tilt.sin_cos();
                    let a = dx * cos + dy * sin;
                    let b = -dx * sin + dy * cos;
                    (a / rx).powi(2) + (b / ry).powi(2) < 1.0
                })
            }
            MarkingKind::Mask => match region {
                Region::Head => v > 0.15 - amount * 0.5 && u > -0.45,
                Region::Snout => true,
                _ => false,
            },
            MarkingKind::Socks => match region {
                Region::Leg { .. } | Region::Arm => u > 0.75 - amount * 1.4,
                _ => false,
            },
            MarkingKind::Saddle => match region {
                Region::Body => v < 0.1 - amount * 0.3 && u.abs() < 0.3 + amount * 0.35,
                Region::Hump => true,
                _ => false,
            },
            MarkingKind::Cap => match region {
                Region::Head => v < 1.2 - amount * 2.2 || amount >= 0.95,
                Region::Ear | Region::Mane => true,
                Region::Neck => u > 0.5 - amount * 0.6,
                _ => false,
            },
            MarkingKind::Shoulders => {
                let upright = matches!(self.fig.plan, Plan::Upright | Plan::Percher);
                match region {
                    Region::Body if upright => v < -0.35 + amount * 0.3,
                    Region::Body => (u - 0.42).abs() < 0.12 + amount * 0.18,
                    Region::Leg { front: true } | Region::Arm => true,
                    _ => false,
                }
            }
        }
    }

    fn body(&mut self) {
        let fig = self.fig;
        let b = fig.body;
        let frame = Frame {
            cx: b.cx,
            cy: b.cy,
            axis: (b.angle.cos(), b.angle.sin()),
            half_length: b.rx,
            half_width: b.ry,
        };
        let body = Shape::turned(b.cx, b.cy, b.rx, b.ry, b.angle);
        if fig.plan == Plan::Floater {
            // One smooth silhouette, the head part of the body; then the head painted again
            // without a line between them, so the face's markings have a head to fall on.
            let h = fig.head;
            let head = Shape::ellipse(h.x, h.y, h.r * 1.05, h.r * 0.95);
            self.region(
                &[body, head.clone()],
                Region::Body,
                frame,
                self.inks.primary,
                true,
                false,
            );
            return;
        }
        let mask = self.region(&[body], Region::Body, frame, self.inks.primary, true, false);
        if self.sheet.hd() {
            self.body = Some(mask);
        }
    }

    fn head(&mut self) {
        let h = self.fig.head;
        let frame = Frame::level(h.x, h.y, h.r, h.r * 0.92);
        let floater = self.fig.plan == Plan::Floater;
        let shape = if floater {
            Shape::ellipse(h.x, h.y, h.r * 1.05, h.r * 0.95)
        } else {
            Shape::ellipse(h.x, h.y, h.r, h.r * 0.92)
        };
        if floater && self.sheet.hd() {
            // Only over the body it already shares, never over a line drawn on it, so it adds
            // no line of its own.
            let mask = self.sheet.mask(&[shape]);
            let base = self.inks.primary;
            for (x, y) in mask.pixels() {
                let (fx, fy) = self.sheet.center(x, y);
                let color = self.hd_color(
                    Region::Head,
                    frame,
                    base,
                    true,
                    fx,
                    fy,
                    mask.depth(x, y),
                    false,
                );
                let alpha = mask.coverage(x, y) * (1.0 - self.sheet.inked(x, y));
                if self.sheet.get(x, y).a > 0 {
                    self.sheet.paint(x, y, color, alpha);
                }
            }
            return;
        }
        if floater {
            // Only over the body it already shares, so it adds no line.
            let mask = self.sheet.mask(&[shape]);
            let body = self.sheet.mask(&[Shape::turned(
                self.fig.body.cx,
                self.fig.body.cy,
                self.fig.body.rx,
                self.fig.body.ry,
                self.fig.body.angle,
            )]);
            let inner: Vec<(i32, i32)> = mask
                .pixels()
                .filter(|&(x, y)| !mask.edge(x, y) || body.has(x, y))
                .collect();
            let base = self.inks.primary;
            for (x, y) in inner {
                if self.sheet.get(x, y) == OUTLINE {
                    continue;
                }
                let (fx, fy) = self.sheet.center(x, y);
                let (u, v) = frame.at(fx, fy);
                let mut color = base;
                let mut under = false;
                if self.underside(Region::Head, u, v) {
                    color = self.inks.underside;
                    under = true;
                }
                for marking in &self.sculpt.markings {
                    if self.marked(marking, Region::Head, u, v, fx, fy, under) {
                        color = soft(marking.color);
                    }
                }
                if v > 0.5 {
                    color = shade(color);
                }
                if self.sculpt.coat.treatment == Treatment::Smooth
                    && (u + 0.4).powi(2) + (v + 0.55).powi(2) < 0.035
                {
                    color = light(color);
                }
                self.sheet.set(x, y, color);
            }
            return;
        }
        self.region(
            &[shape],
            Region::Head,
            frame,
            self.inks.primary,
            true,
            false,
        );
    }

    fn limb(&mut self, limb: Limb, region: Region) {
        let dim = !limb.near;
        let feet = self.part(Slot::Feet);
        let talons = feet.is_some_and(|f| f.kind == PartKind::FeetTalons);
        let fill = if talons && region != Region::Arm {
            self.inks.feature
        } else {
            self.inks.primary
        };
        let (top, foot, r) = (limb.top, limb.foot, limb.radius);
        if talons && region != Region::Arm {
            // Thin legs, drawn flat in the feature colour.
            let color = if dim { far(fill) } else { fill };
            self.sheet.flat(&[Shape::capsule(top, foot, r, r)], color);
        } else {
            if limb.near && matches!(region, Region::Leg { .. }) {
                self.melt = self.body.clone();
            }
            self.region(
                &[Shape::capsule(top, foot, r, r * 0.95)],
                region,
                Frame::along(top, foot, r),
                fill,
                true,
                dim,
            );
            self.melt = None;
        }
        self.foot(limb, region, dim);
    }

    fn foot(&mut self, limb: Limb, region: Region, dim: bool) {
        let (fx, fy) = limb.foot;
        let r = limb.radius;
        let tone = |c: Rgba| if dim { far(c) } else { c };
        let kind = match region {
            Region::Arm => match self.part(Slot::Feet).map(|p| p.kind) {
                Some(PartKind::FeetHands) | None => Some(PartKind::FeetHands),
                other => other,
            },
            _ => self.part(Slot::Feet).map(|p| p.kind),
        };
        let size = self
            .part(Slot::Feet)
            .map_or(1.0, |p| 0.7 + f32::from(p.size) * 0.06);
        let socks = self
            .sculpt
            .markings
            .iter()
            .rev()
            .find(|m| {
                m.kind == MarkingKind::Socks || (m.kind == MarkingKind::Shoulders && limb.front)
            })
            .map(|m| soft(m.color));
        let paw = socks.unwrap_or(self.inks.primary);
        match kind {
            Some(PartKind::FeetPaws) => {
                self.sheet.flat(
                    &[Shape::ellipse(
                        fx + 0.8,
                        fy - r * 0.55,
                        r * 1.15 * size,
                        r * 0.75,
                    )],
                    tone(paw),
                );
            }
            Some(PartKind::FeetBigPaws) => {
                self.sheet.flat(
                    &[Shape::ellipse(
                        fx + 1.0,
                        fy - r * 0.6,
                        r * 1.45 * size,
                        r * 0.9,
                    )],
                    tone(paw),
                );
            }
            Some(PartKind::FeetHooves) => {
                let band = r * 0.7 * size + 0.5;
                self.sheet.flat(
                    &[Shape::Polygon(vec![
                        (fx - r - 0.2, fy - band),
                        (fx + r + 0.4, fy - band),
                        (fx + r + 0.8, fy + 0.5),
                        (fx - r - 0.2, fy + 0.5),
                    ])],
                    tone(self.inks.feature),
                );
            }
            Some(PartKind::FeetPads) => {
                self.sheet.flat(
                    &[Shape::ellipse(
                        fx + 0.4,
                        fy - r * 0.4,
                        r * 1.25 * size,
                        r * 0.6,
                    )],
                    tone(paw),
                );
                for dx in [-0.5_f32, 1.0] {
                    let x = (fx + dx * r * 0.7).floor() as i32 + 1;
                    if self.sheet.hd() {
                        let toe = Shape::ellipse(x as f32 + 0.5, fy.floor() + 0.4, 0.6, 0.45);
                        self.sheet.blob(&[toe], tone(self.inks.feature));
                    } else {
                        self.sheet
                            .set(x, fy.floor() as i32, tone(self.inks.feature));
                    }
                }
            }
            Some(PartKind::FeetHands) => {
                self.sheet.flat(
                    &[Shape::ellipse(
                        fx,
                        fy - r * 0.3,
                        r * 1.0 * size,
                        r * 0.85 * size,
                    )],
                    tone(paw),
                );
            }
            Some(PartKind::FeetTalons) => {
                let claw = tone(self.inks.feature);
                if self.sheet.hd() {
                    // Toes gripping the ground, a short one behind, and a pale claw on top.
                    let (x, y) = (fx.floor(), fy.floor());
                    self.sheet.blob(
                        &[
                            Shape::capsule((x + 0.8, y + 0.5), (x + 3.2, y + 1.4), 0.5, 0.35),
                            Shape::capsule((x + 0.2, y + 0.5), (x - 2.0, y + 1.4), 0.5, 0.35),
                        ],
                        OUTLINE,
                    );
                    self.sheet.blob(
                        &[Shape::capsule(
                            (x + 1.4, y - 0.4),
                            (x + 2.6, y - 0.3),
                            0.55,
                            0.4,
                        )],
                        claw,
                    );
                    return;
                }
                let (x, y) = (fx.floor() as i32, fy.floor() as i32);
                for (dx, dy) in [(1, 0), (2, 0), (3, 1), (-1, 0), (-2, 1)] {
                    self.sheet.set(x + dx, y + dy, OUTLINE);
                }
                for dx in [1, 2] {
                    self.sheet.set(x + dx, y - 1, claw);
                }
            }
            _ => {}
        }
    }

    fn tail(&mut self) {
        let Some(part) = self.part(Slot::Tail) else {
            return;
        };
        let fig = self.fig;
        let k = self.k(&part);
        let (rx, ry) = fig.tail_root;
        let len = fig.tail_length * (0.6 + f32::from(part.size) * 0.08);
        let sway = self.pose.sway;
        let tilt = f32::from(part.tilt) * 0.2;
        let ink = self.ink(part.ink);
        let lift = f32::from(part.lift);
        let root = (rx, ry - lift);
        // Up and back from the rump, unless the plan carries it otherwise.
        let up = match fig.plan {
            Plan::Floater => 0.1,
            Plan::Percher | Plan::Upright | Plan::Crawler => -0.5,
            _ => 0.8,
        } + tilt
            + sway * 0.12;
        let (sin, cos) = up.sin_cos();
        let end = (root.0 - cos * len, root.1 - sin * len);
        let frame = Frame::along(root, end, 2.5 * k);
        let along = |t: f32| (root.0 + (end.0 - root.0) * t, root.1 + (end.1 - root.1) * t);
        match part.kind {
            PartKind::TailStub => {
                self.region(
                    &[Shape::ellipse(root.0 - 1.0, root.1, 2.6 * k, 2.4 * k)],
                    Region::Tail,
                    frame,
                    ink,
                    true,
                    false,
                );
            }
            PartKind::TailTaper => {
                self.region(
                    &[Shape::capsule(root, end, 1.9 * k, 0.9 * k)],
                    Region::Tail,
                    frame,
                    ink,
                    true,
                    false,
                );
            }
            PartKind::TailTufted => {
                self.region(
                    &[Shape::capsule(root, end, 1.3 * k, 0.9 * k)],
                    Region::Tail,
                    frame,
                    ink,
                    true,
                    false,
                );
                self.region(
                    &[Shape::ellipse(end.0, end.1, 2.2 * k, 2.6 * k)],
                    Region::Mane,
                    frame,
                    self.inks.secondary,
                    true,
                    false,
                );
            }
            PartKind::TailPlume => {
                // Carried low and full.
                let (sin, cos) = (-0.35 + tilt + sway * 0.1_f32).sin_cos();
                let end = (root.0 - cos * len, root.1 - sin * len);
                self.region(
                    &[
                        Shape::capsule(root, end, 2.4 * k, 2.8 * k),
                        Shape::ellipse(end.0, end.1, 2.6 * k, 2.6 * k),
                    ],
                    Region::Tail,
                    Frame::along(root, end, 2.6 * k),
                    ink,
                    true,
                    false,
                );
            }
            PartKind::TailFluffy => {
                let mid = (along(0.55).0 - 1.5, along(0.55).1 + 2.0);
                let end = (end.0 + 1.0, end.1 - 1.0);
                self.region(
                    &[
                        Shape::capsule(root, mid, 2.6 * k, 2.8 * k),
                        Shape::capsule(mid, end, 2.8 * k, 3.0 * k),
                    ],
                    Region::Tail,
                    Frame::along(root, end, 3.0 * k),
                    ink,
                    true,
                    false,
                );
            }
            PartKind::TailCurl => {
                let mid = along(0.6);
                self.region(
                    &[Shape::capsule(root, mid, 1.6 * k, 1.4 * k)],
                    Region::Tail,
                    frame,
                    ink,
                    true,
                    false,
                );
                let loop_c = (mid.0 - 1.0, mid.1 - 2.0 * k);
                let ring = self
                    .sheet
                    .mask(&[Shape::ellipse(loop_c.0, loop_c.1, 3.0 * k, 3.0 * k)]);
                let hole = self
                    .sheet
                    .mask(&[Shape::ellipse(loop_c.0, loop_c.1, 1.2 * k, 1.2 * k)]);
                self.sheet.outline(&ring, OUTLINE);
                if self.sheet.hd() {
                    // The coat round the hole, a line just inside the hole, and nothing in it.
                    let res = self.sheet.res() as f32;
                    for (x, y) in ring.pixels() {
                        let dh = hole.distance(x, y);
                        let (fx, fy) = self.sheet.center(x, y);
                        let coat = self.hd_color(
                            Region::Tail,
                            frame,
                            ink,
                            false,
                            fx,
                            fy,
                            ring.depth(x, y),
                            false,
                        );
                        let color = mix(coat, OUTLINE, (0.5 - dh * res).clamp(0.0, 1.0));
                        let keep = ((dh + HD_OUTLINE) * res + 0.5).clamp(0.0, 1.0);
                        self.sheet.paint(x, y, color, ring.coverage(x, y) * keep);
                    }
                    return;
                }
                for (x, y) in ring.pixels() {
                    let c = if hole.has(x, y) {
                        if hole.edge(x, y) { OUTLINE } else { continue }
                    } else {
                        ink
                    };
                    self.sheet.set(x, y, c);
                }
            }
            PartKind::TailFluke => {
                let stalk_end = along(0.75);
                let base_r = (fig.body.ry * 0.45).max(2.0);
                self.region(
                    &[Shape::capsule(root, stalk_end, base_r, 1.4 * k)],
                    Region::Tail,
                    frame,
                    ink,
                    true,
                    false,
                );
                let (x, y) = stalk_end;
                let spread = 4.5 * k;
                let flick = sway * 1.5;
                let fluke = vec![
                    (x + 1.0, y - 0.8),
                    (x - spread * 0.9, y - spread * 1.1 + flick),
                    (x - spread * 0.55, y + flick * 0.5),
                    (x - spread * 0.9, y + spread * 1.0 + flick),
                    (x + 1.0, y + 0.8),
                ];
                self.region(
                    &[Shape::Polygon(fluke)],
                    Region::Fin,
                    frame,
                    ink,
                    true,
                    false,
                );
            }
            PartKind::TailFan => {
                let mut shapes = Vec::new();
                for (i, spread) in [-0.45_f32, 0.0, 0.45].into_iter().enumerate() {
                    let a = -0.6 + spread + tilt + sway * 0.08;
                    let (s, c) = a.sin_cos();
                    let tip = (root.0 - c * len * (1.0 - i as f32 * 0.05), root.1 - s * len);
                    shapes.push(Shape::capsule(root, tip, 1.6 * k, 2.2 * k));
                }
                self.region(&shapes, Region::Wing, frame, ink, true, false);
            }
            _ => {}
        }
    }

    fn ears(&mut self, front: bool) {
        let Some(part) = self.part(Slot::Ears) else {
            return;
        };
        let h = self.fig.head;
        let k = self.k(&part);
        let ink = self.ink(part.ink);
        let perk = self.pose.perk.min(2.0) * 0.5;
        let lift = f32::from(part.lift) * 0.08;
        let inner = if part.ink == Ink::Primary {
            Some(self.inks.accent)
        } else {
            None
        };
        // Floppy ears and great fans hang at the sides of the head; everything else stands on
        // top of it.
        match part.kind {
            PartKind::EarsFloppy => {
                if !front {
                    return;
                }
                for side in [-1.0_f32, 1.0] {
                    let near = side > 0.0;
                    let tilt = f32::from(part.tilt) * 0.15
                        + if near {
                            0.0
                        } else {
                            f32::from(part.uneven) * 0.2
                        };
                    let x = h.x + side * h.r * 0.85 + if near { 1.0 } else { -1.0 };
                    let y = h.y - h.r * 0.3 - lift * h.r;
                    let ear = Shape::turned(x, y + 3.0 * k, 2.4 * k, 5.0 * k, side * (0.3 + tilt));
                    self.region(
                        &[ear],
                        Region::Ear,
                        Frame::level(x, y + 3.0 * k, 2.4 * k, 5.0 * k),
                        ink,
                        true,
                        !near,
                    );
                }
                return;
            }
            PartKind::EarsFan => {
                if front {
                    return;
                }
                for (near, side) in [(false, -1.0_f32), (true, 1.0)] {
                    let tilt = f32::from(part.tilt) * 0.1
                        + if near {
                            0.0
                        } else {
                            f32::from(part.uneven) * 0.15
                        };
                    let (x, y, rx, ry) = if near {
                        (h.x + h.r * 0.9, h.y + h.r * 0.05, 4.2 * k, 6.0 * k)
                    } else {
                        (h.x - h.r * 0.85, h.y + h.r * 0.05, 5.0 * k, 6.6 * k)
                    };
                    let flap = self.pose.sway * 0.08;
                    let ear = Shape::turned(x, y - lift * h.r, rx, ry, side * (0.15 + tilt + flap));
                    let mask = self.region(
                        &[ear],
                        Region::Ear,
                        Frame::level(x, y, rx, ry),
                        ink,
                        true,
                        !near,
                    );
                    if let Some(inner) = inner {
                        let inside = self.sheet.mask(&[Shape::turned(
                            x + side * 0.8,
                            y + 0.5,
                            rx * 0.6,
                            ry * 0.65,
                            side * 0.15,
                        )]);
                        let tone = grain(inner);
                        let tone = if near { tone } else { far(tone) };
                        let pixels: Vec<(i32, i32)> = inside.pixels().collect();
                        for (px, py) in pixels {
                            if self.sheet.hd() {
                                let res = self.sheet.res() as f32;
                                let keep = ((mask.depth(px, py) - 0.6) * res + 0.5).clamp(0.0, 1.0);
                                self.sheet
                                    .paint(px, py, tone, inside.coverage(px, py) * keep);
                            } else if mask.has(px, py) && !mask.edge(px, py) {
                                self.sheet.set(px, py, tone);
                            }
                        }
                    }
                }
                return;
            }
            _ => {}
        }
        if front {
            return;
        }
        for near in [false, true] {
            let tilt = f32::from(part.tilt) * 0.12
                + if near {
                    0.0
                } else {
                    -f32::from(part.uneven) * 0.25
                };
            let angle = if near { 0.45 } else { -0.55 } + tilt;
            let (sin, cos) = angle.sin_cos();
            let reach = h.r * (0.82 - lift);
            let base = (h.x + sin * reach, h.y - cos * reach);
            let out = (sin, -cos);
            let (length, width) = match part.kind {
                PartKind::EarsRound => (3.0, 3.2),
                PartKind::EarsSmall => (2.0, 2.2),
                PartKind::EarsPointed => (6.0 + perk, 3.0),
                PartKind::EarsTufted => (6.5 + perk, 2.8),
                PartKind::EarsLong => (9.0 + perk, 2.3),
                _ => (4.0, 3.0),
            };
            let (length, width) = (length * k, width * k);
            let shape = match part.kind {
                PartKind::EarsRound | PartKind::EarsSmall => Shape::ellipse(
                    base.0 + out.0 * length * 0.4,
                    base.1 + out.1 * (length * 0.4 + perk * 0.5),
                    width,
                    width,
                ),
                PartKind::EarsLong => Shape::turned(
                    base.0 + out.0 * length * 0.55,
                    base.1 + out.1 * length * 0.55,
                    width,
                    length * 0.6,
                    angle,
                ),
                _ => {
                    let tip = (base.0 + out.0 * length, base.1 + out.1 * length);
                    let across = (cos, sin);
                    Shape::Polygon(vec![
                        (base.0 - across.0 * width, base.1 - across.1 * width),
                        tip,
                        (base.0 + across.0 * width, base.1 + across.1 * width),
                    ])
                }
            };
            let mask = self.region(
                &[shape],
                Region::Ear,
                Frame::level(base.0, base.1, width, length),
                ink,
                true,
                !near,
            );
            if let Some(inner) = inner
                && self.sheet.hd()
            {
                // The inside of the ear, softly inset from its edge.
                let color = if near { inner } else { far(inner) };
                let res = self.sheet.res() as f32;
                for (x, y) in mask.pixels() {
                    let (fx, fy) = self.sheet.center(x, y);
                    // Inset more at the base, where the ear meets the head.
                    let (dx, dy) = (fx - base.0, fy - base.1);
                    let up = (dx * out.0 + dy * out.1) / length.max(0.5);
                    let inset = 1.0 + (1.0 - up.clamp(0.0, 1.0)) * 0.4;
                    let keep = ((mask.depth(x, y) - inset) * res + 0.5).clamp(0.0, 1.0);
                    self.sheet.paint(x, y, color, keep);
                }
            } else if let Some(inner) = inner {
                let inside: Vec<(i32, i32)> = mask
                    .pixels()
                    .filter(|&(x, y)| {
                        !mask.edge(x, y)
                            && [(1, 0), (-1, 0), (0, -1)]
                                .into_iter()
                                .all(|(dx, dy)| !mask.edge(x + dx, y + dy))
                    })
                    .collect();
                let color = if near { inner } else { far(inner) };
                for (x, y) in inside {
                    self.sheet.set(x, y, color);
                }
            }
            if part.kind == PartKind::EarsTufted && self.sheet.hd() {
                // A fine wisp of dark fur from the tip.
                let from = (
                    base.0 + out.0 * (length - 0.5),
                    base.1 + out.1 * (length - 0.5),
                );
                let to = (
                    base.0 + out.0 * (length + 2.2) - 0.3,
                    base.1 + out.1 * (length + 2.2),
                );
                self.sheet
                    .blob(&[Shape::capsule(from, to, 0.55, 0.15)], OUTLINE);
            } else if part.kind == PartKind::EarsTufted {
                let tip = (
                    (base.0 + out.0 * (length + 1.5)).floor() as i32,
                    (base.1 + out.1 * (length + 1.5)).floor() as i32,
                );
                self.sheet.set(tip.0, tip.1, OUTLINE);
                self.sheet.set(tip.0, tip.1 - 1, OUTLINE);
            }
        }
    }

    fn snout(&mut self) {
        let Some(part) = self.part(Slot::Snout) else {
            return;
        };
        let k = self.k(&part);
        let (fx, fy) = self.fig.face;
        let h = self.fig.head;
        let ink = self.ink(part.ink);
        let tilt = f32::from(part.tilt) * 0.6;
        let lift = f32::from(part.lift) * 0.6;
        let m = (fx + 0.5 + tilt * 0.5, fy + h.r * 0.42 - lift);
        let nose = |painter: &mut Self, x: f32, y: f32| {
            if painter.sheet.hd() {
                // A soft dark nose with a glint of light on it.
                let (cx, cy) = (x.floor() + 1.0, y.floor() + 0.5);
                painter
                    .sheet
                    .blob(&[Shape::ellipse(cx, cy, 1.25, 0.8)], OUTLINE);
                painter.sheet.blob(
                    &[Shape::ellipse(cx - 0.35, cy - 0.3, 0.35, 0.22)],
                    light(OUTLINE),
                );
                return;
            }
            let (x, y) = (x.floor() as i32, y.floor() as i32);
            painter.sheet.set(x, y, OUTLINE);
            painter.sheet.set(x + 1, y, OUTLINE);
        };
        match part.kind {
            PartKind::SnoutButton => {
                self.region(
                    &[Shape::ellipse(m.0, m.1, 2.6 * k, 1.8 * k)],
                    Region::Snout,
                    Frame::level(m.0, m.1, 2.6 * k, 1.8 * k),
                    ink,
                    false,
                    false,
                );
                nose(self, m.0 - 0.5, m.1 - 1.5 * k);
                self.snout_top = (m.0, m.1 - 2.0 * k);
            }
            PartKind::SnoutMuzzle | PartKind::SnoutBroad => {
                let (rx, ry) = if part.kind == PartKind::SnoutBroad {
                    (5.0 * k, 3.2 * k)
                } else {
                    (3.8 * k, 2.6 * k)
                };
                let c = (m.0 + 0.5, m.1 + 0.3);
                self.region(
                    &[Shape::ellipse(c.0, c.1, rx, ry)],
                    Region::Snout,
                    Frame::level(c.0, c.1, rx, ry),
                    ink,
                    true,
                    false,
                );
                self.snout_top = (c.0 + rx * 0.4, c.1 - ry);
            }
            PartKind::SnoutLong => {
                let a = (m.0, m.1 - 0.5);
                let b = (m.0 + 6.0 * k + tilt, m.1 + 0.5 - lift * 0.3);
                self.region(
                    &[Shape::capsule(a, b, 2.8 * k, 2.0 * k)],
                    Region::Snout,
                    Frame::along(a, b, 2.4 * k),
                    ink,
                    true,
                    false,
                );
                nose(self, b.0 + 0.5, b.1 - 1.5 * k);
                self.snout_top = (b.0, b.1 - 2.0 * k);
            }
            PartKind::SnoutTrunk => {
                let sway = self.pose.sway * 0.6 + tilt;
                let p0 = (fx + h.r * 0.15, fy + h.r * 0.25);
                let p1 = (p0.0 + 2.5 * k + sway * 0.4, p0.1 + 5.0 * k - lift);
                let p2 = (p1.0 + 1.8 * k + sway, p1.1 + 4.0 * k - lift);
                let curl = (p2.0 + 1.5 * k, p2.1 - 0.5 * k);
                self.region(
                    &[
                        Shape::capsule(p0, p1, 3.0 * k, 2.3 * k),
                        Shape::capsule(p1, p2, 2.3 * k, 1.7 * k),
                        Shape::capsule(p2, curl, 1.7 * k, 1.5 * k),
                    ],
                    Region::Snout,
                    Frame::along(p0, p2, 2.5 * k),
                    self.ink(part.ink),
                    true,
                    false,
                );
                // A few folds across it.
                let fold = grain(shade(ink));
                if self.sheet.hd() {
                    let trunk = self.sheet.mask(&[
                        Shape::capsule(p0, p1, 3.0 * k, 2.3 * k),
                        Shape::capsule(p1, p2, 2.3 * k, 1.7 * k),
                        Shape::capsule(p2, curl, 1.7 * k, 1.5 * k),
                    ]);
                    let (dx, dy) = (p2.0 - p0.0, p2.1 - p0.1);
                    let length = (dx * dx + dy * dy).sqrt().max(0.5);
                    let across = (-dy / length * 3.0 * k, dx / length * 3.0 * k);
                    for t in [0.3_f32, 0.45, 0.6, 0.75, 0.9] {
                        let (x, y) = (p0.0 + dx * t, p0.1 + dy * t);
                        self.sheet.line(
                            (x - across.0, y - across.1),
                            (x + across.0, y + across.1),
                            fold,
                            &trunk,
                        );
                    }
                }
                for t in [0.35_f32, 0.6, 0.85] {
                    if self.sheet.hd() {
                        break;
                    }
                    let (x, y) = (p0.0 + (p2.0 - p0.0) * t, p0.1 + (p2.1 - p0.1) * t);
                    self.sheet.set(x.floor() as i32, y.floor() as i32, fold);
                    self.sheet.set(x.floor() as i32 + 1, y.floor() as i32, fold);
                }
                self.snout_top = (p0.0 + 2.0, p0.1);
            }
            PartKind::SnoutBeak | PartKind::SnoutHookedBeak => {
                let top = (fx - 0.5, fy + 0.5 - lift);
                let len = 4.5 * k;
                let mut points = vec![
                    (top.0 - 2.2 * k, top.1),
                    (top.0 + 2.6 * k, top.1 - 0.3),
                    (top.0 + len + tilt, top.1 + 2.2 * k),
                ];
                if part.kind == PartKind::SnoutHookedBeak {
                    points.push((top.0 + len + tilt - 0.4, top.1 + 3.6 * k));
                    points.push((top.0 + len * 0.6, top.1 + 2.8 * k));
                }
                points.push((top.0 + 0.4, top.1 + 3.0 * k));
                points.push((top.0 - 1.8 * k, top.1 + 1.6 * k));
                let beak = self.ink(part.ink);
                self.sheet.flat(&[Shape::Polygon(points)], beak);
                let (x, y) = (top.0.floor() as i32, (top.1 + 1.5 * k).floor() as i32);
                if self.sheet.hd() {
                    // The line of the mouth along the beak.
                    let (x, y) = (x as f32, y as f32 + 0.5);
                    self.sheet.blob(
                        &[Shape::capsule(
                            (x + 0.6, y),
                            (x + 3.2 * k, y + 0.5),
                            0.3,
                            0.2,
                        )],
                        shade(beak),
                    );
                } else {
                    self.sheet.set(x + 1, y, shade(beak));
                    self.sheet.set(x + 2, y, shade(beak));
                }
                self.snout_top = (top.0 + 2.0, top.1);
            }
            PartKind::SnoutRostrum => {
                let a = (fx + h.r * 0.35, fy + h.r * 0.38 - lift);
                let b = (a.0 + 5.0 * k + tilt, a.1 + 0.6);
                self.region(
                    &[Shape::capsule(a, b, 2.3 * k, 1.6 * k)],
                    Region::Snout,
                    Frame::along(a, b, 2.0 * k),
                    ink,
                    true,
                    false,
                );
                self.snout_top = (b.0, b.1 - 1.5);
            }
            _ => {}
        }
    }

    fn horns(&mut self, front: bool) {
        let Some(part) = self.part(Slot::Horns) else {
            return;
        };
        let k = self.k(&part);
        let h = self.fig.head;
        let ink = self.ink(part.ink);
        let tilt = f32::from(part.tilt) * 0.15;
        let nose = matches!(part.kind, PartKind::HornsNose | PartKind::HornsDoubleNose);
        if nose != front {
            return;
        }
        if nose {
            let (x, y) = self.snout_top;
            let mut horns = vec![(x, y, 6.0 * k)];
            if part.kind == PartKind::HornsDoubleNose {
                horns.push((x - 3.0 * k, y - 1.0, 3.5 * k));
            }
            for (x, y, height) in horns.into_iter().rev() {
                let lean = tilt * height;
                self.sheet.flat(
                    &[Shape::Polygon(vec![
                        (x - 2.0 * k, y + 1.0),
                        (x - 0.5 * k + lean - height * 0.2, y - height),
                        (x + 2.2 * k, y + 1.0),
                    ])],
                    ink,
                );
            }
            return;
        }
        for near in [false, true] {
            let side = if near { 1.0_f32 } else { -1.0 };
            let skew = if near {
                0.0
            } else {
                f32::from(part.uneven) * 0.2
            };
            let base = (
                h.x + side * h.r * 0.38 + 0.5,
                h.y - h.r * (0.82 - f32::from(part.lift) * 0.05),
            );
            let tone = if near { ink } else { far(ink) };
            match part.kind {
                PartKind::HornsNubs => {
                    self.sheet.flat(
                        &[Shape::ellipse(base.0, base.1 - 1.0 * k, 1.6 * k, 1.8 * k)],
                        tone,
                    );
                }
                PartKind::HornsSwept => {
                    let a = -0.5 + tilt + skew;
                    let (s, c) = a.sin_cos();
                    let mid = (base.0 + s * 3.5 * k, base.1 - c * 3.5 * k);
                    let tip = (mid.0 - 2.5 * k, mid.1 - 1.0 * k);
                    self.sheet.flat(
                        &[
                            Shape::capsule(base, mid, 1.7 * k, 1.2 * k),
                            Shape::capsule(mid, tip, 1.2 * k, 0.6),
                        ],
                        tone,
                    );
                }
                PartKind::HornsOssicones => {
                    let a = side * 0.15 + tilt + skew;
                    let (s, c) = a.sin_cos();
                    let tip = (base.0 + s * 4.5 * k, base.1 - c * 4.5 * k);
                    self.sheet
                        .flat(&[Shape::capsule(base, tip, 1.1 * k, 1.0 * k)], tone);
                    let knob = if near {
                        self.inks.secondary
                    } else {
                        far(self.inks.secondary)
                    };
                    self.sheet
                        .flat(&[Shape::ellipse(tip.0, tip.1, 1.6 * k, 1.5 * k)], knob);
                }
                PartKind::HornsAntlers => {
                    let a = side * 0.35 + tilt + skew;
                    let (s, c) = a.sin_cos();
                    let tip = (base.0 + s * 6.0 * k, base.1 - c * 6.5 * k);
                    let mid = ((base.0 + tip.0) / 2.0, (base.1 + tip.1) / 2.0);
                    let branch = (mid.0 + side * 3.0 * k, mid.1 - 1.5 * k);
                    let back = (tip.0 - 2.5 * k, tip.1 - 1.0);
                    self.sheet.flat(
                        &[
                            Shape::capsule(base, tip, 1.0 * k, 0.7),
                            Shape::capsule(mid, branch, 0.8, 0.6),
                            Shape::capsule(tip, back, 0.7, 0.6),
                        ],
                        tone,
                    );
                }
                _ => {}
            }
        }
    }

    fn tusks(&mut self) {
        let Some(part) = self.part(Slot::Tusks) else {
            return;
        };
        let k = self.k(&part);
        let (fx, fy) = self.fig.face;
        let h = self.fig.head;
        let long = part.kind == PartKind::TusksLong;
        let ink = self.ink(part.ink);
        for near in [false, true] {
            let side = if near { 1.0 } else { -1.0 };
            let root = (fx + side * (self.eye_dx + 0.5), fy + h.r * 0.45);
            let (dx, dy) = if long { (3.0, 5.0) } else { (1.5, 2.8) };
            let tilt = f32::from(part.tilt) * 0.5
                + if near {
                    0.0
                } else {
                    f32::from(part.uneven) * 0.5
                };
            let mid = (root.0 + (dx * 0.4 + tilt * 0.3) * k, root.1 + dy * 0.7 * k);
            let tip = (
                root.0 + (dx + tilt) * k,
                root.1 + dy * k - if long { 1.5 * k } else { 0.0 },
            );
            let tone = if near { ink } else { far(ink) };
            self.sheet.flat(
                &[
                    Shape::capsule(root, mid, 1.2 * k, 1.0 * k),
                    Shape::capsule(mid, tip, 1.0 * k, 0.6),
                ],
                tone,
            );
        }
    }

    fn mane_behind(&mut self, part: Part) {
        let k = self.k(&part);
        let h = self.fig.head;
        let ink = self.ink(part.ink);
        let lift = f32::from(part.lift);
        let mut points = Vec::new();
        let (cx, cy) = (h.x - 0.8, h.y + 0.5 - lift * 0.5);
        let shaggy = part.kind == PartKind::ManeShaggy;
        let spikes = if shaggy { 18 } else { 14 };
        let r = h.r * (1.05 + 0.25 * k);
        for i in 0..spikes * 2 {
            let a = i as f32 / (spikes * 2) as f32 * std::f32::consts::TAU;
            let rr = if i % 2 == 0 { r } else { r * 0.84 };
            let (stretch_x, stretch_y) = if shaggy { (1.1, 1.25) } else { (1.0, 1.0) };
            points.push((
                cx + a.cos() * rr * stretch_x,
                cy + a.sin() * rr * stretch_y + if shaggy && a.sin() > 0.0 { 2.0 } else { 0.0 },
            ));
        }
        self.region(
            &[Shape::Polygon(points)],
            Region::Mane,
            Frame::level(cx, cy, r, r),
            ink,
            true,
            false,
        );
    }

    fn crest_along(&mut self, part: Part, a: (f32, f32), b: (f32, f32), r: f32) {
        let k = self.k(&part);
        let ink = self.ink(part.ink);
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let length = (dx * dx + dy * dy).sqrt().max(1.0);
        // The back of the neck is to the left of the way it runs.
        let back = (dy / length, -dx / length);
        let back = if back.0 > 0.0 {
            (-back.0, -back.1)
        } else {
            back
        };
        let mut shapes = Vec::new();
        let count = (length / 3.0).ceil() as i32;
        for i in 0..count {
            let t = 0.15 + 0.75 * i as f32 / count.max(1) as f32;
            let p = (
                a.0 + dx * t + back.0 * (r - 0.5),
                a.1 + dy * t + back.1 * (r - 0.5),
            );
            shapes.push(Shape::ellipse(
                p.0 + back.0 * 0.8,
                p.1 + back.1 * 0.8,
                1.3 * k,
                1.3 * k,
            ));
        }
        self.region(
            &shapes,
            Region::Mane,
            Frame::along(a, b, r),
            ink,
            true,
            false,
        );
        // Paint the neck back over the crest's inner edge so it grows out of the neck.
        let neck = self
            .sheet
            .mask(&[Shape::capsule(a, b, r - 1.0, r * 0.9 - 1.0)]);
        if self.sheet.hd() {
            for (x, y) in neck.pixels() {
                let alpha = neck.coverage(x, y) * self.sheet.inked(x, y);
                let (fx, fy) = self.sheet.center(x, y);
                let color = self.hd_color(
                    Region::Neck,
                    Frame::along(a, b, r),
                    self.inks.primary,
                    true,
                    fx,
                    fy,
                    neck.depth(x, y) + 1.0,
                    false,
                );
                self.sheet.paint(x, y, color, alpha);
            }
            return;
        }
        for (x, y) in neck.pixels() {
            if self.sheet.get(x, y) == OUTLINE {
                self.sheet.set(x, y, self.inks.primary);
            }
        }
    }

    fn tuft(&mut self, part: Part) {
        let k = self.k(&part);
        let h = self.fig.head;
        let ink = self.ink(part.ink);
        let top = (
            h.x - 0.5 + f32::from(part.tilt) * 0.5,
            h.y - h.r * 0.9 - f32::from(part.lift) * 0.5,
        );
        let mut points = Vec::new();
        for (i, (dx, height)) in [(-3.0_f32, 2.5_f32), (-1.0, 4.5), (1.0, 3.0), (3.0, 2.0)]
            .into_iter()
            .enumerate()
        {
            if i == 0 {
                points.push((top.0 + dx * k - 1.0, top.1 + 1.5));
            }
            points.push((top.0 + dx * k, top.1 - height * k));
            points.push((top.0 + (dx + 1.0) * k, top.1 + 0.5));
        }
        points.push((top.0 + 4.0 * k, top.1 + 1.5));
        self.region(
            &[Shape::Polygon(points)],
            Region::Mane,
            Frame::level(top.0, top.1, 4.0 * k, 3.0 * k),
            ink,
            true,
            false,
        );
    }

    /// The height of the top of the body at `x`.
    fn back_at(&self, x: f32) -> f32 {
        let b = self.fig.body;
        let u = ((x - b.cx) / b.rx).clamp(-1.0, 1.0);
        b.cy - b.ry * (1.0 - u * u).sqrt()
    }

    fn back(&mut self, behind: bool) {
        let Some(part) = self.part(Slot::Back) else {
            return;
        };
        let k = self.k(&part);
        let b = self.fig.body;
        let ink = self.ink(part.ink);
        let tilt = f32::from(part.tilt) * 0.8;
        let along = f32::from(part.lift) * b.rx * 0.08;
        match (part.kind, behind) {
            (PartKind::BackDorsal | PartKind::BackTallDorsal | PartKind::BackTinyDorsal, true) => {
                let height = match part.kind {
                    PartKind::BackTallDorsal => 9.5,
                    PartKind::BackTinyDorsal => 2.5,
                    _ => 5.0,
                } * k;
                let x = b.cx - b.rx * 0.15 + along;
                let width = height.max(4.0) * 0.55 + 1.5;
                let y = self.back_at(x) + 1.5;
                let tall = part.kind == PartKind::BackTallDorsal;
                let tip = (x - if tall { 1.0 } else { width * 0.7 } - tilt, y - height);
                self.region(
                    &[Shape::Polygon(vec![
                        (x + width * 0.6, y + 1.0),
                        (x + width * 0.1, y - height * 0.6),
                        tip,
                        (x - width * 0.5, y - height * 0.25),
                        (x - width * 0.6, y + 1.0),
                    ])],
                    Region::Fin,
                    Frame::level(x, y - height / 2.0, width, height),
                    ink,
                    true,
                    false,
                );
            }
            (PartKind::BackHump, true) => {
                let x = b.cx - b.rx * 0.05 + along;
                let y = self.back_at(x) + 1.0;
                self.region(
                    &[Shape::ellipse(x, y, 4.5 * k, 3.5 * k)],
                    Region::Hump,
                    Frame::level(x, y, 4.5 * k, 3.5 * k),
                    ink,
                    true,
                    false,
                );
            }
            (PartKind::BackRidge, false) => {
                let mut shapes = Vec::new();
                let count = 5;
                for i in 0..count {
                    let x = b.cx - b.rx * 0.6 + along + i as f32 * b.rx * 1.1 / count as f32;
                    let y = self.back_at(x) + 0.5;
                    shapes.push(Shape::Polygon(vec![
                        (x - 1.6 * k, y + 0.5),
                        (x - 0.4 * k - tilt * 0.3, y - 3.0 * k),
                        (x + 1.6 * k, y + 0.5),
                    ]));
                }
                let accent = self.ink(part.ink);
                self.sheet.flat(&shapes, accent);
            }
            _ => {}
        }
    }

    fn fins(&mut self, near: bool, flippers: bool) {
        let Some(part) = self.part(Slot::Fins) else {
            return;
        };
        let k = self.k(&part);
        let fig = self.fig;
        let b = fig.body;
        let ink = self.ink(part.ink);
        let paddle = self.pose.paddle * 0.25
            + f32::from(part.tilt) * 0.12
            + if near {
                0.0
            } else {
                f32::from(part.uneven) * 0.15
            };
        if flippers {
            // A crawler's front limbs are flippers.
            for leg in fig.legs.iter().filter(|leg| leg.front && leg.near == near) {
                let root = leg.top;
                let a = 0.9 + paddle;
                let (s, c) = a.sin_cos();
                let len = 7.5 * k;
                let tip = (root.0 + c * len, root.1 + s * len * 0.6 + 1.5);
                self.region(
                    &[Shape::capsule(root, tip, 2.4 * k, 1.4 * k)],
                    Region::Fin,
                    Frame::along(root, tip, 2.0 * k),
                    ink,
                    true,
                    !near,
                );
            }
            return;
        }
        let root = (
            b.cx + b.rx * if near { 0.3 } else { 0.38 },
            b.cy + b.ry * 0.45 - if near { 0.0 } else { 1.0 } - f32::from(part.lift) * 0.5,
        );
        let (len, width, angle) = match part.kind {
            PartKind::FinsLong => (11.0, 1.8, 2.6),
            PartKind::FinsFlippers => (6.0, 2.4, 2.2),
            _ => (6.0, 2.0, 2.3),
        };
        let a = angle - paddle;
        let (s, c) = a.sin_cos();
        let tip = (root.0 + c * len * k, root.1 + s * len * k * 0.55);
        let shape = Shape::capsule(root, tip, width * k * 1.2, width * k * 0.55);
        self.region(
            &[shape],
            Region::Fin,
            Frame::along(root, tip, width * k),
            ink,
            true,
            !near,
        );
        if part.kind == PartKind::FinsLong {
            // A humpback's knobbly leading edge.
            let pale = self.inks.underside;
            for t in [0.3_f32, 0.55, 0.8] {
                let (x, y) = (root.0 + (tip.0 - root.0) * t, root.1 + (tip.1 - root.1) * t);
                if self.sheet.hd() {
                    let (x, y) = (x.floor() + 0.5, (y - width * k * 0.6).floor() + 0.5);
                    self.sheet.blob(&[Shape::ellipse(x, y, 0.65, 0.5)], pale);
                    continue;
                }
                self.sheet
                    .set(x.floor() as i32, (y - width * k * 0.6).floor() as i32, pale);
            }
        }
    }

    fn wings(&mut self, near: bool) {
        let Some(part) = self.part(Slot::Wings) else {
            return;
        };
        let k = self.k(&part);
        let b = self.fig.body;
        let ink = self.ink(part.ink);
        let spread = match part.kind {
            PartKind::WingsSpread => 0.55 + self.pose.spread * 0.45,
            _ => self.pose.spread,
        };
        let tilt = f32::from(part.tilt) * 0.1
            + if near {
                0.0
            } else {
                f32::from(part.uneven) * 0.15
            };
        let upright = matches!(self.fig.plan, Plan::Percher | Plan::Upright);
        let shoulder = if upright {
            (b.cx + b.rx * 0.1, b.cy - b.ry * 0.45)
        } else {
            (b.cx + b.rx * 0.15, b.cy - b.ry * 0.55)
        };
        let shoulder = (
            shoulder.0 - if near { 0.0 } else { 2.0 },
            shoulder.1 - f32::from(part.lift) * 0.5,
        );
        if part.kind == PartKind::WingsTiny {
            let c = (shoulder.0 - 2.0, shoulder.1 - 2.0 - spread * 2.0);
            self.region(
                &[Shape::turned(
                    c.0,
                    c.1,
                    3.4 * k,
                    2.4 * k,
                    -0.6 - spread * 0.5 - tilt,
                )],
                Region::Wing,
                Frame::level(c.0, c.1, 3.4 * k, 2.4 * k),
                ink,
                true,
                !near,
            );
            return;
        }
        if spread < 0.3 {
            if !near {
                return;
            }
            // Folded along the side.
            let c = if upright {
                (b.cx - b.rx * 0.2, b.cy + b.ry * 0.05)
            } else {
                (b.cx - b.rx * 0.1, b.cy - b.ry * 0.05)
            };
            let (rx, ry) = if upright {
                (b.rx * 0.8 * k, b.ry * 0.7)
            } else {
                (b.rx * 0.6 * k, b.ry * 0.55)
            };
            let angle = if upright { 1.1 } else { 0.25 } + tilt;
            let mask = self.region(
                &[Shape::turned(c.0, c.1, ry.max(rx), rx.min(ry), angle)],
                Region::Wing,
                Frame::level(c.0, c.1, rx, ry),
                ink,
                true,
                false,
            );
            // The line of the flight feathers.
            let line = shade(ink);
            let (s, cth) = angle.sin_cos();
            let reach = ry.max(rx);
            for i in 1..=2 {
                let off = i as f32 * 1.4;
                self.sheet.line(
                    (
                        c.0 - cth * reach * 0.1 - s * off,
                        c.1 - s * reach * 0.1 + cth * off,
                    ),
                    (
                        c.0 - cth * reach * 0.9 - s * off,
                        c.1 - s * reach * 0.9 + cth * off * 0.6,
                    ),
                    line,
                    &mask,
                );
            }
            return;
        }
        // Spread: up and back from the shoulder, feathers fanned at the end.
        let lift = (spread - 0.3) / 0.7;
        let a = 1.0 + lift * 0.9 - tilt;
        let (s, c) = a.sin_cos();
        let reach = 11.0 * k;
        let elbow = (shoulder.0 - c * reach * 0.5, shoulder.1 - s * reach * 0.5);
        let tip = (shoulder.0 - c * reach * 1.05, shoulder.1 - s * reach * 0.95);
        let mut shapes = vec![Shape::capsule(shoulder, elbow, 2.6 * k, 2.4 * k)];
        for (i, back) in [0.0_f32, 0.5, 1.0].into_iter().enumerate() {
            let end = (
                tip.0 + back * 3.0 * k + i as f32 * 0.5,
                tip.1 + back * 4.5 * k,
            );
            shapes.push(Shape::capsule(elbow, end, 2.2 * k, 1.3 * k));
        }
        self.region(
            &shapes,
            Region::Wing,
            Frame::along(shoulder, tip, 3.0 * k),
            ink,
            true,
            !near,
        );
    }

    fn shell(&mut self) {
        let b = self.fig.body;
        let part = self.part(Slot::Shell);
        let flat = part.is_some_and(|p| p.kind == PartKind::ShellFlat);
        let (k, ink) = match part {
            Some(part) => (self.k(&part) / self.fig.scale, self.ink(part.ink)),
            None => return,
        };
        let rx = b.rx * (0.95 + 0.1 * k);
        let ry = b.ry * if flat { 0.7 } else { 1.0 } * (0.9 + 0.12 * k);
        let cy = b.cy - b.ry * 0.05;
        let cut = b.cy + b.ry * 0.4;
        let mut points = Vec::new();
        for i in 0..=24 {
            let a = std::f32::consts::PI * (1.0 + i as f32 / 24.0);
            points.push((b.cx + a.cos() * rx, (cy + a.sin() * ry).min(cut)));
        }
        points.push((b.cx + rx, cut));
        points.push((b.cx - rx, cut));
        let frame = Frame::level(b.cx, cy, rx, ry);
        let shell = Shape::Polygon(points);
        let mask = self.sheet.mask(&[shell]);
        self.sheet.outline(&mask, OUTLINE);
        let pixels: Vec<(i32, i32)> = mask.pixels().collect();
        let rim = light(ink);
        let hd = self.sheet.hd();
        for &(x, y) in &pixels {
            let (fx, fy) = self.sheet.center(x, y);
            let (u, v) = frame.at(fx, fy);
            let mut color = if hd {
                mix(ink, rim, smoothstep(cut - 1.8, cut - 1.2, fy))
            } else if fy >= cut - 1.5 {
                rim
            } else {
                ink
            };
            for marking in &self.sculpt.markings {
                if matches!(marking.kind, MarkingKind::Mottle | MarkingKind::Spots)
                    && self.marked(marking, Region::Body, u, v, fx, fy, false)
                {
                    color = soft(marking.color);
                }
            }
            if hd {
                // A soft dome of light, and the shell darkening toward its sides.
                let glint = (u + 0.35).powi(2) + (v + 0.7).powi(2);
                color = mix(color, light(color), 1.0 - smoothstep(0.02, 0.09, glint));
                let rim_shade = (1.0 - (mask.depth(x, y) / 1.2).min(1.0)) * 0.4;
                color = mix(
                    color,
                    shade(color),
                    rim_shade + smoothstep(0.2, 1.0, u) * 0.2,
                );
                let alpha = mask.coverage(x, y);
                self.sheet.paint(x, y, color, alpha);
                continue;
            }
            if (u + 0.35).powi(2) + (v + 0.7).powi(2) < 0.04 {
                color = light(color);
            }
            self.sheet.set(x, y, color);
        }
        // Scutes: a row along the middle and lines down to the rim.
        let line = shade(ink);
        let top = cy - ry;
        let mid = (top + cut) / 2.0 - 0.5;
        for u in [-0.55_f32, -0.15, 0.25, 0.62] {
            let x = b.cx + u * rx;
            self.sheet
                .line((x, mid), (x + u * 1.5, cut - 2.0), line, &mask);
        }
        self.sheet
            .line((b.cx - rx * 0.75, mid), (b.cx + rx * 0.8, mid), line, &mask);
        for u in [-0.35_f32, 0.05, 0.45] {
            let x = b.cx + u * rx;
            let y = cy - ry * (1.0 - (u * 1.1).powi(2)).max(0.0).sqrt() + 2.0;
            self.sheet.line((x, y), (x, mid), line, &mask);
        }
        let _ = pixels;
    }
}

/// The grain a coat's treatment gives a pixel, if any.
fn texture(
    treatment: Treatment,
    region: Region,
    u: f32,
    v: f32,
    x: i32,
    y: i32,
    color: Rgba,
) -> Option<Rgba> {
    let large = matches!(
        region,
        Region::Body | Region::Head | Region::Neck | Region::Mane | Region::Hump | Region::Wing
    );
    match treatment {
        Treatment::Fur => (large && v > -0.6 && v < 0.45 && hash(x, y, 3).is_multiple_of(29))
            .then(|| grain(color)),
        Treatment::Shaggy => {
            (large && v > -0.7 && (hash(x, y, 5).is_multiple_of(9) || (x + y * 2) % 11 == 0))
                .then(|| grain(color))
        }
        Treatment::Feathers => {
            let feathered = matches!(region, Region::Body | Region::Wing | Region::Neck);
            let row = y.rem_euclid(3);
            let col = (x + (y / 3) * 2).rem_euclid(4);
            (feathered && v > -0.5 && row == 0 && col != 0).then(|| grain(color))
        }
        Treatment::Plated => {
            let folded = match region {
                Region::Body => [-0.45_f32, 0.05, 0.5]
                    .into_iter()
                    .any(|fold| (u - fold - v * 0.12).abs() < 0.045),
                Region::Leg { .. } => (u - 0.55).abs() < 0.06 || (u - 0.3).abs() < 0.05,
                _ => false,
            };
            folded.then(|| grain(shade(color)))
        }
        Treatment::Smooth => None,
    }
}

/// The grain a coat's treatment gives a point in high definition, in frame pixels, and how
/// strongly: fine strands of fur, longer locks for shaggy fur, rows of rounded feathers and the
/// folds of thick skin, each with soft edges.
fn texture_hd(
    treatment: Treatment,
    region: Region,
    u: f32,
    v: f32,
    x: f32,
    y: f32,
    color: Rgba,
) -> Option<(Rgba, f32)> {
    let large = matches!(
        region,
        Region::Body | Region::Head | Region::Neck | Region::Mane | Region::Hump | Region::Wing
    );
    // The nearest of the short strokes scattered over a grid of `cell`, each `length` long and
    // lying down and back: how far the point is from it.
    let strands = |cell: f32, chance: u32, length: f32, seed: u32| {
        let (gx, gy) = ((x / cell).floor() as i32, (y / cell).floor() as i32);
        let mut nearest = f32::MAX;
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (cx, cy) = (gx + dx, gy + dy);
                let h = hash(cx, cy, seed);
                if h % 100 >= chance {
                    continue;
                }
                let px = (cx as f32 + ((h >> 8) % 100) as f32 / 100.0) * cell;
                let py = (cy as f32 + ((h >> 16) % 100) as f32 / 100.0) * cell;
                let tilt = ((h >> 24) % 40) as f32 / 100.0 - 0.2;
                let (dirx, diry) = (-0.45 + tilt, 0.9);
                let norm = (dirx * dirx + diry * diry).sqrt();
                let (dirx, diry) = (dirx / norm * length, diry / norm * length);
                let t = (((x - px) * dirx + (y - py) * diry) / (length * length)).clamp(0.0, 1.0);
                let d = ((x - px - dirx * t).powi(2) + (y - py - diry * t).powi(2)).sqrt();
                nearest = nearest.min(d);
            }
        }
        nearest
    };
    match treatment {
        Treatment::Fur => {
            if !large || v <= -0.75 || v >= 0.55 {
                return None;
            }
            let d = strands(1.5, 40, 0.9, 3);
            let amount = 1.0 - smoothstep(0.1, 0.24, d);
            (amount > 0.0).then(|| (grain(color), amount * 0.85))
        }
        Treatment::Shaggy => {
            if !large || v <= -0.85 {
                return None;
            }
            let d = strands(1.1, 75, 1.7, 5);
            let amount = 1.0 - smoothstep(0.12, 0.26, d);
            (amount > 0.0).then(|| (grain(color), amount))
        }
        Treatment::Feathers => {
            let feathered = matches!(region, Region::Body | Region::Wing | Region::Neck);
            if !feathered || v <= -0.55 {
                return None;
            }
            // Overlapping rounded feathers, each row set half a feather along from the last.
            let (w, h) = (2.2_f32, 1.6_f32);
            let row = (y / h).floor();
            let shift = if (row as i32).rem_euclid(2) == 0 {
                0.0
            } else {
                w / 2.0
            };
            let lx = (x + shift).rem_euclid(w) - w / 2.0;
            let ly = y - row * h;
            let r = (lx * lx + (ly + 0.2).powi(2)).sqrt();
            let amount =
                (1.0 - smoothstep(0.1, 0.25, (r - 1.15).abs())) * smoothstep(0.35, 0.7, ly);
            (amount > 0.0).then(|| (grain(color), amount))
        }
        Treatment::Plated => {
            let distance = match region {
                Region::Body => [-0.45_f32, 0.05, 0.5]
                    .into_iter()
                    .map(|fold| (u - fold - v * 0.12).abs() - 0.045)
                    .fold(f32::MAX, f32::min),
                Region::Leg { .. } => ((u - 0.55).abs() - 0.06).min((u - 0.3).abs() - 0.05),
                _ => return None,
            };
            let amount = 1.0 - smoothstep(-0.02, 0.025, distance);
            (amount > 0.0).then(|| (grain(shade(color)), amount))
        }
        Treatment::Smooth => None,
    }
}
