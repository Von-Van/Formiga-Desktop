#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const TRANSPARENT: Self = Self {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };

    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Canvas {
    width: u32,
    height: u32,
    pixels: Vec<Rgba>,
}

impl Canvas {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![Rgba::TRANSPARENT; (width * height) as usize],
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn pixels(&self) -> &[Rgba] {
        &self.pixels
    }

    pub fn rgba_bytes(&self) -> Vec<u8> {
        self.pixels
            .iter()
            .flat_map(|pixel| [pixel.r, pixel.g, pixel.b, pixel.a])
            .collect()
    }

    pub fn set(&mut self, x: i32, y: i32, color: Rgba) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return;
        }
        self.pixels[(y as u32 * self.width + x as u32) as usize] = color;
    }

    pub fn get(&self, x: i32, y: i32) -> Rgba {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return Rgba::TRANSPARENT;
        }
        self.pixels[(y as u32 * self.width + x as u32) as usize]
    }

    pub fn fill_rect(&mut self, x: i32, y: i32, width: i32, height: i32, color: Rgba) {
        for py in y..y + height {
            for px in x..x + width {
                self.set(px, py, color);
            }
        }
    }

    pub fn fill_ellipse(
        &mut self,
        center_x: i32,
        center_y: i32,
        radius_x: i32,
        radius_y: i32,
        color: Rgba,
    ) {
        if radius_x <= 0 || radius_y <= 0 {
            return;
        }
        let rx2 = (radius_x * radius_x) as i64;
        let ry2 = (radius_y * radius_y) as i64;
        let rhs = rx2 * ry2;
        for y in -radius_y..=radius_y {
            for x in -radius_x..=radius_x {
                if x as i64 * x as i64 * ry2 + y as i64 * y as i64 * rx2 <= rhs {
                    self.set(center_x + x, center_y + y, color);
                }
            }
        }
    }

    pub fn fill_circle(&mut self, center_x: i32, center_y: i32, radius: i32, color: Rgba) {
        if radius == 0 {
            self.set(center_x, center_y, color);
            return;
        }
        self.fill_ellipse(center_x, center_y, radius, radius, color);
    }

    pub fn line(
        &mut self,
        mut x0: i32,
        mut y0: i32,
        x1: i32,
        y1: i32,
        thickness: i32,
        color: Rgba,
    ) {
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut error = dx + dy;
        loop {
            self.fill_circle(x0, y0, thickness.saturating_sub(1), color);
            if x0 == x1 && y0 == y1 {
                break;
            }
            let twice = 2 * error;
            if twice >= dy {
                error += dy;
                x0 += sx;
            }
            if twice <= dx {
                error += dx;
                y0 += sy;
            }
        }
    }

    pub fn mirror_horizontal(&mut self) {
        for y in 0..self.height as i32 {
            for x in 0..self.width as i32 / 2 {
                let opposite = self.width as i32 - 1 - x;
                let left = self.get(x, y);
                let right = self.get(opposite, y);
                self.set(x, y, right);
                self.set(opposite, y, left);
            }
        }
    }

    pub fn translate(&mut self, dx: i32, dy: i32) {
        if dx == 0 && dy == 0 {
            return;
        }
        let mut translated = vec![Rgba::TRANSPARENT; self.pixels.len()];
        for y in 0..self.height as i32 {
            for x in 0..self.width as i32 {
                let target_x = x + dx;
                let target_y = y + dy;
                if target_x >= 0
                    && target_y >= 0
                    && target_x < self.width as i32
                    && target_y < self.height as i32
                {
                    let source = (y as u32 * self.width + x as u32) as usize;
                    let target = (target_y as u32 * self.width + target_x as u32) as usize;
                    translated[target] = self.pixels[source];
                }
            }
        }
        self.pixels = translated;
    }

    pub fn alpha_bounds(&self) -> Option<(u32, u32, u32, u32)> {
        let mut min_x = self.width;
        let mut min_y = self.height;
        let mut max_x = 0;
        let mut max_y = 0;
        let mut found = false;
        for y in 0..self.height {
            for x in 0..self.width {
                if self.pixels[(y * self.width + x) as usize].a > 0 {
                    found = true;
                    min_x = min_x.min(x);
                    min_y = min_y.min(y);
                    max_x = max_x.max(x);
                    max_y = max_y.max(y);
                }
            }
        }
        found.then_some((min_x, min_y, max_x, max_y))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_pixel_lines_are_visible_and_connected() {
        let mut canvas = Canvas::new(8, 8);
        let color = Rgba::new(1, 2, 3, 255);
        canvas.line(1, 1, 6, 4, 1, color);
        assert_eq!(canvas.get(1, 1), color);
        assert_eq!(canvas.get(6, 4), color);
        assert!(canvas.pixels().iter().filter(|pixel| pixel.a > 0).count() >= 6);
    }
}
