#![doc = include_str!("../README.md")]

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(feature = "simd", feature(portable_simd))]

extern crate alloc;
pub extern crate rgb;
pub extern crate vek;

#[cfg(feature = "std")]
extern crate std;

use vek::vec::Vec2;

#[allow(unused_imports)]
use vek::num_traits::Float;

#[cfg(any(doc, feature = "contour"))]
mod contour;

#[cfg(any(doc, feature = "contour"))]
pub use contour::contour;

#[cfg(any(doc, feature = "shapes"))]
pub mod shapes;

/// Implementations of [`Canvas`] using only the CPU
pub mod cpu;

/// Implementations of [`Canvas`] using OpenGL ES 2.0
#[cfg(feature = "gles2")]
pub mod gles2;

// const AABB_SAFE_MARGIN: f32 = 1.0;

const TRANSPARENT: Color = Color::new(0, 0, 0, 0);

// lower is better, higher is cheaper
// more than one => glitchy
const STRAIGHT_THRESHOLD: f32 = 0.8;

/// Texture handle created using a [`Canvas`]
#[derive(Copy, Clone, Debug)]
pub struct BitmapHandle(usize);

impl BitmapHandle {
    /// Forge a [`BitmapHandle`]
    ///
    /// This function is made available for implementations of [`Canvas`] outside of this crate.
    pub unsafe fn forge(inner: usize) -> Self {
        Self(inner)
    }

    /// Leak the inner usize
    ///
    /// This function is made available for implementations of [`Canvas`] outside of this crate.
    pub unsafe fn leak(&self) -> usize {
        self.0
    }
}

/// The expected content of a filled path
#[derive(Copy, Clone, Debug)]
pub enum Texture<'a> {
    SolidColor(Color),
    /// Not supported yet
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

/// Pixel or Subpixel coordinates
pub type Point = Vec2<f32>;

#[derive(Copy, Clone)]
struct BoundingBox {
    min: Point,
    max: Point,
}

impl Default for BoundingBox {
    fn default() -> Self {
        Self {
            min: Point::new(f32::INFINITY, f32::INFINITY),
            max: Point::new(f32::NEG_INFINITY, f32::NEG_INFINITY),
        }
    }
}

impl BoundingBox {
    fn overlaps_with(&self, other: BoundingBox) -> bool {
        let x_overlap = (self.min.x <= other.max.x) & (self.max.x >= other.min.x);
        let y_overlap = (self.min.y <= other.max.y) & (self.max.y >= other.min.y);
        x_overlap & y_overlap
    }

    fn union(&self, other: BoundingBox) -> Self {
        let min_x = self.min.x.min(other.min.x);
        let min_y = self.min.y.min(other.min.y);
        let max_x = self.max.x.max(other.max.x);
        let max_y = self.max.y.max(other.max.y);

        Self {
            min: Point::new(min_x, min_y),
            max: Point::new(max_x, max_y),
        }
    }
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

/// The trait that rendering backends must implement
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
    fn fill_cbc(&mut self, cbc: &[CubicBezier], texture: &Texture, ssaa: SsaaConfig);
}

#[inline(always)]
fn travel(a: Point, b: Point, t: f32) -> Point {
    Point {
        x: a.x + (b.x - a.x) * t,
        y: a.y + (b.y - a.y) * t,
    }
}

impl CubicBezier {
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
        BoundingBox {
            min: Point::new(min_x, min_y),
            max: Point::new(max_x, max_y),
        }
    }

    fn split_4(&self) -> [Self; 4] {
        let (ab, cd) = self.split(0.5);
        let (a, b) = ab.split(0.5);
        let (c, d) = cd.split(0.5);
        [a, b, c, d]
    }

    fn overlaps(&self, tile: BoundingBox) -> bool {
        if self.aabb().overlaps_with(tile) {
            let [a, b, c, d] = self.split_4();
            let (aabb_1, aabb_2) = (a.aabb(), b.aabb());
            let (aabb_3, aabb_4) = (c.aabb(), d.aabb());

            aabb_1.overlaps_with(tile) || aabb_2.overlaps_with(tile) ||
            aabb_3.overlaps_with(tile) || aabb_4.overlaps_with(tile)
        } else {
            false
        }
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

type SubPixelOffsets = &'static [(f32, f32)];

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
    fn as_mul<T: From<u8>>(&self) -> T {
        match self {
            SsaaConfig::None =>  1,
            SsaaConfig::X2   =>  2,
            SsaaConfig::X4   =>  4,
            SsaaConfig::X8   =>  8,
            SsaaConfig::X16  => 16,
        }.into()
    }

    const fn offsets(&self) -> SubPixelOffsets {
        match self {
            Self::None => &[(0.0, 0.0)],
            Self::X2 => &[(-0.25, -0.25), (0.25, 0.25)],
            Self::X4 => &[
                (-0.25, -0.25),
                (-0.25,  0.25),
                ( 0.25, -0.25),
                ( 0.25,  0.25),
            ],
            Self::X8 => &[
                (-0.125, -0.125),
                (-0.375, -0.375),

                (-0.125,  0.125),
                (-0.375,  0.375),

                ( 0.125, -0.125),
                ( 0.375, -0.375),

                ( 0.125,  0.125),
                ( 0.375,  0.375),
            ],
            Self::X16 => &[
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
        }
    }
}

// looks dumb but improves performance
// because the operand is a const
#[doc(hidden)]
#[macro_export]
macro_rules! const_ssaa {
    ($ssaa:expr, $a:expr, $op:tt) => {
        match $ssaa {
            SsaaConfig::None => $a $op 1,
            SsaaConfig::X2 => $a $op 2,
            SsaaConfig::X4 => $a $op 4,
            SsaaConfig::X8 => $a $op 8,
            SsaaConfig::X16 => $a $op 16,
        }
    }
}
