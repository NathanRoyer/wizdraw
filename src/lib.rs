#![allow(dead_code)]
#![doc = include_str!("../README.md")]

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(feature = "simd", feature(portable_simd))]
#![cfg_attr(feature = "simd", feature(slice_flatten))]

extern crate alloc;
extern crate std;

use vek::vec::Vec2;

#[allow(unused_imports)]
use vek::num_traits::Float;

pub mod util;
pub mod cpu;

// pub mod opengl;

pub use rgb;

#[cfg(feature = "simd")]
const MAX_SIMD_LANES: usize = 8;
const AABB_SAFE_MARGIN: f32 = 1.0;

const TRANSPARENT: Color = Color::new(0, 0, 0, 0);

// lower is better, higher is cheaper
// more than one => glitchy
const STRAIGHT_THRESHOLD: f32 = 0.5;

const DEG_90: f32 = core::f32::consts::PI * 0.5;

/// Super-Sampling Anti-Aliasing Configuration
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SsaaConfig {
    None,
    X2,
    X4,
    X8,
    X16,
}

impl SsaaConfig {
    fn as_mul(&self) -> u16 {
        match self {
            SsaaConfig::None =>  1,
            SsaaConfig::X2   =>  2,
            SsaaConfig::X4   =>  4,
            SsaaConfig::X8   =>  8,
            SsaaConfig::X16  => 16,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct BitmapHandle(usize);

#[derive(Copy, Clone, Debug)]
pub enum Texture<'a> {
    SolidColor(Color),
    Gradient(&'a [(Point, Color)]),
    Bitmap {
        top_left: Point,
        scale: f32,
        repeat: bool,
        bitmap: BitmapHandle,
    },
    QuadBitmap {
        top_left: Point,
        btm_left: Point,
        top_right: Point,
        btm_right: Point,
        bitmap: BitmapHandle,
    },
    Debug,
}

/// A 4-byte color (RGBA)
pub type Color = rgb::RGBA<u8>;
pub type Point = Vec2<f32>;
type BoundingBox = Vec2<(f32, f32)>;

#[derive(Copy, Clone, Debug, Default)]
#[repr(C)]
pub struct Rectangle {
    top_left: Point,
    size: Vec2<f32>,
    border_radius: f32,
}

/// Cubic Bezier Curve, made of 4 control points
#[derive(Copy, Clone, Debug, Default)]
#[repr(C)]
pub struct CubicBezier {
    pub c1: Point,
    pub c2: Point,
    pub c3: Point,
    pub c4: Point,
}

pub trait Canvas {
    fn framebuffer_size(&self) -> Vec2<usize>;

    fn alloc_bitmap(&mut self, width: usize, height: usize) -> BitmapHandle;
    fn fill_bitmap(&mut self, bitmap: BitmapHandle, x: usize, y: usize, w: usize, h: usize, buf: &[Color]);
    fn free_bitmap(&mut self, bitmap: BitmapHandle);

    fn clear(&mut self);

    /// Fills a shape delimited by a path, which is a sequence of cubic bezier curves
    ///
    /// The shape must be a [Composite Bezier Curve](https://en.wikipedia.org/wiki/Composite_B%C3%A9zier_curve).
    /// In other words: in the `path` slice, a curve at index N must end where the N+1 curve starts;
    /// additionally, the last curve must end where the first one starts.
    ///
    /// If `holes` is true, path holes won't be filled; if it's false, path holes will be filled too.
    /// If this sounds unclear, read the [Wikipedia entry on Winding Numbers](https://en.wikipedia.org/wiki/Winding_number):
    /// Pixels which yield winding numbers other than -1, 0 and 1 are in holes.
    ///
    fn fill_cbc(&mut self, cbc: &[CubicBezier], texture: &Texture, holes: bool, ssaa: SsaaConfig);
}

#[inline(always)]
fn travel(a: Point, b: Point, t: f32) -> Point {
    Point {
        x: a.x + (b.x - a.x) * t,
        y: a.y + (b.y - a.y) * t,
    }
}

impl CubicBezier {
    fn reversed(self, actually: bool) -> Self {
        match actually {
            true => CubicBezier {
                c1: self.c4,
                c2: self.c3,
                c3: self.c2,
                c4: self.c1,
            },
            false => self,
        }
    }

    fn offset(&self, normal_factor: f32) -> Self {
        let side1 = travel(self.c1, self.c2, 0.5);
        let side2 = travel(self.c2, self.c3, 0.5);
        let side3 = travel(self.c3, self.c4, 0.5);

        let this_norm_c1 = (self.c2 - self.c1).normalized().rotated_z(DEG_90) * normal_factor;
        let this_norm_c2 = (side2 - side1).normalized().rotated_z(DEG_90) * normal_factor;
        let this_norm_c3 = (side3 - side2).normalized().rotated_z(DEG_90) * normal_factor;
        let this_norm_c4 = (self.c4 - self.c3).normalized().rotated_z(DEG_90) * normal_factor;


        CubicBezier {
            c1: self.c1 + this_norm_c1,
            c2: self.c2 + this_norm_c2,
            c3: self.c3 + this_norm_c3,
            c4: self.c4 + this_norm_c4,
        }
    }

    // along normal
    fn eval_and_offset(&self, t: f32, normal_factor: f32) -> Point {
        let side1 = travel(self.c1, self.c2, t);
        let side2 = travel(self.c2, self.c3, t);
        let side3 = travel(self.c3, self.c4, t);

        let diag1 = travel(side1, side2, t);
        let diag2 = travel(side2, side3, t);

        let split_point = travel(diag1, diag2, t);
        let offset = (diag2 - diag1).normalized().rotated_z(DEG_90) * normal_factor;

        split_point + offset
    }

    // used by util::contour
    fn max_offset_error(&self, offset_curve: &Self, offset: f32, steps: usize) -> f32 {
        let step_inc = 1.0 / (steps as f32);
        let mut t = step_inc;
        let mut max_error = 0.0;

        for _ in 0..(steps - 1) {
            let expected = self.eval_and_offset(t, offset);
            let actual = offset_curve.eval_and_offset(t, 0.0);
            let error = expected.distance(actual);

            if max_error < error {
                max_error = error;
            }

            t += step_inc;
        }

        max_error
    }

    fn eval(self, t: f32) -> Point {
        let side1 = travel(self.c1, self.c2, t);
        let side2 = travel(self.c2, self.c3, t);
        let side3 = travel(self.c3, self.c4, t);

        let diag1 = travel(side1, side2, t);
        let diag2 = travel(side2, side3, t);

        travel(diag1, diag2, t)
    }

    fn quick_max_len(&self) -> f32 {
        let a = self.c1.distance_squared(self.c2);
        let b = self.c2.distance_squared(self.c3);
        let c = self.c3.distance_squared(self.c4);

        a + b + c
    }

    fn split(self, t: f32) -> (Self, Self) {
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
        let (min_x, max_x) = min_max([self.c1.x, self.c2.x, self.c3.x, self.c4.x]);
        let (min_y, max_y) = min_max([self.c1.y, self.c2.y, self.c3.y, self.c4.y]);
        BoundingBox::new((min_x, max_x), (min_y, max_y))
    }
}

fn min_max(input: [f32; 4]) -> (f32, f32) {
    let mut min = f32::MAX;
    let mut max = f32::MIN;

    for float in input {
        if float < min {
            min = float;
        }
        if float > max {
            max = float;
        }
    }

    (min, max)
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
