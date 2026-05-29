#![doc = include_str!("../README.md")]

// #![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;
pub extern crate rgb;
pub extern crate vek;

#[cfg(feature = "std")]
extern crate std;

use core::ops::RangeInclusive;
use alloc::vec::Vec;
use vek::vec::Vec2;

#[allow(unused_imports)]
use vek::num_traits::Float;

pub use ssaa::SsaaConfig;
mod ssaa;

pub use path::{Path, Transformer};
mod path;

mod stroke;

#[cfg(any(doc, feature = "shapes"))]
pub mod shapes;

/// Implementations of [`Canvas`] using only the CPU
pub mod cpu;

/// Implementations of [`Canvas`] using a GPU
#[cfg(feature = "gles2")]
pub mod gles2;

/// Text/Font/Glyph support
#[cfg(feature = "text")]
pub mod text;

const TRANSPARENT: Color = Color::new(0, 0, 0, 0);

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

type Point64 = Vec2<f64>;

#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Arc {
    pub center: Point,
    pub radius: f32,
    pub min_angle_magic: u32,
    pub max_angle_magic: u32,
    pub wraps: bool,
}

const CONV: f64 = 1073741823.5;

fn encode_magic_2(cos: f64, neg_angle: bool) -> u32 {
    let pos_cos = (cos + 1.0) * CONV;
    let base = pos_cos.trunc() as u32;
    (base << 1) | (neg_angle as u32)
}

fn encode_magic(angle: f32) -> u32 {
    encode_magic_2(f64::cos(angle as f64), angle < 0.0)
}

// returns (cos, neg_angle)
fn decode_magic(magic: u32) -> (f64, bool) {
    let neg_angle = (magic & 0b01) > 0;
    let raw_base = (magic >> 1) as f64;
    let cos = (raw_base / CONV) - 1.0;
    (cos, neg_angle)
}

impl Arc {
    pub fn new(center: Point, radius: f32, start_angle: f32, stop_angle: f32, clockwise: bool) -> Self {
        let sorted = start_angle < stop_angle;
        let wraps = clockwise == sorted;

        let (min_angle, max_angle) = match sorted {
            true => (start_angle, stop_angle),
            false => (stop_angle, start_angle),
        };

        let min_angle_magic = encode_magic(min_angle);
        let max_angle_magic = encode_magic(max_angle);

        Self { center, radius, min_angle_magic, max_angle_magic, wraps }
    }

    pub fn disk(center: Point, radius: f32) -> Self {
        let min_angle_magic = encode_magic_2(-1.0, true);
        let max_angle_magic = encode_magic_2(-1.0, false);
        Self { center, radius, min_angle_magic, max_angle_magic, wraps: false }
    }

    pub fn ends(&self) -> [Point; 2] {
        let (min_cos, min_neg) = decode_magic(self.min_angle_magic);
        let (max_cos, max_neg) = decode_magic(self.max_angle_magic);

        let mut min_sin = f64::sqrt(1.0 - min_cos * min_cos);
        let mut max_sin = f64::sqrt(1.0 - max_cos * max_cos);

        if min_neg { min_sin = -min_sin; }
        if max_neg { max_sin = -max_sin; }

        let radius = self.radius as f64;
        let neg_radius = -radius;
        let min_angle_x = self.center.x + (radius * min_cos) as f32;
        let max_angle_x = self.center.x + (radius * max_cos) as f32;
        let min_angle_y = self.center.y + (neg_radius * min_sin) as f32;
        let max_angle_y = self.center.y + (neg_radius * max_sin) as f32;

        [Point::new(min_angle_x, min_angle_y), Point::new(max_angle_x, max_angle_y)]
    }

    pub fn rows(&self) -> RangeInclusive<f32> {
        let (min_cos, min_neg) = decode_magic(self.min_angle_magic);
        let (max_cos, max_neg) = decode_magic(self.max_angle_magic);

        let top_max = (max_cos < 0.0) & !max_neg;
        let top_min = (min_cos > 0.0) | min_neg;
        let top = (top_min & top_max) ^ self.wraps;

        let btm_min = (min_cos < 0.0) & min_neg;
        let btm_max = (max_cos > 0.0) | !max_neg;
        let btm = (btm_min & btm_max) ^ self.wraps;

        let top_y = self.center.y - self.radius;
        let btm_y = self.center.y + self.radius;

        if top & btm {
            return top_y..=btm_y;
        }

        let mut min_sin = f64::sqrt(1.0 - min_cos * min_cos);
        let mut max_sin = f64::sqrt(1.0 - max_cos * max_cos);

        if !min_neg { min_sin = -min_sin; }
        if !max_neg { max_sin = -max_sin; }

        // screen coordinates
        let radius = self.radius as f64;
        let min_angle_y = self.center.y + (radius * min_sin) as f32;
        let max_angle_y = self.center.y + (radius * max_sin) as f32;

        // adding a bias for safety regarding sin results
        let bias = 1.0;
        let (mut min_y, mut max_y) = match min_angle_y <= max_angle_y {
            true => (min_angle_y - bias, max_angle_y + bias),
            false => (max_angle_y - bias, min_angle_y + bias),
        };

        if top { min_y = top_y; }
        if btm { max_y = btm_y; }

        min_y..=max_y
    }

    pub fn cols(&self) -> RangeInclusive<f32> {
        let (min_cos, min_neg) = decode_magic(self.min_angle_magic);
        let (max_cos, max_neg) = decode_magic(self.max_angle_magic);

        let r = min_neg & !max_neg;
        let l = self.wraps;

        let l_x = self.center.x - self.radius;
        let r_x = self.center.x + self.radius;

        if l & r {
            return l_x..=r_x;
        }

        let radius = self.radius as f64;
        let min_angle_x = self.center.x + (radius * min_cos) as f32;
        let max_angle_x = self.center.x + (radius * max_cos) as f32;

        // adding a bias for safety regarding cos results
        let bias = 1.0;
        let (mut min_x, mut max_x) = match min_angle_x <= max_angle_x {
            true => (min_angle_x - bias, max_angle_x + bias),
            false => (max_angle_x - bias, min_angle_x + bias),
        };

        if l { min_x = l_x; }
        if r { max_x = r_x; }

        min_x..=max_x
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Segment {
    pub a: Point,
    pub b: Point,
}

impl Segment {
    pub fn new(a: Point, b: Point) -> Self {
        Self { a, b }
    }

    pub fn rows(&self) -> RangeInclusive<f32> {
        match self.a.y <= self.b.y {
            true => self.a.y..=self.b.y,
            false => self.b.y..=self.a.y,
        }
    }
}

impl From<(Point, Point)> for Segment {
    fn from((a, b): (Point, Point)) -> Self {
        Self::new(a, b)
    }
}

pub trait PathApi {
    fn segment(&mut self, segment: Segment);
    fn arc(&mut self, arc: Arc);
}

pub trait Shape {
    fn draw(&self, api: &mut dyn PathApi);

    fn transform(&self, scale: f32, translation: Vec2<f32>) -> Transformer<&Self> where Self: Sized {
        Transformer {
            shape: self,
            scale,
            translation,
        }
    }
}

impl<T: Shape> Shape for &T {
    fn draw(&self, api: &mut dyn PathApi) {
        T::draw(self, api)
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Disk {
    pub center: Point,
    pub radius: f32,
}

impl Shape for Disk {
    fn draw(&self, api: &mut dyn PathApi) {
        api.arc(Arc::disk(self.center, self.radius));
    }
}

/// The trait that rendering backends must implement
pub trait Canvas {
    fn framebuffer_size(&self) -> Vec2<usize>;

    fn alloc_bitmap(&mut self, width: usize, height: usize) -> BitmapHandle;
    fn fill_bitmap(&mut self, bitmap: BitmapHandle, x: usize, y: usize, w: usize, h: usize, buf: &[Color]);
    fn free_bitmap(&mut self, bitmap: BitmapHandle);

    fn clear(&mut self);
    fn draw(&mut self, shape: &dyn Shape, texture: &Texture, ssaa: SsaaConfig);

    fn stroke(&mut self, shape: &dyn Shape, width: f32, texture: &Texture, ssaa: SsaaConfig) {
        stroke::stroke(self, shape, width, texture, ssaa);
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

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct BoundingBox {
    pub min: Point,
    pub max: Point,
}

impl Default for BoundingBox {
    fn default() -> Self {
        Self {
            min: Point::new(f32::INFINITY, f32::INFINITY),
            max: Point::new(-f32::INFINITY, -f32::INFINITY),
        }
    }
}

impl BoundingBox {
    fn check(&mut self, point: Point) {
        if point.x < self.min.x { self.min.x = point.x; }
        if point.x > self.max.x { self.max.x = point.x; }
        if point.y < self.min.y { self.min.y = point.y; }
        if point.y > self.max.y { self.max.y = point.y; }
    }
}

impl PathApi for BoundingBox {
    fn segment(&mut self, segment: Segment) {
        self.check(segment.a);
        self.check(segment.b);
    }

    fn arc(&mut self, arc: Arc) {
        let rows = arc.rows();
        let cols = arc.cols();

        let min = Point::new(*cols.start(), *rows.start());
        let max = Point::new(*cols.end(), *rows.end());
        self.check(min);
        self.check(max);
    }
}

fn rotate_90deg<T: std::ops::Neg<Output = T>>(vector: Vec2<T>) -> Vec2<T> {
    Vec2::new(-vector.y, vector.x)
}

fn aligned(points: [Point; 3]) -> bool {
    let [a, b, c] = points;
    let m = (b.y - a.y) * (c.x - a.x);
    let n = (c.y - a.y) * (b.x - a.x);
    (m - n).abs() < 0.01
}
