//! The hand's glyphs, rasterised with a hard threshold and packed into one texture.

use egui::Color32;
use skrifa::MetadataProvider as _;
use std::collections::HashMap;
use vello_cpu::{color, kurbo};

/// One of Kalam's two weights, read once.
pub(crate) struct Face {
    charmap: skrifa::charmap::Charmap<'static>,
    outlines: skrifa::outline::OutlineGlyphCollection<'static>,
    metrics: skrifa::metrics::GlyphMetrics<'static>,
    units_per_em: f32,
}

impl Face {
    pub(crate) fn new(data: &'static [u8]) -> Self {
        let font = skrifa::FontRef::new(data).expect("Kalam is a font");
        let location = skrifa::instance::LocationRef::default();
        Self {
            charmap: font.charmap(),
            outlines: font.outline_glyphs(),
            metrics: font.glyph_metrics(skrifa::instance::Size::unscaled(), location),
            units_per_em: f32::from(
                font.metrics(skrifa::instance::Size::unscaled(), location)
                    .units_per_em,
            ),
        }
    }

    /// How far a glyph moves the pen on, in ems.
    pub(crate) fn advance(&self, glyph: skrifa::GlyphId) -> f32 {
        self.metrics.advance_width(glyph).unwrap_or(0.0) / self.units_per_em
    }

    /// Kalam's glyph for `chr`, if it has one; egui draws anything else from a fallback font.
    pub(crate) fn glyph(&self, chr: char) -> Option<skrifa::GlyphId> {
        self.charmap
            .map(chr)
            .filter(|id| *id != skrifa::GlyphId::NOTDEF)
    }
}

/// A glyph in the atlas: where it is, how big, and where its top left sits from its pen
/// position, in texels, down and to the right.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Placed {
    pub(crate) at: [u16; 2],
    pub(crate) size: [u16; 2],
    pub(crate) offset: [i32; 2],
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
struct Key {
    face: u8,
    glyph: u32,
    /// Texels to the em, in sixteenths.
    em: u32,
    /// The least coverage, of 255, a texel needs to be drawn.
    threshold: u8,
}

const WIDTH: usize = 1024;
const MOST_HEIGHT: usize = 4096;
/// A texel left clear round every glyph, so nearest sampling never picks up a neighbour's.
const PAD: usize = 1;

/// Every glyph drawn so far, on one texture that grows taller as it fills.
pub(crate) struct Atlas {
    pixels: Vec<Color32>,
    height: usize,
    placed: HashMap<Key, Option<Placed>>,
    /// The shelf being filled: its top, its height so far, and how far along it is.
    shelf: (usize, usize, usize),
    /// What has changed since the texture was last sent: the rows, or all of it after growing.
    pub(crate) dirty: Option<(usize, usize)>,
    pub(crate) grown: bool,
}

impl Atlas {
    pub(crate) fn new() -> Self {
        let height = 256;
        Self {
            pixels: vec![Color32::TRANSPARENT; WIDTH * height],
            height,
            placed: HashMap::new(),
            shelf: (0, 0, 0),
            dirty: None,
            grown: true,
        }
    }

    pub(crate) fn size(&self) -> [usize; 2] {
        [WIDTH, self.height]
    }

    /// The whole atlas, for sending to the texture.
    pub(crate) fn image(&self) -> egui::ColorImage {
        egui::ColorImage::new([WIDTH, self.height], self.pixels.clone())
    }

    /// Rows `from..to` of the atlas, for sending only what changed.
    pub(crate) fn rows(&self, from: usize, to: usize) -> egui::ColorImage {
        egui::ColorImage::new(
            [WIDTH, to - from],
            self.pixels[from * WIDTH..to * WIDTH].to_vec(),
        )
    }

    /// `glyph` of `face`, at `em` texels to the em, with every texel at least `threshold` covered
    /// drawn solid and the rest left out. `None` for a glyph with nothing to draw (a space), or
    /// once the atlas is full.
    pub(crate) fn glyph(
        &mut self,
        faces: &[Face],
        face: usize,
        glyph: skrifa::GlyphId,
        em: f32,
        threshold: u8,
    ) -> Option<Placed> {
        let key = Key {
            face: face as u8,
            glyph: glyph.to_u32(),
            em: (em * 16.0).round() as u32,
            threshold,
        };
        if let Some(placed) = self.placed.get(&key) {
            return *placed;
        }
        let placed = raster(&faces[face], glyph, key.em as f32 / 16.0, threshold)
            .and_then(|(mask, size, offset)| self.pack(&mask, size, offset));
        self.placed.insert(key, placed);
        placed
    }

    fn pack(&mut self, mask: &[bool], [w, h]: [usize; 2], offset: [i32; 2]) -> Option<Placed> {
        if w + 2 * PAD > WIDTH {
            return None;
        }
        let (mut top, mut tall, mut along) = self.shelf;
        if along + w + 2 * PAD > WIDTH {
            top += tall;
            tall = 0;
            along = 0;
        }
        while top + h + 2 * PAD > self.height {
            if self.height >= MOST_HEIGHT {
                return None;
            }
            self.height *= 2;
            self.pixels
                .resize(WIDTH * self.height, Color32::TRANSPARENT);
            self.grown = true;
        }
        let (x, y) = (along + PAD, top + PAD);
        for row in 0..h {
            for col in 0..w {
                if mask[row * w + col] {
                    self.pixels[(y + row) * WIDTH + x + col] = Color32::WHITE;
                }
            }
        }
        self.shelf = (top, tall.max(h + 2 * PAD), along + w + 2 * PAD);
        let (from, to) = self.dirty.unwrap_or((y, y + h));
        self.dirty = Some((from.min(y), to.max(y + h)));
        Some(Placed {
            at: [x as u16, y as u16],
            size: [w as u16, h as u16],
            offset,
        })
    }
}

/// `glyph` drawn at `em` texels to the em, thresholded: which texels are drawn, how many across
/// and down, and where the top left texel sits from the pen, y down.
fn raster(
    face: &Face,
    glyph: skrifa::GlyphId,
    em: f32,
    threshold: u8,
) -> Option<(Vec<bool>, [usize; 2], [i32; 2])> {
    let outline = face.outlines.get(glyph)?;
    let mut path = kurbo::BezPath::new();
    let settings = skrifa::outline::DrawSettings::unhinted(
        skrifa::instance::Size::new(em),
        skrifa::instance::LocationRef::default(),
    );
    outline.draw(settings, &mut Pen(&mut path)).ok()?;
    let bounds = kurbo::Shape::bounding_box(&path).expand();
    let (w, h) = (bounds.width() as usize, bounds.height() as usize);
    if w == 0 || h == 0 || w > u16::MAX as usize || h > u16::MAX as usize {
        return None;
    }
    let mut context = vello_cpu::RenderContext::new(w as u16, h as u16);
    context.set_transform(kurbo::Affine::translate((-bounds.x0, -bounds.y0)));
    context.set_paint(color::OpaqueColor::<color::Srgb>::WHITE);
    context.fill_path(&path);
    let mut pixmap = vello_cpu::Pixmap::new(w as u16, h as u16);
    let mut resources = vello_cpu::Resources::new();
    context.render(&mut pixmap, &mut resources);
    let coverage = pixmap.data_as_u8_slice();
    let mask: Vec<bool> = (0..w * h)
        .map(|i| coverage[4 * i + 3] >= threshold)
        .collect();
    if !mask.iter().any(|drawn| *drawn) {
        return None;
    }
    Some((mask, [w, h], [bounds.x0 as i32, bounds.y0 as i32]))
}

/// Turns a glyph's outline into a path, y down as a screen is.
struct Pen<'a>(&'a mut kurbo::BezPath);

impl skrifa::outline::OutlinePen for Pen<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.move_to((f64::from(x), -f64::from(y)));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.0.line_to((f64::from(x), -f64::from(y)));
    }

    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.0.quad_to(
            (f64::from(cx0), -f64::from(cy0)),
            (f64::from(x), -f64::from(y)),
        );
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.0.curve_to(
            (f64::from(cx0), -f64::from(cy0)),
            (f64::from(cx1), -f64::from(cy1)),
            (f64::from(x), -f64::from(y)),
        );
    }

    fn close(&mut self) {
        self.0.close_path();
    }
}
