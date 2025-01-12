use rgb::ComponentMap;
use super::*;

use alloc::{vec, vec::Vec, boxed::Box};

mod bitmap;
mod texture;
mod tile;

// mod seq;

#[cfg(any(doc, feature = "simd"))]
mod simd;

type IntPoint = Vec2<i32>;
const POINTS: usize = 64;
const PX_WIDTH: i32 = 32;

type MaskRow = u32;
const TILE_W: usize = MaskRow::BITS as usize;
const TILE_H: usize = 32;

const DEBUG_NON_OVERLAPPING: bool = false;

type Color16 = rgb::RGBA::<u16>;

use tile::TileIterator;
use bitmap::Bitmaps;

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

type Mask = [MaskRow; TILE_H];

impl Canvas {
    /// Create a basic in-memory canvas
    pub fn new(width: usize, height: usize) -> Canvas {
        let sz = width * height;
        Canvas {
            bitmaps: Bitmaps::new(),
            pixels: vec![Default::default(); sz].into(),
            mask: vec![0; TILE_H].try_into().unwrap(),
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

    fn fill_cbc(
        &mut self,
        path: &[CubicBezier],
        texture: &Texture,
        ssaa: SsaaConfig,
    ) {
        for mut tile in self.tiles(ssaa) {
            if path.iter().any(|c| c.overlaps(tile.aabb)) {
                let mut last_end = path.last().map(|c| c.c4);
                for curve in path {
                    assert_eq!(Some(curve.c1), last_end);
                    last_end = Some(curve.c4);
                    tile.advance(*curve, &mut self.mask);
                }

                tile.mask_pass(&mut self.mask);

                tile.render(
                    &mut self.pixels,
                    self.size,
                    &self.mask,
                    texture,
                    &self.bitmaps,
                );

                self.mask.fill(0);

            } else if tile.sample_oob(path) {

                tile.render_all(
                    &mut self.pixels,
                    self.size,
                    texture,
                    &self.bitmaps,
                );

            }
        }
    }
}

fn is_curve_straight(curve: CubicBezier) -> bool {
    let close_enough = |p: Point| {
        // https://en.wikipedia.org/wiki/Distance_from_a_point_to_a_line#Line_defined_by_two_points

        let l = curve.c4 - curve.c1;
        let a = l.x * (curve.c1.y - p.y);
        let b = l.y * (curve.c1.x - p.x);

        // distance from p to projected point
        let distance = (a - b).abs() * fast_inv_sqrt(l.x * l.x + l.y * l.y);

        distance < STRAIGHT_THRESHOLD
    };

    close_enough(curve.c2) && close_enough(curve.c3)
}

// |num| 1.0 / num.sqrt()
#[inline(always)]
fn fast_inv_sqrt(num: f32) -> f32 {
    f32::from_bits(0x5f37_5a86 - (num.to_bits() >> 1))
}

#[inline(always)]
fn blend(src: Color16, dst: Color) -> Color {
    match src.a {
        255 => return src.map(|c| c as u8),
        0 => return dst,
        _ => (),
    };

    let dst = rgb::RGBA::new(dst.r as u16, dst.g as u16, dst.b as u16, dst.a as u16);

    let u8_max = u8::MAX as u16;
    let dst_a = u8_max - src.a;

    let out_r = (src.r * src.a + dst.r * dst_a) / u8_max;
    let out_g = (src.g * src.a + dst.g * dst_a) / u8_max;
    let out_b = (src.b * src.a + dst.b * dst_a) / u8_max;
    let out_a = (src.a * src.a + dst.a * dst_a) / u8_max;

    Color::new(out_r as u8, out_g as u8, out_b as u8, out_a as u8)
}
