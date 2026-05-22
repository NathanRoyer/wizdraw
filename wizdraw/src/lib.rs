#![doc = include_str!("../README.md")]

// #![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(feature = "simd", feature(portable_simd))]

extern crate alloc;
pub extern crate rgb;
pub extern crate vek;

#[cfg(feature = "std")]
extern crate std;

use core::ops::RangeInclusive;
use vek::vec::Vec2;

#[allow(unused_imports)]
use vek::num_traits::Float;

pub use ssaa::SsaaConfig;
mod ssaa;

pub use path::{Path, Transformer, Striker};
mod path;

// #[cfg(any(doc, feature = "shapes"))]
// pub mod shapes;

/// Implementations of [`Canvas`] using only the CPU
pub mod cpu;

/// Implementations of [`Canvas`] using a GPU
#[cfg(feature = "gles2")]
pub mod gles2;

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

#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Arc {
    pub center: Point,
    pub radius: f32,
    pub start_angle: f32,
    pub stop_angle: f32,
    pub clockwise: bool,
}

impl Arc {
    pub fn new(center: Point, radius: f32, start_angle: f32, stop_angle: f32, clockwise: bool) -> Self {
        Self { center, radius, start_angle, stop_angle, clockwise }
    }

    #[inline(always)]
    pub fn data(&self) -> ArcData {
        let sorted = self.start_angle < self.stop_angle;
        let wraps = self.clockwise == sorted;

        let (min_angle, max_angle) = match sorted {
            true => (self.start_angle, self.stop_angle),
            false => (self.stop_angle, self.start_angle),
        };

        ArcData {
            min_angle,
            max_angle,
            min_angle_cos: f32::cos(min_angle),
            max_angle_cos: f32::cos(max_angle),
            wraps,
        }
    }

    pub fn rows(&self, data: &ArcData) -> RangeInclusive<f32> {
        use core::f32::consts::FRAC_PI_2;

        #[inline(always)]
        fn fast_sin(x: f32) -> f32 {
            fast_cos(x - FRAC_PI_2)
        }

        let top = data.covers_angle(FRAC_PI_2);
        let btm = data.covers_angle(-FRAC_PI_2);

        let top_y = self.center.y - self.radius;
        let btm_y = self.center.y + self.radius;

        if top & btm {
            return top_y..=btm_y;
        }

        let neg_radius = -self.radius;
        let start_y = self.center.y + neg_radius * fast_sin(self.start_angle);
        let stop_y = self.center.y + neg_radius * fast_sin(self.stop_angle);

        // adding a bias for safety regarding sin approximation
        let bias = 2.0;
        let (mut min_y, mut max_y) = match start_y <= stop_y {
            true => (start_y - bias, stop_y + bias),
            false => (stop_y - bias, start_y + bias),
        };

        if top { min_y = top_y; }
        if btm { max_y = btm_y; }

        min_y..=max_y
    }

    pub fn cols(&self, data: &ArcData) -> RangeInclusive<f32> {
        let r = data.covers_angle(0.0);
        let l = data.wraps;

        let l_x = self.center.x - self.radius;
        let r_x = self.center.x + self.radius;

        if l & r {
            return l_x..=r_x;
        }

        let start_x = self.center.x + self.radius * fast_cos(self.start_angle);
        let stop_x = self.center.x + self.radius * fast_cos(self.stop_angle);

        // adding a bias for safety regarding cos approximation
        let bias = 2.0;
        let (mut min_x, mut max_x) = match start_x <= stop_x {
            true => (start_x - bias, stop_x + bias),
            false => (stop_x - bias, start_x + bias),
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

    /// Caution: path junctions must be aligned!
    fn strike(&self, width: f32) -> Striker<&Self> where Self: Sized {
        Striker {
            shape: self,
            width,
        }
    }
}

impl<T: Shape> Shape for &T {
    fn draw(&self, api: &mut dyn PathApi) {
        T::draw(self, api)
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
pub struct ArcData {
    pub min_angle: f32,
    pub max_angle: f32,
    pub min_angle_cos: f32,
    pub max_angle_cos: f32,
    /// covered angles span across +/-PI
    pub wraps: bool,
}

impl ArcData {
    #[inline(always)]
    pub fn covers_angle(&self, angle: f32) -> bool {
        (self.min_angle..=self.max_angle).contains(&angle) ^ self.wraps
    }
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
        let data = arc.data();
        let rows = arc.rows(&data);
        let cols = arc.cols(&data);

        let min = Point::new(*cols.start(), *rows.start());
        let max = Point::new(*cols.end(), *rows.end());
        self.check(min);
        self.check(max);
    }
}

#[inline(always)]
fn fast_cos(mut x: f32) -> f32 {
    use core::f32::consts::FRAC_1_PI;

    x *= FRAC_1_PI / 2.0;
    x -= 0.25 + (x + 0.25).floor();
    x *= 16.0 * (x.abs() - 0.5);
    x += 0.225 * x * (x.abs() - 1.0);
    x
}
