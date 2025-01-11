use rgb::{RGBA, ComponentMap};
use super::*;

#[cfg(feature = "simd")]
use simd::process_row;

pub struct TileIterator {
    fb_size: Vec2<usize>,
    next: Vec2<usize>,
    tile_width: usize,
    row_coords: [IntPoint; TILE_SIDE],
    ssaa: SsaaConfig,
}

impl TileIterator {
    pub fn new(fb_size: Vec2<usize>, ssaa: SsaaConfig) -> Self {
        let tile_width = TILE_SIDE / ssaa.as_mul::<usize>();
        let mut row_coords = Vec::new();

        for x in 0..tile_width {
            for offset in ssaa.offsets() {
                let mut offset = convert(Vec2::from(*offset));
                offset.x += (x as i32) * PX_WIDTH + (PX_WIDTH / 2);
                offset.y += PX_WIDTH / 2;
                row_coords.push(offset);
            }
        }

        let row_coords = row_coords.try_into().unwrap();

        Self {
            fb_size,
            next: Vec2::new(0, 0),
            tile_width,
            row_coords,
            ssaa,
        }
    }

    fn into_tile(&self) -> Tile {
        const Z: IntPoint = IntPoint::new(0, 0);

        Tile {
            origin_u: self.next,
            origin_f: self.next.map(|u| u as f32),
            tile_width: self.tile_width,
            ssaa: self.ssaa,
            workspace: [Z; POINTS],
            row_coords: self.row_coords,
            i: 0,
        }
    }
}

pub struct Tile {
    /// top left corner of the tile
    origin_u: Vec2<usize>,
    origin_f: Vec2<f32>,
    tile_width: usize,
    ssaa: SsaaConfig,
    workspace: [IntPoint; POINTS],
    row_coords: [IntPoint; TILE_SIDE],
    i: usize,
}

impl Iterator for TileIterator {
    type Item = Tile;

    fn next(&mut self) -> Option<Self::Item> {
        let mut tile = None;

        if self.next.x < self.fb_size.x {
            tile = Some(self.into_tile());
            self.next.x += self.tile_width;
        } else {
            if self.next.y < self.fb_size.y {
                self.next.y += TILE_SIDE;
                self.next.x = 0;
                tile = Some(self.into_tile());
                self.next.x += self.tile_width;
            }
        }

        tile
    }
}

#[inline(always)]
fn convert(p: Point) -> IntPoint {
    (p * (PX_WIDTH as f32)).map(|f| f as i32)
}

impl Tile {
    #[inline(always)]
    pub fn overlaps(&self, aabb: BoundingBox) -> bool {
        let size_f = Vec2::new(self.tile_width as f32, TILE_SIDE as f32);
        let btm_right = self.origin_f + size_f;
        let x_overlap = self.origin_f.x < aabb.x.1 && btm_right.x > aabb.x.0;
        let y_overlap = self.origin_f.y < aabb.y.1 && btm_right.y > aabb.y.0;
        x_overlap && y_overlap
    }

    #[inline(always)]
    fn point(&mut self, p: IntPoint) {
        self.workspace[self.i] = p;
        self.i += 1;
    }

    #[inline(always)]
    fn line(&mut self, a: Point, b: Point, mask: &mut Mask) {
        let a = convert(a - self.origin_f);
        let b = convert(b - self.origin_f);

        let mut prev_is_a = match self.i.checked_sub(1) {
            Some(prev) => self.workspace[prev] == a,
            None => false,
        };

        let needed = 2 - (prev_is_a as usize);
        if self.i + needed > POINTS {
            self.mask_pass(mask);
            prev_is_a = false;
        }

        if !prev_is_a {
            self.point(a);
        }

        self.point(b);
    }

    #[inline(always)]
    pub fn sample_oob(&self, path: &[CubicBezier]) -> bool {
        // sample at (1, 1)
        let point = IntPoint::new(PX_WIDTH, PX_WIDTH);

        let mut inside = false;
        for curve in path {
            let start = convert(curve.c1 - self.origin_f);
            let end = convert(curve.c4 - self.origin_f);
            inside ^= toggle_in_shape(point, start, end);
        }

        inside
    }

    pub fn advance(&mut self, mut curve: CubicBezier, mask: &mut Mask) {
        let mut trial: f32 = 1.0;

        loop {
            let (trial_sc, future_sc) = curve.split(trial);

            let no_overlap = !self.overlaps(trial_sc.aabb());
            let use_as_is = no_overlap || is_curve_straight(trial_sc);

            if use_as_is {

                self.line(trial_sc.c1, trial_sc.c4, mask);

                // did we complete this curve?
                if trial == 1.0 {
                    break;
                }

                curve = future_sc;
                trial = 1.0;

            } else {
                trial *= 0.5;
            }
        }
    }

    #[inline(never)] /* dbg */
    pub fn mask_pass(&mut self, mask: &mut Mask) {
        let mut start = self.workspace[0];
        for p_i in 1..self.i {
            let end = self.workspace[p_i];

            for (y, row) in mask.iter_mut().enumerate() {
                process_row(y, &self.row_coords, start, end, row);
            }

            start = end;
        }

        self.i = 0;
    }

    pub fn render(
        &mut self,
        pixels: &mut [Color],
        fb_size: Vec2<usize>,
        mask: &Mask,
        texture: &Texture,
        bitmaps: &Bitmaps,
    ) {
        let offsets = self.ssaa.offsets();

        let x_min = self.origin_u.x;
        let x_max = x_min + self.tile_width;
        let y_min = self.origin_u.y;
        let y_max = y_min + TILE_SIDE;
        let mut subp_base = Vec2::default();

        let mut rows = mask.iter();
        for y in y_min..y_max {
            let fb_line_offset = y * fb_size.x;
            subp_base.y = y as f32;

            if y >= fb_size.y {
                break;
            }

            let row = rows.next().unwrap();
            let mut row = row.iter();
            for x in x_min..x_max {
                subp_base.x = x as f32;

                if x >= fb_size.x {
                    break;
                }

                let mut color = RGBA::<u16>::new(0, 0, 0, 0);
                let mut hits = false;

                for offset in offsets {
                    if *row.next().unwrap() {
                        let point = subp_base + Vec2::from(*offset);
                        let sample = texture.sample(point, bitmaps);
                        color += sample.map(|c| c as u16);
                        hits = true;
                    }
                }

                if hits {
                    // divide by num_subpx
                    let color = const_ssaa!(self.ssaa, color, /);
                    let src = color.map(|c| c as u8).into();

                    let px_i = fb_line_offset + x;
                    let dst = &mut pixels[px_i];
                    *dst = blend(src, *dst);
                }
            }
        }
    }

    pub fn render_all(
        &mut self,
        pixels: &mut [Color],
        fb_size: Vec2<usize>,
        texture: &Texture,
        bitmaps: &Bitmaps,
    ) {
        let offsets = self.ssaa.offsets();

        let x_min = self.origin_u.x;
        let x_max = x_min + self.tile_width;
        let y_min = self.origin_u.y;
        let y_max = y_min + TILE_SIDE;
        let mut subp_base = Vec2::default();

        for y in y_min..y_max {
            let fb_line_offset = y * fb_size.x;
            subp_base.y = y as f32;

            if y >= fb_size.y {
                break;
            }

            for x in x_min..x_max {
                subp_base.x = x as f32;

                if x >= fb_size.x {
                    break;
                }

                let mut color = RGBA::<u16>::new(0, 0, 0, 0);

                for offset in offsets {
                    let point = subp_base + Vec2::from(*offset);
                    let sample = texture.sample(point, bitmaps);
                    color += sample.map(|c| c as u16);
                }

                // divide by num_subpx
                let color = const_ssaa!(self.ssaa, color, /);
                let src = color.map(|c| c as u8).into();

                let px_i = fb_line_offset + x;
                let dst = &mut pixels[px_i];
                *dst = blend(src, *dst);
            }
        }
    }
}

#[cfg(not(feature = "simd"))]
#[inline(always)]
fn process_row(
    y: usize,
    row_coords: &[IntPoint; TILE_SIDE],
    start: IntPoint,
    end: IntPoint,
    row: &mut [bool; TILE_SIDE],
) {
    let row_offset = IntPoint::new(0, y as i32 * PX_WIDTH);
    for i in 0..TILE_SIDE {
        let shifted = row_offset + row_coords[i];
        row[i] ^= toggle_in_shape(shifted, start, end);
    }
}

#[cfg(not(feature = "simd"))]
// Computes a one bit winding number increment/decrement
#[inline(always)]
fn toggle_in_shape(point: IntPoint, a: IntPoint, b: IntPoint) -> bool {
    let v1 = point - a;
    let v2 = b - a;

    let crit_1 = a.y <= point.y;
    let crit_2 = b.y > point.y;
    let crit_3 = (v1.x * v2.y) > (v1.y * v2.x);

    let dec = ( crit_1) & ( crit_2) & ( crit_3);
    let inc = (!crit_1) & (!crit_2) & (!crit_3);

    inc != dec
}
