use super::*;

use alloc::{vec, vec::Vec, boxed::Box};

mod bitmap;
mod worker;
mod texture;

mod seq;

#[cfg(any(doc, feature = "simd"))]
mod simd;

use worker::Worker;
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
    mask: Box<[u8]>,
    size: Vec2<usize>,
    worker: Worker,
}

impl Canvas {
    /// Create a basic in-memory canvas
    pub fn new_seq(width: usize, height: usize) -> Canvas {
        let sz = width * height;
        Canvas {
            bitmaps: Bitmaps::new(),
            pixels: vec![Default::default(); sz].into(),
            mask: vec![0; sz].into(),
            size: Vec2::new(width, height),
            worker: Worker::new(worker::seq_sample),
        }
    }

    /// Create a SIMD-accelerated in-memory canvas
    ///
    /// Available when the `simd` feature is enabled.
    ///
    #[cfg(any(feature = "simd", doc))]
    pub fn new_simd(width: usize, height: usize) -> Canvas {
        let sz = width * height;
        Canvas {
            bitmaps: Bitmaps::new(),
            pixels: vec![Default::default(); sz].into(),
            mask: vec![0; sz].into(),
            size: Vec2::new(width, height),
            worker: Worker::new(worker::simd_sample),
        }
    }

    fn tex_sample(
        &self,
        pixel: Point,
        texture: &Texture,
        ssaa: SsaaConfig,
    ) -> Color {
        let sampler = match ssaa {
            SsaaConfig::None => Texture::sample::< 1>,
            SsaaConfig::X2   => Texture::sample::< 2>,
            SsaaConfig::X4   => Texture::sample::< 4>,
            SsaaConfig::X8   => Texture::sample::< 8>,
            SsaaConfig::X16  => Texture::sample::<16>,
        };

        sampler(texture, pixel, &self.bitmaps)
    }

    pub fn pixels(&self) -> &[Color] {
        &self.pixels
    }
}

fn add_margin(aabb: BoundingBox, x_lim: f32, y_lim: f32) -> Vec2<(usize, usize)> {
    let min_x = (aabb.x.0 - AABB_SAFE_MARGIN).clamp(0.0, x_lim) as usize;
    let max_x = (aabb.x.1 + AABB_SAFE_MARGIN).clamp(0.0, x_lim) as usize;
    let min_y = (aabb.y.0 - AABB_SAFE_MARGIN).clamp(0.0, y_lim) as usize;
    let max_y = (aabb.y.1 + AABB_SAFE_MARGIN).clamp(0.0, y_lim) as usize;

    Vec2::new((min_x, max_x), (min_y, max_y))
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
        holes: bool,
        ssaa: SsaaConfig,
    ) {
        if path.is_empty() {
            return;
        }

        let w_f = self.size.x as f32;
        let h_f = self.size.y as f32;
        let x_lim = w_f - 1.0;
        let y_lim = h_f - 1.0;

        let mut aabb = BoundingBox::new((w_f, 0.0), (h_f, 0.0));

        for curve in path {
            aabb = combine_aabb(aabb, curve.aabb());
        }

        let aabb = add_margin(aabb, x_lim, y_lim);

        for y in aabb.y.0..=aabb.y.1 {
            let line_offset = y * self.size.x;
            self.mask[line_offset..][aabb.x.0..=aabb.x.1].fill(u8::MIN);
        }

        for curve in path {
            const TRESHOLD: f32 = 2.0;
            let mut t = 1.0;
            let mut curve = *curve;

            loop {
                let (half_1, half_2) = curve.split(t);
                let big_len = half_1.quick_max_len();

                if cpu::seq::is_curve_straight(half_1) || big_len < TRESHOLD {
                    let aabb_c = add_margin(half_1.aabb(), x_lim, y_lim);

                    for y in aabb_c.y.0..=aabb_c.y.1 {
                        let line_offset = y * self.size.x;
                        self.mask[line_offset..][aabb_c.x.0..=aabb_c.x.1].fill(u8::MAX);
                    }

                    if t == 1.0 {
                        break;
                    }

                    curve = half_2;
                    t = 1.0;
                } else {
                    t *= 0.1;
                }
            }
        }

        for y in aabb.y.0..=aabb.y.1 {
            for x in aabb.x.0..=aabb.x.1 {
                let i = y * self.size.x + x;

                let should_compute = self.mask[i] == u8::MAX;
                let last_of_line = x == aabb.x.1;

                if should_compute || last_of_line {
                    let point = Point::new(x as f32, y as f32);
                    self.mask[i] = 1;
                    self.worker.queue_ssaa(i, point, ssaa);
                    self.worker.try_advance(path, &mut self.mask, holes);
                }
            }
        }
        
        self.worker.force_advance(path, &mut self.mask, holes);

        let mut line = &mut self.mask[aabb.y.0 * self.size.x..];
        for _ in aabb.y.0..=aabb.y.1 {
            let mut go_back = 0;

            for x in aabb.x.0..=aabb.x.1 {
                if line[x] == 0 {
                    go_back += 1;
                } else {
                    let opacity = line[x] - 1;
                    line[(x - go_back)..=x].fill(opacity);
                    go_back = 0;
                }
            }

            line = &mut line[self.size.x..];
        }

        for y in aabb.y.0..=aabb.y.1 {
            let line_offset = y * self.size.x;

            for x in aabb.x.0..=aabb.x.1 {
                let i = line_offset + x;
                let opacity = self.mask[i];

                if opacity > 0 {
                    let opacity = 255u16 * (opacity as u16) / ssaa.as_mul();
                    let point = Point::new(x as f32, y as f32);
                    let color = self.tex_sample(point, texture, ssaa);
                    self.pixels[i] = blend(color, self.pixels[i], opacity as u8);
                }
            }
        }
    }
}

#[inline(always)]
fn combine_aabb(a: BoundingBox, b: BoundingBox) -> BoundingBox {
    let min_x = a.x.0.min(b.x.0);
    let max_x = a.x.1.max(b.x.1);

    let min_y = a.y.0.min(b.y.0);
    let max_y = a.y.1.max(b.y.1);

    BoundingBox::new((min_x, max_x), (min_y, max_y))
}

fn blend(src: Color, dst: Color, opacity: u8) -> Color {
    let mut src = rgb::RGBA::new(src.r as u16, src.g as u16, src.b as u16, src.a as u16);
    let     dst = rgb::RGBA::new(dst.r as u16, dst.g as u16, dst.b as u16, dst.a as u16);
    let u8_max = u8::MAX as u16;

    src.a *= opacity as u16;
    src.a /= u8_max;

    let dst_a = u8_max - src.a;

    let out_r = (src.r * src.a + dst.r * dst_a) / u8_max;
    let out_g = (src.g * src.a + dst.g * dst_a) / u8_max;
    let out_b = (src.b * src.a + dst.b * dst_a) / u8_max;
    let out_a = (src.a * src.a + dst.a * dst_a) / u8_max;

    Color::new(out_r as u8, out_g as u8, out_b as u8, out_a as u8)
}
