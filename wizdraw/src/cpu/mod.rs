use alloc::{vec, vec::Vec, boxed::Box};

use rgb::ComponentMap;
use super::*;

mod bitmap;
mod texture;
mod mask;
mod tile;

use tile::TileIterator;
use bitmap::Bitmaps;
use mask::Mask;

type Color16 = rgb::RGBA::<u16>;

#[derive(Debug, Clone)]
struct Bitmap {
    size: Vec2<usize>,
    pixels: Box<[Color]>,
}

/// Drawing Surface
#[derive(Debug, Clone)]
pub struct Canvas {
    bitmaps: Bitmaps,
    pixels: Box<[Color]>,
    mask: Box<Mask>,
    size: Vec2<usize>,
}

impl Canvas {
    /// Create a basic in-memory canvas
    pub fn new(width: usize, height: usize) -> Canvas {
        let sz = width * height;
        Canvas {
            bitmaps: Bitmaps::new(),
            pixels: vec![Default::default(); sz].into(),
            mask: Box::new(Mask::new()),
            size: Vec2::new(width, height),
        }
    }

    pub fn pixels(&self) -> &[Color] {
        &self.pixels
    }

    fn tiles(&self, ssaa: SsaaConfig) -> TileIterator {
        TileIterator::new(self.size, ssaa)
    }
}

impl super::Canvas for Canvas {
    /// Sets all pixels to fully transparent
    fn clear(&mut self) {
        self.pixels.fill(Default::default());
    }

    fn framebuffer_size(&self) -> Vec2<usize> {
        self.size
    }

    fn alloc_bitmap(&mut self, width: usize, height: usize) -> BitmapHandle {
        let pixels = vec![TRANSPARENT; width * height].into();
        self.bitmaps.push(Bitmap {
            pixels,
            size: Vec2::new(width, height),
        })
    }

    fn fill_bitmap(&mut self, bitmap: BitmapHandle, x: usize, y: usize, w: usize, h: usize, buf: &[Color]) {
        if let Some(bitmap) = self.bitmaps.get_mut(bitmap) {
            let max_x = x + w;
            let max_y = y + h;

            if max_x > bitmap.size.x || max_y > bitmap.size.y {
                return;
            }

            for y_offset in 0..h {
                for x_offset in 0..w {
                    let x = x + x_offset;
                    let y = y + y_offset;
                    let src_i = y_offset * w + x_offset;
                    let dst_i = y * bitmap.size.x + x;
                    bitmap.pixels[dst_i] = buf[src_i];
                }
            }
        }
    }

    fn free_bitmap(&mut self, bitmap: BitmapHandle) {
        if let Some(bitmap) = self.bitmaps.get_mut(bitmap) {
            bitmap.size = Vec2::new(0, 0);
            bitmap.pixels = Box::new([]);
        }
    }

    fn draw(&mut self, shape: &dyn Shape, texture: &Texture, ssaa: SsaaConfig) {
        let mut bb = BoundingBox::default();
        shape.draw(&mut bb);

        for mut tile in self.tiles(ssaa) {
            let sz_f32 = tile.tile_size.map(|c| c as f32);
            let tile_max = tile.origin_f + sz_f32;

            let x_overlap = (bb.max.x >= tile.origin_f.x) & (bb.min.x < tile_max.x);
            let y_overlap = (bb.max.y >= tile.origin_f.y) & (bb.min.y < tile_max.y);

            if !(x_overlap & y_overlap) {
                continue;
            }

            let mut path_impl = PathImpl {
                mask: &mut self.mask,
                ssaa,
                tile_origin: tile.origin_f,
            };

            shape.draw(&mut path_impl);

            if self.mask.unpack() {
                tile.render(&mut self.pixels, self.size, &self.mask, texture, &self.bitmaps);
            }

            self.mask.clear();
        }
    }
}

struct PathImpl<'a> {
    mask: &'a mut Mask,
    ssaa: SsaaConfig,
    tile_origin: Point,
}

impl PathApi for PathImpl<'_> {
    fn segment(&mut self, segment: Segment) {
        let rows = segment.rows();
        let col_fn = |row| segment_collision(row, &segment);
        self.apply_collision(rows, col_fn);
    }

    fn arc(&mut self, arc: Arc) {
        let rows = arc.rows();
        let col_fn = |row| arc_collision(row, &arc);
        self.apply_collision(rows, col_fn);
    }
}

impl PathImpl<'_> {
    fn apply_collision<F: Fn(f32) -> Collisions>(&mut self, rows: RangeInclusive<f32>, collisions: F) {
        // this bias avoids double-cross glitches
        const Y_BIAS: f32 = 0.1;

        let log2_cols = self.ssaa.log2_cols::<u16>();
        let tile_width = (256u16 >> log2_cols) as f32;
        let x_mul = (1 << log2_cols) as f32;
        let vo_len: usize = self.ssaa.rows();
        let offsets = self.ssaa.offsets();

        let mut vo_index = 0;
        let mut row_v_offset = self.tile_origin.y;

        for mask_y in 0..128 {
            let (v_offset, _h_offsets) = offsets[vo_index];
            let y = row_v_offset + v_offset + Y_BIAS;

            if y >= *rows.start() {
                if y >= *rows.end() {
                    break;
                }

                let collisions = collisions(y);
                for collision_x in collisions.iter().flatten() {
                    let x = collision_x - self.tile_origin.x;

                    if x >= 0.0 {
                        if x >= tile_width {
                            self.mask.toggle(255, mask_y);
                        } else {
                            let mask_x = (x * x_mul).trunc();
                            self.mask.toggle(mask_x as u8, mask_y);
                        }
                    }
                }
            }

            vo_index += 1;
            if vo_index >= vo_len {
                row_v_offset += 1.0;
                vo_index = 0;
            }
        }
    }
}

type Collisions = [Option<f32>; 2];

fn segment_collision(row: f32, segment: &Segment) -> Collisions {
    let Segment { a, b } = *segment;

    let y_range = match a.y < b.y {
        true => a.y..=b.y,
        false => b.y..=a.y,
    };

    if (!y_range.contains(&row)) || a.y == b.y {
        return [None, None];
    }

    let t = (a.y - row) / (a.y - b.y);

    if !(0.0..=1.0).contains(&t) {
        return [None, None];
    }

    let offset = a.x + (b.x - a.x) * t;
    // let downward = b.y > a.y;

    [Some(offset), None]
}

pub(crate) fn cos_range(min_angle_magic: u32, max_angle_magic: u32, row_pos: bool) -> RangeInclusive<f64> {
    let (min_cos, min_neg) = decode_magic(min_angle_magic);
    let (max_cos, max_neg) = decode_magic(max_angle_magic);
    let fake = 2.0;

    let (cos_a, cos_b) = match (min_neg, max_neg, row_pos) {
        (true, true, true) => (fake, fake), // no intersection
        (true, true, false) => (min_cos, max_cos),
        (true, false, true) => (1.0, max_cos),
        (true, false, false) => (min_cos, 1.0),
        (false, false, true) => (min_cos, max_cos),
        (false, false, false) => (fake, fake), // no intersection
        (false, true, _) => unreachable!(),
    };

    let (min_cos, max_cos) = match cos_a <= cos_b {
        true => (cos_a, cos_b),
        false => (cos_b, cos_a),
    };

    min_cos..=max_cos
}

fn arc_collision(row: f32, arc: &Arc) -> Collisions {
    type Precise = f64;
    let sin_radius = row - arc.center.y;

    if arc.radius <= 0.0 || !(-arc.radius..=arc.radius).contains(&sin_radius) {
        return [None, None];
    }

    let precise_radius = arc.radius as Precise;
    let sin = (sin_radius as Precise) / precise_radius;
    let r_cos = Precise::sqrt(1.0 - sin * sin);
    let l_cos = -r_cos;

    let range = cos_range(arc.min_angle_magic, arc.max_angle_magic, sin < 0.0);

    let r_col = range.contains(&r_cos) ^ arc.wraps;
    let l_col = range.contains(&l_cos) ^ arc.wraps;

    let mut l_collision = None;
    let mut r_collision = None;

    if l_col {
        let offset = arc.center.x + (l_cos * precise_radius) as f32;
        l_collision = Some(offset);
    }

    if r_col {
        let offset = arc.center.x + (r_cos * precise_radius) as f32;
        r_collision = Some(offset);
    }

    [l_collision, r_collision]
}
