use rgb::RGBA;
use super::*;

pub struct TileIterator {
    fb_size: Vec2<usize>,
    next: Vec2<usize>,
    tile_width: usize,
    ssaa: SsaaConfig,
    #[cfg(not(feature = "simd"))]
    row_coords: [IntPoint; TILE_W],
    #[cfg(feature = "simd")]
    row_coords: [simd::SimdPoint; simd::S_TILE_W],
}

impl TileIterator {
    pub fn new(fb_size: Vec2<usize>, ssaa: SsaaConfig) -> Self {
        let tile_width = TILE_W / ssaa.as_mul::<usize>();
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
            #[cfg(not(feature = "simd"))]
            row_coords,
            #[cfg(feature = "simd")]
            row_coords: simd::prepare_coords(&row_coords),
            ssaa,
        }
    }

    fn into_tile(&self) -> Tile {
        const Z: IntPoint = IntPoint::new(0, 0);
        let origin_f = self.next.map(|u| u as f32);

        let width = self.tile_width as f32;
        let height = TILE_H as f32;
        let btm_right = origin_f + Vec2::new(width, height);
        let aabb = BoundingBox {
            min: origin_f,
            max: btm_right,
        };

        Tile {
            origin_u: self.next,
            origin_f,
            tile_width: self.tile_width,
            ssaa: self.ssaa,
            aabb,
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
    pub(super) aabb: BoundingBox,
    i: usize,
    workspace: [IntPoint; POINTS],
    #[cfg(not(feature = "simd"))]
    row_coords: [IntPoint; TILE_W],
    #[cfg(feature = "simd")]
    row_coords: [simd::SimdPoint; simd::S_TILE_W],
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
                self.next.y += TILE_H;
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
        const POINT: IntPoint = IntPoint::new(PX_WIDTH, PX_WIDTH);
        let mut inside = false;

        for curve in path {
            if self.aabb.overlaps_with(curve.aabb()) {
                for sub_curve in curve.split_4() {
                    let start = convert(sub_curve.c1 - self.origin_f);
                    let end = convert(sub_curve.c4 - self.origin_f);
                    inside ^= seq_toggle_in_shape(POINT, start, end);
                }
            } else {
                let start = convert(curve.c1 - self.origin_f);
                let end = convert(curve.c4 - self.origin_f);
                inside ^= seq_toggle_in_shape(POINT, start, end);
            }
        }

        inside
    }

    pub fn advance(&mut self, mut curve: CubicBezier, mask: &mut Mask) {
        let mut trial: f32 = 1.0;

        loop {
            let (trial_sc, future_sc) = curve.split(trial);

            let no_overlap = !self.aabb.overlaps_with(trial_sc.aabb());
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
                #[cfg(not(feature = "simd"))] {
                    *row ^= seq_process_row(y, &self.row_coords, start, end)
                }

                #[cfg(feature = "simd")] {
                    *row ^= simd::process_row(y, &self.row_coords, start, end)
                }
            }

            start = end;
        }

        self.i = 0;
    }
}

#[cfg(not(feature = "simd"))]
#[inline(always)]
fn seq_process_row(
    y: usize,
    row_coords: &[IntPoint; TILE_W],
    start: IntPoint,
    end: IntPoint,
) -> MaskRow {
    let row_offset = IntPoint::new(0, y as i32 * PX_WIDTH);
    let mut xor_mask = 0;

    for i in 0..TILE_W {
        let shifted = row_offset + row_coords[i];
        let inside = seq_toggle_in_shape(shifted, start, end);
        xor_mask |= (inside as MaskRow) << i;
    }

    xor_mask
}

// Computes a one bit winding number increment/decrement
#[inline(always)]
fn seq_toggle_in_shape(point: IntPoint, a: IntPoint, b: IntPoint) -> bool {
    let v1 = point - a;
    let v2 = b - a;

    let crit_1 = a.y <= point.y;
    let crit_2 = b.y > point.y;
    let crit_3 = (v1.x * v2.y) > (v1.y * v2.x);

    let dec = ( crit_1) & ( crit_2) & ( crit_3);
    let inc = (!crit_1) & (!crit_2) & (!crit_3);

    inc != dec
}

// rendering
impl Tile {
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
        let y_max = y_min + TILE_H;

        let mut subp_base = Vec2::default();
        let mut rows = mask.iter();

        for y in y_min..y_max {
            let fb_line_offset = y * fb_size.x;
            subp_base.y = y as f32;

            if y >= fb_size.y {
                break;
            }

            let mut row = *rows.next().unwrap();
            for x in x_min..x_max {
                subp_base.x = x as f32;

                if x >= fb_size.x {
                    break;
                }

                let mut color = RGBA::<u16>::new(0, 0, 0, 0);
                let mut hits = false;

                for offset in offsets {
                    if (row & 1) > 0 {
                        let point = subp_base + Vec2::from(*offset);
                        let sample = texture.sample(point, bitmaps);
                        color += sample;
                        hits = true;
                    }

                    row >>= 1;
                }

                if hits {
                    // divide by num_subpx
                    let src = const_ssaa!(self.ssaa, color, /);

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
        mut texture: &Texture,
        bitmaps: &Bitmaps,
    ) {
        let offsets = self.ssaa.offsets();

        if DEBUG_NON_OVERLAPPING {
            texture = &Texture::Debug;
        }

        let x_min = self.origin_u.x;
        let x_max = x_min + self.tile_width;
        let y_min = self.origin_u.y;
        let y_max = y_min + TILE_H;

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
                    color += sample;
                }

                // divide by num_subpx
                let src = const_ssaa!(self.ssaa, color, /);

                let px_i = fb_line_offset + x;
                let dst = &mut pixels[px_i];
                *dst = blend(src, *dst);
            }
        }
    }
}
