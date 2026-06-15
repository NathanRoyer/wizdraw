use alloc::{vec, vec::Vec, boxed::Box};
use core::marker::PhantomData;

use rgb::ComponentMap;
use super::*;

use crate::ssaa::*;

mod mask;
mod tile;
mod sampler;
pub(crate) mod inside;

use tile::TileIterator;
use mask::Mask;

type Color16 = rgb::RGBA::<u16>;

#[derive(Debug, Clone)]
struct Bitmap {
    size: Vec2<usize>,
    pixels: Box<[Color]>,
}

#[derive(Debug, Clone)]
struct MaskImage {
    size: Vec2<usize>,
    pixels: Box<[u8]>,
}

/// Drawing Surface
#[derive(Debug, Clone)]
pub struct Canvas {
    bitmaps: Vec<Bitmap>,
    masks: Vec<MaskImage>,
    pixels: Box<[Color]>,
    mask: Box<Mask>,
    size: Vec2<usize>,
}

impl Canvas {
    /// Create a basic in-memory canvas
    pub fn new(width: usize, height: usize) -> Canvas {
        let sz = width * height;
        Canvas {
            bitmaps: Vec::new(),
            masks: Vec::new(),
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

    fn fill<S: ConstSsaa>(&mut self, shape: &dyn Shape, bb: Option<BoundingBox>, texture: &Texture) {
        let bb = match bb {
            Some(bb) => bb,
            None => shape.bounding_box(),
        };

        if bb.is_empty() {
            return;
        }

        let tile_iter = TileIterator::<S>::new(self.size, bb);
        let stop = tile_iter.stop();

        let sampler = texture.sampler(&self.bitmaps, &self.masks);

        for mut tile in tile_iter {
            let mut path_impl = PathImpl::<S> {
                mask: &mut self.mask,
                mask_dirty: false,
                tile_origin: tile.origin_f,
                _s: PhantomData,
            };

            shape.draw(&mut path_impl);

            if path_impl.mask_dirty {
                use sampler::SamplerEnum::*;
                self.mask.unpack();

                match &sampler {
                    Mask(s) => tile.render(&mut self.pixels, self.size.x, stop, &self.mask, s),
                    Solid(s) => tile.render(&mut self.pixels, self.size.x, stop, &self.mask, s),
                    Rainbow(s) => tile.render(&mut self.pixels, self.size.x, stop, &self.mask, s),
                    ScaledBitmap(s) => tile.render(&mut self.pixels, self.size.x, stop, &self.mask, s),
                    QuadBitmap(s) => tile.render(&mut self.pixels, self.size.x, stop, &self.mask, s),
                }

                self.mask.clear();
            }
        }
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
        let handle = BitmapHandle(self.bitmaps.len());
        self.bitmaps.push(Bitmap {
            pixels,
            size: Vec2::new(width, height),
        });
        handle
    }

    fn fill_bitmap(&mut self, bitmap: BitmapHandle, x: usize, y: usize, w: usize, h: usize, buf: &[Color]) {
        if let Some(bitmap) = self.bitmaps.get_mut(bitmap.0) {
            let (max_x, max_y) = (x + w, y + h);

            if max_x > bitmap.size.x || max_y > bitmap.size.y {
                return;
            }

            for y_offset in 0..h {
                for x_offset in 0..w {
                    let (x, y) = (x + x_offset, y + y_offset);
                    let src_i = y_offset * w + x_offset;
                    let dst_i = y * bitmap.size.x + x;
                    bitmap.pixels[dst_i] = buf[src_i];
                }
            }
        }
    }

    fn free_bitmap(&mut self, bitmap: BitmapHandle) {
        if let Some(bitmap) = self.bitmaps.get_mut(bitmap.0) {
            bitmap.size = Vec2::new(0, 0);
            bitmap.pixels = Box::new([]);
        }
    }

    fn alloc_mask(&mut self, width: usize, height: usize) -> MaskHandle {
        let pixels = vec![0u8; width * height].into();
        let handle = MaskHandle(self.masks.len());
        self.masks.push(MaskImage {
            pixels,
            size: Vec2::new(width, height),
        });
        handle
    }

    fn fill_mask(&mut self, mask: MaskHandle, x: usize, y: usize, w: usize, h: usize, buf: &[u8]) {
        if let Some(mask) = self.masks.get_mut(mask.0) {
            let (max_x, max_y) = (x + w, y + h);

            if max_x > mask.size.x || max_y > mask.size.y {
                return;
            }

            for y_offset in 0..h {
                for x_offset in 0..w {
                    let (x, y) = (x + x_offset, y + y_offset);
                    let src_i = y_offset * w + x_offset;
                    let dst_i = y * mask.size.x + x;
                    mask.pixels[dst_i] = buf[src_i];
                }
            }
        }
    }

    fn free_mask(&mut self, mask: MaskHandle) {
        if let Some(mask) = self.masks.get_mut(mask.0) {
            mask.size = Vec2::new(0, 0);
            mask.pixels = Box::new([]);
        }
    }

    fn fill(&mut self, shape: &dyn Shape, bb: Option<BoundingBox>, texture: &Texture, ssaa: SsaaConfig) {
        match ssaa {
            SsaaConfig::None => self.fill::<NoSsaa>(shape, bb, texture),
            SsaaConfig::X2 => self.fill::<SsaaX2>(shape, bb, texture),
            SsaaConfig::X4 => self.fill::<SsaaX4>(shape, bb, texture),
            SsaaConfig::X8 => self.fill::<SsaaX8>(shape, bb, texture),
            SsaaConfig::X16 => self.fill::<SsaaX16>(shape, bb, texture),
        }
    }
}

struct PathImpl<'a, S> {
    mask: &'a mut Mask,
    mask_dirty: bool,
    tile_origin: Point,
    _s: PhantomData<S>,
}

trait Collisions {
    fn rows(&self) -> RangeInclusive<f32>;
    fn collision<G: FnMut(f32)>(&self, row: f32, col_check: G);
}

impl<S: ConstSsaa> PathApi for PathImpl<'_, S> {
    fn segment(&mut self, segment: Segment) {
        let precomp = SegmentPrecomp::new(segment);
        self.apply_collision(precomp);
    }

    fn arc(&mut self, arc: Arc) {
        let precomp = ArcPrecomp::new(arc);
        self.apply_collision(precomp);
    }
}

impl<S: ConstSsaa> PathImpl<'_, S> {
    fn apply_collision<P: Collisions>(&mut self, primitive: P) {
        // makes even edges more consistent with expectations
        const X_BIAS: f32 = 0.1;
        // this bias avoids double-cross glitches
        const Y_BIAS: f32 = 0.1;

        let tile_width = (256u16 >> S::LOG2_COLS) as f32;
        let tile_height = (128u16 >> S::LOG2_ROWS) as f32 + 1.0;

        let mut row_offset = self.tile_origin.y + Y_BIAS;
        let col_offset = self.tile_origin.x + X_BIAS;
        let mut dirty = false;
        let threshold = S::ROWS - 1;

        let y_to_mask_y = |y: f32| {
            let tile_y = y - (row_offset + S::Y_BIAS);
            let tile_y = tile_y.clamp(0.0, tile_height);
            (tile_y * (S::ROWS as f32)) as u8
        };

        let (min_row, max_row) = primitive.rows().into_inner();
        let mask_start = y_to_mask_y(min_row).saturating_sub(S::ROWS);
        let mask_end = u8::min(y_to_mask_y(max_row) + S::ROWS, 128);

        row_offset += (mask_start >> S::LOG2_ROWS) as f32;
        let mut y = row_offset + S::Y_BIAS;

        for mask_y in mask_start..mask_end {
            let col_check = |collision_x: f32| {
                let x = collision_x - col_offset;

                if x >= 0.0 {
                    dirty = true;

                    if x >= tile_width {
                        self.mask.toggle(255, mask_y);
                    } else {
                        let x_mul = S::COLS as f32;
                        let mask_x = (x * x_mul).trunc();
                        self.mask.toggle(mask_x as u8, mask_y);
                    }
                }
            };

            primitive.collision(y, col_check);

            if (mask_y & threshold) == threshold {
                row_offset += 1.0;
                y = row_offset + S::Y_BIAS;
            } else {
                y += S::Y_STEP;
            }
        }

        self.mask_dirty |= dirty;
    }
}

struct SegmentPrecomp {
    inner: Segment,
    inv_div: Option<f32>,
}

impl SegmentPrecomp {
    fn new(inner: Segment) -> Self {
        let sub = inner.a.y - inner.b.y;

        let inv_div = match sub {
            0.0 => None,
            _ => Some(1.0 / sub),
        };

        Self { inner, inv_div }
    }
}

impl Collisions for SegmentPrecomp {
    fn rows(&self) -> RangeInclusive<f32> {
        self.inner.rows()
    }

    fn collision<G: FnMut(f32)>(&self, row: f32, mut col_check: G) {
        let Some(inv_div) = self.inv_div else {
            return;
        };

        let Segment { a, b } = self.inner;
        let y_range = match a.y < b.y {
            true => a.y..=b.y,
            false => b.y..=a.y,
        };

        if !y_range.contains(&row) {
            return;
        }

        let t = (a.y - row) * inv_div;

        if !(0.0..=1.0).contains(&t) {
            return;
        }

        col_check(a.x + (b.x - a.x) * t);
    }
}

struct ArcPrecomp {
    inner: Arc,
    range_pos: Option<RangeInclusive<f64>>,
    range_neg: Option<RangeInclusive<f64>>,
    radius: f64,
    inv_rad: f64,
}

impl ArcPrecomp {
    fn new(inner: Arc) -> Self {
        let range_pos = cos_range(inner.min_angle_magic, inner.max_angle_magic, true);
        let range_neg = cos_range(inner.min_angle_magic, inner.max_angle_magic, false);
        let radius = inner.radius as f64;
        let inv_rad = 1.0 / radius;
        Self { inner, range_pos, range_neg, radius, inv_rad }
    }
}

impl Collisions for ArcPrecomp {
    fn rows(&self) -> RangeInclusive<f32> {
        self.inner.rows()
    }

    fn collision<G: FnMut(f32)>(&self, row: f32, mut col_check: G) {
        let sin_radius = row - self.inner.center.y;

        if !(-self.inner.radius..=self.inner.radius).contains(&sin_radius) {
            return;
        }

        let range = match sin_radius < 0.0 {
            true => &self.range_pos,
            false => &self.range_neg,
        };

        let Some(range) = range else {
            return;
        };

        let sin = (sin_radius as f64) * self.inv_rad;
        let r_cos = f64::sqrt(1.0 - sin * sin);

        if range.contains(&r_cos) ^ self.inner.wraps {
            col_check(self.inner.center.x + (r_cos * self.radius) as f32);
        }

        let l_cos = -r_cos;

        if range.contains(&l_cos) ^ self.inner.wraps {
            col_check(self.inner.center.x + (l_cos * self.radius) as f32);
        }
    }
}

pub(crate) fn cos_range(min_angle_magic: u32, max_angle_magic: u32, row_pos: bool) -> Option<RangeInclusive<f64>> {
    let (min_cos, min_neg) = primitives::decode_magic(min_angle_magic);
    let (max_cos, max_neg) = primitives::decode_magic(max_angle_magic);

    // row_pos = row < arc.center.y (its circle position is a positive angle)

    let (cos_a, cos_b) = match (min_neg, max_neg, row_pos) {
        (true, true, true) => return None, // no intersection
        (true, true, false) => (min_cos, max_cos),
        (true, false, true) => (1.0, max_cos),
        (true, false, false) => (min_cos, 1.0),
        (false, false, true) => (min_cos, max_cos),
        (false, false, false) => return None, // no intersection
        (false, true, _) => unreachable!(),
    };

    let (min_cos, max_cos) = match cos_a <= cos_b {
        true => (cos_a, cos_b),
        false => (cos_b, cos_a),
    };

    Some(min_cos..=max_cos)
}
