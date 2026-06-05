use alloc::{vec, vec::Vec, boxed::Box};

use rgb::ComponentMap;
use super::*;

mod mask;
mod tile;
mod bitmap;
mod sampler;
pub(crate) mod inside;

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

    pub fn resize(&mut self, width: usize, height: usize) {
        let sz = width * height;
        self.size = Vec2::new(width, height);
        self.pixels = vec![Default::default(); sz].into();
    }

    pub fn pixels(&self) -> &[Color] {
        &self.pixels
    }

    fn tiles(&self, ssaa: SsaaConfig, bb: BoundingBox) -> TileIterator {
        TileIterator::new(self.size, ssaa, bb)
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

    fn fill(&mut self, shape: &dyn Shape, bb: Option<BoundingBox>, texture: &Texture, ssaa: SsaaConfig) {
        let bb = match bb {
            Some(bb) => bb,
            None => shape.bounding_box(),
        };

        let tile_iter = self.tiles(ssaa, bb);
        let stop = tile_iter.stop();

        for mut tile in tile_iter {
            let mut path_impl = PathImpl {
                mask: &mut self.mask,
                mask_dirty: false,
                ssaa,
                tile_origin: tile.origin_f,
            };

            shape.draw(&mut path_impl);

            if path_impl.mask_dirty {
                self.mask.unpack();
                tile.render(&mut self.pixels, self.size.x, stop, &self.mask, texture, &self.bitmaps);
                self.mask.clear();
            }
        }
    }
}

struct PathImpl<'a> {
    mask: &'a mut Mask,
    mask_dirty: bool,
    ssaa: SsaaConfig,
    tile_origin: Point,
}

trait Collisions {
    fn _rows(&self) -> RangeInclusive<f32>;
    fn collision<G: FnMut(f32)>(&self, row: f32, col_check: G);
}

impl PathApi for PathImpl<'_> {
    fn segment(&mut self, segment: Segment) {
        self.apply_collision(segment);
    }

    fn arc(&mut self, arc: Arc) {
        self.apply_collision(arc);
    }
}

impl PathImpl<'_> {
    fn apply_collision<P: Collisions>(&mut self, primitive: P) {
        // makes even edges more consistent with expectations
        const X_BIAS: f32 = 0.1;
        // this bias avoids double-cross glitches
        const Y_BIAS: f32 = 0.1;

        let log2_cols = self.ssaa.log2_cols::<u16>();
        let tile_width = (256u16 >> log2_cols) as f32;
        let x_mul = (1 << log2_cols) as f32;
        let vo_len: usize = self.ssaa.rows();
        let offsets = self.ssaa.offsets();

        let mut dirty = false;
        let mut vo_index = 0;
        let mut row_v_offset = self.tile_origin.y;

        let rows = primitive._rows();

        for mask_y in 0..128 {
            let (v_offset, _h_offsets) = offsets[vo_index];
            let y = row_v_offset + v_offset + Y_BIAS;

            if y >= *rows.start() {
                if y > *rows.end() {
                    break;
                }

                let col_check = |collision_x: f32| {
                    let x = collision_x - self.tile_origin.x - X_BIAS;

                    if x >= 0.0 {
                        dirty = true;

                        if x >= tile_width {
                            self.mask.toggle(255, mask_y);
                        } else {
                            let mask_x = (x * x_mul).trunc();
                            self.mask.toggle(mask_x as u8, mask_y);
                        }
                    }
                };

                primitive.collision(y, col_check);
            }

            vo_index += 1;
            if vo_index >= vo_len {
                row_v_offset += 1.0;
                vo_index = 0;
            }
        }

        self.mask_dirty |= dirty;
    }
}

impl Collisions for Segment {
    fn _rows(&self) -> RangeInclusive<f32> {
        self.rows()
    }

    fn collision<G: FnMut(f32)>(&self, row: f32, mut col_check: G) {
        let Segment { a, b } = *self;

        let y_range = match a.y < b.y {
            true => a.y..=b.y,
            false => b.y..=a.y,
        };

        if (!y_range.contains(&row)) || a.y == b.y {
            return;
        }

        let t = (a.y - row) / (a.y - b.y);

        if !(0.0..=1.0).contains(&t) {
            return;
        }

        let offset = a.x + (b.x - a.x) * t;
        col_check(offset);
    }
}

impl Collisions for Arc {
    fn _rows(&self) -> RangeInclusive<f32> {
        self.rows()
    }

    fn collision<G: FnMut(f32)>(&self, row: f32, mut col_check: G) {
        let sin_radius = row - self.center.y;

        if !(-self.radius..=self.radius).contains(&sin_radius) {
            return;
        }

        let range = cos_range(self.min_angle_magic, self.max_angle_magic, sin_radius < 0.0);

        if range == NO_COLLISION {
            return;
        }

        let radius = self.radius as f64;
        let sin = (sin_radius as f64) / radius;

        let r_cos = f64::sqrt(1.0 - sin * sin);

        if range.contains(&r_cos) ^ self.wraps {
            col_check(self.center.x + (r_cos * radius) as f32);
        }

        let l_cos = -r_cos;

        if range.contains(&l_cos) ^ self.wraps {
            col_check(self.center.x + (l_cos * radius) as f32);
        }
    }
}

const FAKE_COS: f64 = 2.0;
pub(crate) const NO_COLLISION: RangeInclusive<f64> = FAKE_COS..=FAKE_COS;

pub(crate) fn cos_range(min_angle_magic: u32, max_angle_magic: u32, row_pos: bool) -> RangeInclusive<f64> {
    let (min_cos, min_neg) = decode_magic(min_angle_magic);
    let (max_cos, max_neg) = decode_magic(max_angle_magic);

    // row_pos = row < arc.center.y (its circle position is a positive angle)

    let (cos_a, cos_b) = match (min_neg, max_neg, row_pos) {
        (true, true, true) => (FAKE_COS, FAKE_COS), // no intersection
        (true, true, false) => (min_cos, max_cos),
        (true, false, true) => (1.0, max_cos),
        (true, false, false) => (min_cos, 1.0),
        (false, false, true) => (min_cos, max_cos),
        (false, false, false) => (FAKE_COS, FAKE_COS), // no intersection
        (false, true, _) => unreachable!(),
    };

    let (min_cos, max_cos) = match cos_a <= cos_b {
        true => (cos_a, cos_b),
        false => (cos_b, cos_a),
    };

    min_cos..=max_cos
}
