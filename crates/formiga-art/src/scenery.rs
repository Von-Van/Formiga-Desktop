//! The pictures a village can be laid out on. Each is a painted picture rather than something
//! generated, kept as a PNG at exactly shelter-pixel scale beside the walk map `formiga-core`
//! holds for it, and decoded once when it is first drawn.

use crate::{Canvas, Rgba};
use formiga_core::VillageScenery;

/// The picture itself, as it is kept.
pub const fn scenery_png(scenery: VillageScenery) -> &'static [u8] {
    match scenery {
        VillageScenery::Pond => include_bytes!("../assets/scenery/pond.png"),
    }
}

/// The picture, decoded: the size its walk map is measured in, every pixel fully drawn or clear.
pub fn render_scenery(scenery: VillageScenery) -> Canvas {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(scenery_png(scenery)));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .expect("the scenery pictures are built into the app");
    let mut bytes = vec![
        0;
        reader
            .output_buffer_size()
            .expect("a picture of sensible size")
    ];
    let frame = reader
        .next_frame(&mut bytes)
        .expect("the scenery pictures are built into the app");
    let channels = frame.color_type.samples();
    let mut canvas = Canvas::new(frame.width, frame.height);
    for y in 0..frame.height {
        for x in 0..frame.width {
            let at = (y as usize * frame.line_size) + x as usize * channels;
            let pixel = &bytes[at..at + channels];
            let colour = match channels {
                4 => Rgba::new(pixel[0], pixel[1], pixel[2], pixel[3]),
                3 => Rgba::new(pixel[0], pixel[1], pixel[2], 255),
                2 => Rgba::new(pixel[0], pixel[0], pixel[0], pixel[1]),
                _ => Rgba::new(pixel[0], pixel[0], pixel[0], 255),
            };
            canvas.set(x as i32, y as i32, colour);
        }
    }
    canvas
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_picture_is_the_size_of_its_walk_map_and_drawn_or_clear_pixel_for_pixel() {
        for scenery in VillageScenery::ALL {
            let canvas = render_scenery(scenery);
            let map = scenery.map();
            assert_eq!((canvas.width(), canvas.height()), (map.width, map.height));
            assert!(
                canvas
                    .pixels()
                    .iter()
                    .all(|pixel| pixel.a == 0 || pixel.a == 255)
            );
            // Everywhere anybody can stand is on the picture, not out in the clear around it.
            for &(x, y) in map.nodes() {
                let drawn = (-2..=2)
                    .flat_map(|dy| (-2..=2).map(move |dx| (dx, dy)))
                    .any(|(dx, dy)| canvas.get(x as i32 + dx, y as i32 + dy).a == 255);
                assert!(drawn, "({x}, {y}) is out in the clear");
            }
        }
    }
}
