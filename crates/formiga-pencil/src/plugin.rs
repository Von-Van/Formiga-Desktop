//! The plugin that draws text set in the hand from the pencil atlas instead of egui's.

use crate::atlas::{Atlas, Face};
use crate::{Everywhere, HAND, HAND_BOLD};
use egui::emath::Rot2;
use egui::epaint::{ClippedShape, ImageDelta, Mesh, Shape, TextShape, Vertex};
use egui::{Color32, FontFamily, Pos2, TextureId, TextureOptions, Vec2, pos2, vec2};
use std::sync::Arc;

/// Draws the hand in pencil. Added to a context by [`crate::install`].
///
/// Each frame, every piece of text with a glyph in the hand is left in the output where egui put
/// it, made invisible, so whatever reads the frame's text still finds it; beside it go the
/// pencilled glyphs, and anything in the same text that is not in the hand (another font, a
/// glyph Kalam lacks, a highlight behind it) drawn exactly as egui would have.
pub struct Pencil {
    everywhere: Everywhere,
    faces: [Face; 2],
    atlas: Atlas,
    texture: Option<TextureId>,
}

impl Pencil {
    pub fn new(everywhere: Everywhere) -> Self {
        Self {
            everywhere,
            faces: [Face::new(crate::REGULAR), Face::new(crate::BOLD)],
            atlas: Atlas::new(),
            texture: None,
        }
    }

    /// Which of Kalam's faces a family is written in, if it is written in the hand.
    fn face_of(&self, family: &FontFamily) -> Option<usize> {
        match family {
            FontFamily::Name(name) if &**name == HAND => Some(0),
            FontFamily::Name(name) if &**name == HAND_BOLD => Some(1),
            FontFamily::Proportional if self.everywhere == Everywhere::Yes => Some(0),
            _ => None,
        }
    }

    fn redraw(&mut self, shape: &mut Shape, frame: &Frame) {
        match shape {
            Shape::Vec(shapes) => {
                for shape in shapes {
                    self.redraw(shape, frame);
                }
            }
            Shape::Text(text) => {
                if let Some(drawn) = self.pencil(text, frame) {
                    let mut hidden = text.clone();
                    hidden.opacity_factor = 0.0;
                    let mut shapes = vec![Shape::Text(hidden)];
                    shapes.extend(drawn);
                    *shape = Shape::Vec(shapes);
                }
            }
            _ => {}
        }
    }

    /// `text` drawn again: what egui drew of it that is not in the hand, then the hand in pencil.
    /// `None` when none of it is in the hand, or the galley's glyphs cannot be matched to its
    /// characters (shaped into ligatures, or cut short), in which case egui draws it as it is.
    fn pencil(&mut self, text: &TextShape, frame: &Frame) -> Option<Vec<Shape>> {
        let galley = &text.galley;
        let job = &galley.job;
        if galley.elided
            || !job
                .sections
                .iter()
                .any(|section| self.face_of(&section.format.font_id.family).is_some())
        {
            return None;
        }
        let ppp = frame.pixels_per_point;
        let origin = (text.pos.to_vec2() * ppp).round() / ppp;
        let turn = Rot2::from_angle(text.angle);
        let place = |local: Vec2| -> Pos2 { (origin + turn * local).to_pos2() };
        let colour = |vertex: &Vertex, glyph: bool| {
            let mut colour = vertex.color;
            if let Some(over) = text.override_text_color {
                if glyph {
                    colour = over;
                }
            } else if colour == Color32::PLACEHOLDER {
                colour = text.fallback_color;
            }
            if text.opacity_factor < 1.0 {
                colour = colour.gamma_multiply(text.opacity_factor);
            }
            colour
        };

        let mut chars = job.text.char_indices();
        let mut section = 0;
        let mut pencilled = Mesh::with_texture(self.texture?);
        let mut rest = Mesh::default();
        let mut any = false;
        for row in &galley.rows {
            let visuals = &row.visuals;
            let mut skip = vec![false; visuals.mesh.vertices.len()];
            // Each glyph with its character and the section it was set in.
            let mut glyphs = Vec::with_capacity(row.glyphs.len());
            for glyph in &row.glyphs {
                let (byte, chr) = chars.next()?;
                if chr != glyph.chr {
                    return None;
                }
                while job.sections.get(section)?.byte_range.end <= egui::text::ByteIndex(byte) {
                    section += 1;
                }
                glyphs.push((glyph, section));
            }
            for (index, (glyph, section)) in glyphs.iter().enumerate() {
                let format = &job.sections[*section].format;
                let Some(face) = self.face_of(&format.font_id.family) else {
                    continue;
                };
                let first = glyph.first_vertex as usize;
                if glyph.uv_rect.is_nothing() || first + 4 > visuals.mesh.vertices.len() {
                    continue;
                }
                // A ligature is one glyph followed by an empty one for each further character it
                // stands for; the pencil writes each character of it in turn instead.
                let cluster: Vec<char> = std::iter::once(glyph.chr)
                    .chain(
                        glyphs[index + 1..]
                            .iter()
                            .take_while(|(next, _)| {
                                next.uv_rect.is_nothing()
                                    && next.advance_width == 0.0
                                    && !next.chr.is_whitespace()
                            })
                            .map(|(next, _)| next.chr),
                    )
                    .collect();
                let Some(ids) = cluster
                    .iter()
                    .map(|chr| self.faces[face].glyph(*chr))
                    .collect::<Option<Vec<_>>>()
                else {
                    continue;
                };
                let heading = format.font_id.size >= frame.heading_size && text.angle == 0.0;
                // Screen pixels to a texel: a point's worth for writing, two for a heading.
                let k = (ppp * if heading { 2.0 } else { 1.0 }).round().max(1.0);
                let threshold = if heading { 128 } else { 84 };
                let em = format.font_id.size * ppp / k;
                let ink = colour(&visuals.mesh.vertices[first], true);
                let mut pen = (row.pos.to_vec2() + glyph.pos.to_vec2()) * ppp;
                for id in ids {
                    let advance = self.faces[face].advance(id) * format.font_id.size * ppp;
                    let Some(placed) = self.atlas.glyph(&self.faces, face, id, em, threshold)
                    else {
                        pen.x += advance;
                        continue;
                    };
                    // The pen, snapped to the texel grid of the screen, so every glyph's texels
                    // fall on whole pixels and line up with the next glyph's.
                    let absolute = origin * ppp + pen;
                    let snapped = vec2((absolute.x / k).round() * k, (absolute.y / k).round() * k)
                        - origin * ppp;
                    let min = snapped + vec2(placed.offset[0] as f32, placed.offset[1] as f32) * k;
                    let size = vec2(placed.size[0] as f32, placed.size[1] as f32) * k;
                    let corners = [
                        min,
                        min + vec2(size.x, 0.0),
                        min + vec2(0.0, size.y),
                        min + size,
                    ]
                    .map(|corner| place(corner / ppp));
                    let (u0, v0) = (f32::from(placed.at[0]), f32::from(placed.at[1]));
                    let (u1, v1) = (
                        u0 + f32::from(placed.size[0]),
                        v0 + f32::from(placed.size[1]),
                    );
                    let uvs = [pos2(u0, v0), pos2(u1, v0), pos2(u0, v1), pos2(u1, v1)];
                    let base = pencilled.vertices.len() as u32;
                    for (pos, uv) in corners.into_iter().zip(uvs) {
                        pencilled.vertices.push(Vertex {
                            pos,
                            uv,
                            color: ink,
                        });
                    }
                    pencilled.add_triangle(base, base + 1, base + 2);
                    pencilled.add_triangle(base + 2, base + 1, base + 3);
                    pen.x += advance;
                }
                skip[first..first + 4].fill(true);
                any = true;
            }
            if row.ends_with_newline {
                chars.next()?;
            }
            // Everything egui drew for the row that the pencil did not take over.
            let base = rest.vertices.len() as u32;
            let row_at = row.pos.to_vec2();
            for (i, vertex) in visuals.mesh.vertices.iter().enumerate() {
                rest.vertices.push(Vertex {
                    pos: place(row_at + vertex.pos.to_vec2()),
                    uv: pos2(
                        vertex.uv.x / frame.font_texture[0],
                        vertex.uv.y / frame.font_texture[1],
                    ),
                    color: colour(vertex, visuals.glyph_vertex_range.contains(&i)),
                });
            }
            for triangle in visuals.mesh.indices.chunks_exact(3) {
                if triangle.iter().all(|i| !skip[*i as usize]) {
                    rest.indices.extend(triangle.iter().map(|i| base + i));
                }
            }
        }
        if !any {
            return None;
        }
        // Texel coordinates become the atlas's own, now that it has grown to hold them all.
        let [w, h] = self.atlas.size();
        for vertex in &mut pencilled.vertices {
            vertex.uv = pos2(vertex.uv.x / w as f32, vertex.uv.y / h as f32);
        }
        // A mesh with nothing to draw is left out: a renderer may refuse an empty buffer.
        Some(
            [rest, pencilled]
                .into_iter()
                .filter(|mesh| !mesh.indices.is_empty())
                .map(Shape::mesh)
                .collect(),
        )
    }
}

/// What every piece of text in a frame is drawn against.
struct Frame {
    pixels_per_point: f32,
    font_texture: [f32; 2],
    heading_size: f32,
}

impl egui::Plugin for Pencil {
    fn debug_name(&self) -> &'static str {
        "formiga-pencil"
    }

    fn setup(&mut self, ctx: &egui::Context) {
        self.texture = Some(ctx.tex_manager().write().alloc(
            "formiga-pencil".into(),
            egui::ImageData::Color(Arc::new(self.atlas.image())),
            TextureOptions::NEAREST,
        ));
        self.atlas.grown = true;
    }

    fn output_hook(&mut self, ctx: &egui::Context, output: &mut egui::FullOutput) {
        let Some(texture) = self.texture else {
            return;
        };
        let [fw, fh] = ctx.fonts(|fonts| fonts.font_image_size());
        let frame = Frame {
            pixels_per_point: output.pixels_per_point,
            font_texture: [fw as f32, fh as f32],
            heading_size: crate::heading_size(ctx),
        };
        for ClippedShape { shape, .. } in &mut output.shapes {
            self.redraw(shape, &frame);
        }
        // The atlas is sent whole when it is new or has grown, which also replaces the blank one
        // queued when it was allocated, and otherwise only the rows that gained glyphs.
        let delta = if self.atlas.grown {
            output.textures_delta.set.remove(&texture);
            Some(ImageDelta::full(
                self.atlas.image(),
                TextureOptions::NEAREST,
            ))
        } else {
            self.atlas.dirty.map(|(from, to)| {
                ImageDelta::partial(
                    [0, from],
                    self.atlas.rows(from, to),
                    TextureOptions::NEAREST,
                )
            })
        };
        if let Some(delta) = delta {
            output
                .textures_delta
                .set
                .entry(texture)
                .or_default()
                .push(delta);
        }
        self.atlas.grown = false;
        self.atlas.dirty = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One frame of `ctx` with a line in the hand and a printed line, and what came out of it.
    fn frame(ctx: &egui::Context) -> egui::FullOutput {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, vec2(400.0, 200.0))),
            ..Default::default()
        };
        ctx.run_ui(input, |ui| {
            ui.label(egui::RichText::new("Two down, one to go!").font(crate::hand(16.0)));
            ui.label(egui::RichText::new("FORMIGA").monospace());
        })
    }

    fn texts(shape: &Shape, out: &mut Vec<(String, f32)>) {
        match shape {
            Shape::Vec(shapes) => shapes.iter().for_each(|shape| texts(shape, out)),
            Shape::Text(text) => out.push((text.galley.text().to_owned(), text.opacity_factor)),
            _ => {}
        }
    }

    fn meshes_on(shape: &Shape, texture: TextureId) -> usize {
        match shape {
            Shape::Vec(shapes) => shapes.iter().map(|shape| meshes_on(shape, texture)).sum(),
            Shape::Mesh(mesh) if mesh.texture_id == texture && !mesh.is_empty() => 1,
            _ => 0,
        }
    }

    #[test]
    fn writing_in_the_hand_is_drawn_in_pencil_and_print_is_left_to_egui() {
        let ctx = egui::Context::default();
        crate::install(&ctx, Everywhere::No);
        let mut output = frame(&ctx);
        let mut found = Vec::new();
        for clipped in &output.shapes {
            texts(&clipped.shape, &mut found);
        }
        // Both lines are still there for whatever reads the frame's words; only the hand is
        // hidden, because the pencil draws it.
        assert!(found.contains(&("Two down, one to go!".to_owned(), 0.0)));
        assert!(found.contains(&("FORMIGA".to_owned(), 1.0)));
        let texture = ctx
            .tex_manager()
            .read()
            .allocated()
            .find(|(_, meta)| meta.name == "formiga-pencil")
            .map(|(id, _)| *id)
            .expect("the pencil has a texture");
        let pencilled: usize = output
            .shapes
            .iter()
            .map(|clipped| meshes_on(&clipped.shape, texture))
            .sum();
        assert_eq!(pencilled, 1, "one line is drawn from the pencil atlas");
        // Nothing empty goes to the renderer: wgpu refuses an empty buffer.
        fn empty(shape: &Shape) -> bool {
            match shape {
                Shape::Vec(shapes) => shapes.iter().any(empty),
                Shape::Mesh(mesh) => mesh.indices.is_empty(),
                _ => false,
            }
        }
        assert!(!output.shapes.iter().any(|clipped| empty(&clipped.shape)));
        // The atlas goes out with the frame that first needs it, and holds only drawn or blank
        // texels: nothing is anti-aliased.
        let deltas = &output.textures_delta.set[&texture];
        let egui::ImageData::Color(image) = &deltas[0].image;
        assert!(image.pixels.iter().any(|p| p.a() == 255));
        assert!(image.pixels.iter().all(|p| p.a() == 0 || p.a() == 255));
        output.textures_delta.clear();
    }

    #[test]
    fn a_heading_is_drawn_on_a_grid_twice_as_coarse_as_writing() {
        let faces = [Face::new(crate::REGULAR), Face::new(crate::BOLD)];
        let mut atlas = Atlas::new();
        let a = faces[0].glyph('a').expect("Kalam has an a");
        let writing = atlas.glyph(&faces, 0, a, 16.0, 84).unwrap();
        let heading = atlas.glyph(&faces, 0, a, 20.0, 128).unwrap();
        // A 40-point heading at two points a texel is drawn from a glyph half as many texels
        // across as a 40-point line of writing would be.
        let full = atlas.glyph(&faces, 0, a, 40.0, 84).unwrap();
        assert!(heading.size[0] * 2 <= full.size[0] + 2);
        assert!(writing.size[0] < full.size[0]);
    }
}
