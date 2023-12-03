#![doc = include_str!("../README.md")]

// #![no_std]
#![cfg_attr(feature = "simd", feature(portable_simd))]
#![cfg_attr(feature = "simd", feature(slice_flatten))]

extern crate alloc;
use alloc::{vec::Vec, vec};

use vek::vec::Vec2;

#[allow(unused_imports)]
use vek::num_traits::Float;

const MAX_SIMD_LANES: usize = 8;
const AABB_SAFE_MARGIN: f32 = 1.0;

// lower is better, higher is cheaper
// more than one => glitchy
const STRAIGHT_THRESHOLD: f32 = 0.5;

mod seq;

#[cfg(feature = "simd")]
mod simd;

#[cfg(not(feature = "simd"))]
mod simd {
    pub fn pixel_opacity<const P: usize>(p: Point, path: &[CubicBezier]) -> u8 {
        seq::pixel_opacity::<P>(p, path)
    }
}

type Point = Vec2<f32>;
type BoundingBox = Vec2<(f32, f32)>;

/// Cubic Bezier Curve, made of 4 control points
#[derive(Copy, Clone, Debug, Default)]
pub struct CubicBezier {
    pub c1: Point,
    pub c2: Point,
    pub c3: Point,
    pub c4: Point,
}

impl CubicBezier {
    fn split(self, t: f32) -> (Self, Self) {

        #[inline(always)]
        fn travel(a: Point, b: Point, t: f32) -> Point {
            Point {
                x: a.x + (b.x - a.x) * t,
                y: a.y + (b.y - a.y) * t,
            }
        }

        // step 1: take 2nd half of self

        let side1 = travel(self.c1, self.c2, t);
        let side2 = travel(self.c2, self.c3, t);
        let side3 = travel(self.c3, self.c4, t);

        let diag1 = travel(side1, side2, t);
        let diag2 = travel(side2, side3, t);

        let split_point = travel(diag1, diag2, t);

        let first_half = Self {
            c1: self.c1,
            c2: side1,
            c3: diag1,
            c4: split_point,
        };

        let second_half = Self {
            c1: split_point,
            c2: diag2,
            c3: side3,
            c4: self.c4,
        };

        (first_half, second_half)
    }

    fn aabb(&self) -> BoundingBox {
        let min_x = self.c1.x.min(self.c2.x).min(self.c3.x).min(self.c4.x);
        let max_x = self.c1.x.max(self.c2.x).max(self.c3.x).max(self.c4.x);

        let min_y = self.c1.y.min(self.c2.y).min(self.c3.y).min(self.c4.y);
        let max_y = self.c1.y.max(self.c2.y).max(self.c3.y).max(self.c4.y);

        BoundingBox::new((min_x, max_x), (min_y, max_y))
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

/// A 4-byte color (RGBA)
pub type Color = rgb::RGBA<u8>;

/// Drawing Surface
#[derive(Debug, Clone)]
pub struct Canvas {
    mask: Vec<u8>,
    pixels: Vec<Color>,
    width: usize,
    height: usize,
}

/// Super-Sampling Anti-Aliasing Configuration
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SsaaConfig {
    None,
    X2,
    X4,
    X8,
    X16,
}

impl Canvas {
    pub fn new(width: usize, height: usize) -> Self {
        let sz = width * height;
        Self {
            mask: vec![0; sz],
            pixels: vec![Default::default(); sz],
            width,
            height,
        }
    }

    /// Sets all pixels to fully transparent
    pub fn clear(&mut self) {
        self.pixels.fill(Default::default());
    }

    /// Retrieves the inner pixel buffer, which has a size of width x height
    pub fn pixels(&self) -> &[Color] {
        &self.pixels
    }

    /// Fills a shape delimited by a path, which is a sequence of cubic bezier curves
    ///
    /// In the `path` slice, a curve at index N must end where the N+1 curve starts.
    /// Additionally, the last curve must end where the first one starts.
    ///
    /// This function first locates the pixels which are inside the shape, creating a blending mask.
    /// Then, it calls the `texture_sample` function for each of these pixels.
    /// The returned color is finally applied to the inner pixel buffer (taking
    /// transparency into account).
    ///
    /// If `try_simd` is true and the `simd` feature is enabled, the blending mask is
    /// created using a parallel algorithm. `ssaa` determines how much anti-aliasing to
    /// apply to the blending mask. Using SIMD is only advised when `ssaa` isn't `None`.
    ///
    /// This function doesn't allocate.
    pub fn fill<F: Fn(usize, usize) -> Color>(
        &mut self,
        path: &[CubicBezier],
        texture_sample: F,
        try_simd: bool,
        ssaa: SsaaConfig,
    ) {
        if path.is_empty() {
            return;
        }

        let w_f = self.width as f32;
        let h_f = self.height as f32;
        let x_lim = w_f - 1.0;
        let y_lim = h_f - 1.0;

        // determine minimal canvas rectangle

        let mut aabb = BoundingBox::new((w_f, 0.0), (h_f, 0.0));

        for curve in path {
            aabb = combine_aabb(aabb, curve.aabb());
        }

        let min_x = (aabb.x.0 - AABB_SAFE_MARGIN).clamp(0.0, x_lim);
        let max_x = (aabb.x.1 + AABB_SAFE_MARGIN).clamp(0.0, x_lim);
        let min_y = (aabb.y.0 - AABB_SAFE_MARGIN).clamp(0.0, y_lim);
        let max_y = (aabb.y.1 + AABB_SAFE_MARGIN).clamp(0.0, y_lim);

        let min_x_i = min_x as usize;
        let max_x_i = max_x as usize;
        let min_y_i = min_y as usize;
        let max_y_i = max_y as usize;

        for y in min_y_i..=max_y_i {
            let line_offset = y * self.width;
            self.mask[line_offset..][min_x_i..=max_x_i].fill(u8::MIN);
        }

        // accept segments if:
        // - it's axis-aligned
        // - the pixel area is smaller than 9
        // else:
        //   half the time increment
        for curve in path {
            const SMALL_PX_AREA: f32 = 4.0;
            let mut t0 = 0.0;
            let mut rem_sc = *curve;
            let mut step = 1.0;

            while t0 < 1.0 {
                let t1 = (t0 + step).min(1.0);

                // the AABB of [curve(t0), curve(t1)] doesn't always cover all curve points,
                // so we must either use
                // - the AABB of all points of a subcurve
                // - the AABB of all control points of a subcurve (cheaper; what we do)
                // both of which cover all curve points.
                let remaining = 1.0 - t0;
                let step2t = (t1 - t0) / remaining;
                let (trial_sc, future_sc) = rem_sc.split(step2t);
                let trial_aabb = trial_sc.aabb();

                let diff_f32 = |(a, b): (f32, f32)| (a - b).abs();
                let same_f32 = |tuple| diff_f32(tuple) < f32::EPSILON;

                let axis_aligned = |aabb: BoundingBox| same_f32(aabb.x) || same_f32(aabb.y);
                let small_pixel_area = |aabb: BoundingBox| diff_f32(aabb.x) * diff_f32(aabb.y) < SMALL_PX_AREA;

                if axis_aligned(trial_aabb) || small_pixel_area(trial_aabb) {

                    let min_x_sc = (trial_aabb.x.0 - AABB_SAFE_MARGIN) as usize;
                    let max_x_sc = (trial_aabb.x.1 + AABB_SAFE_MARGIN) as usize;
                    let min_y_sc = (trial_aabb.y.0 - AABB_SAFE_MARGIN) as usize;
                    let max_y_sc = (trial_aabb.y.1 + AABB_SAFE_MARGIN) as usize;

                    for y in min_y_sc..=max_y_sc {
                        let line_offset = y * self.width;
                        self.mask[line_offset..][min_x_sc..=max_x_sc].fill(u8::MAX);
                    }

                    t0 = t1;
                    rem_sc = future_sc;

                } else {
                    step = step.min(remaining) * 0.5;
                }
            }
        }

        let mut line = &mut self.mask[min_y_i * self.width..];
        for y in min_y_i..=max_y_i {
            let mut go_back = 0;

            for x in min_x_i..=max_x_i {
                let not_last_point = x != max_x_i;
                let point = Point::new(x as f32, y as f32);

                if line[x] == 0 && not_last_point {
                    go_back += 1;
                } else {
                    let opacity = match (try_simd, ssaa) {
                        (false, SsaaConfig::None) => seq::pixel_opacity::< 1>(point, path),
                        (false, SsaaConfig::X2  ) => seq::pixel_opacity::< 2>(point, path),
                        (false, SsaaConfig::X4  ) => seq::pixel_opacity::< 4>(point, path),
                        (false, SsaaConfig::X8  ) => seq::pixel_opacity::< 8>(point, path),
                        (false, SsaaConfig::X16 ) => seq::pixel_opacity::<16>(point, path),
                        (true, SsaaConfig::None) => simd::pixel_opacity::< 1>(point, path),
                        (true, SsaaConfig::X2  ) => simd::pixel_opacity::< 2>(point, path),
                        (true, SsaaConfig::X4  ) => simd::pixel_opacity::< 4>(point, path),
                        (true, SsaaConfig::X8  ) => simd::pixel_opacity::< 8>(point, path),
                        (true, SsaaConfig::X16 ) => simd::pixel_opacity::<16>(point, path),
                    };

                    line[(x - go_back)..=x].fill(opacity);

                    go_back = 0;
                }
            }

            line = &mut line[self.width..];
        }

        for y in min_y_i..=max_y_i {
            let line_offset = y * self.width;

            for x in min_x_i..=max_x_i {
                let i = line_offset + x;
                let opacity = self.mask[i];

                if opacity > 0 {
                    self.pixels[i] = blend(texture_sample(x, y), self.pixels[i], opacity);
                }
            }
        }
    }
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

/// Debugging texture showing a continuous rainbow
///
/// Stripes are diagonal and separated by a transparent line.
pub fn rainbow(x: usize, y: usize) -> Color {
    let i = ((x + y) % 128) >> 4;

    [
        Color::new(255,   0,   0, 255),
        Color::new(255, 127,   0, 255),
        Color::new(255, 255,   0, 255),
        Color::new(  0, 255,   0, 255),
        Color::new(  0,   0, 255, 255),
        Color::new( 75,   0, 130, 255),
        Color::new(148,   0, 211, 255),
        Color::new(  0,   0,   0,   0),
    ][i]
}

const fn ssaa_subpixel_map<const P: usize>() -> &'static [(f32, f32)] {
    match P {
        1 => &[(0.0, 0.0)],
        2 => &[(-0.25, -0.25), (0.25, 0.25)],
        4 => &[
            (-0.25, -0.25),
            (-0.25,  0.25),
            ( 0.25, -0.25),
            ( 0.25,  0.25),
        ],
        8 => &[
            (-0.125, -0.125),
            (-0.375, -0.375),

            (-0.125,  0.125),
            (-0.375,  0.375),

            ( 0.125, -0.125),
            ( 0.375, -0.375),

            ( 0.125,  0.125),
            ( 0.375,  0.375),
        ],
        16 => &[
            (-0.125, -0.125),
            (-0.375, -0.375),

            (-0.125,  0.125),
            (-0.375,  0.375),

            ( 0.125, -0.125),
            ( 0.375, -0.375),

            ( 0.125,  0.125),
            ( 0.375,  0.375),

            (-0.125, -0.375),
            (-0.375, -0.125),

            (-0.125,  0.375),
            (-0.375,  0.125),

            ( 0.125, -0.375),
            ( 0.375, -0.125),

            ( 0.125,  0.375),
            ( 0.375,  0.125),
        ],
        _ => panic!("unsupported SSAA configuration"),
    }
}

#[allow(unused_variables, dead_code)]
fn striking_path(outline: &[CubicBezier], width: f32, output: &mut Vec<CubicBezier>) {
    todo!()
}

