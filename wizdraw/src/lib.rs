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

pub use path::Path;
mod path;

mod stroke;

pub use primitives::{Arc, Segment};
pub mod primitives;

pub use shapes::{Polygon, Rectangle, Transformer, Combiner};
pub mod shapes;

pub use texture::{Texture, BitmapHandle, MaskHandle};
pub mod texture;

/// Implementations of [`Canvas`] using only the CPU
pub mod cpu;
pub use cpu::inside::shape_contains;

/// Implementations of [`Canvas`] using a GPU
#[cfg(feature = "gles2")]
pub mod gles2;

/// Text/Font/Glyph support
#[cfg(feature = "text")]
pub mod text;

const TRANSPARENT: Color = Color::new(0, 0, 0, 0);

/// A 4-byte color (RGBA)
pub type Color = rgb::RGBA<u8>;

/// Pixel or Subpixel coordinates
pub type Point = Vec2<f32>;

type Point64 = Vec2<f64>;

pub trait PathApi {
    fn segment(&mut self, segment: Segment);
    fn arc(&mut self, arc: Arc);
}

pub trait Shape {
    fn draw(&self, api: &mut dyn PathApi);

    fn bounding_box(&self) -> BoundingBox {
        let mut bb = BoundingBox::default();
        self.draw(&mut bb);
        bb
    }

    fn contains(&self, point: Point) -> bool where Self: Sized {
        shape_contains(point, self)
    }

    fn transform(&self, scale: f32, translation: Vec2<f32>) -> Transformer<&Self> where Self: Sized {
        Transformer {
            shape: self,
            scale,
            translation,
        }
    }

    fn combine<B: Shape + Sized>(&self, b: B) -> Combiner<&Self, B> where Self: Sized {
        Combiner {
            a: self,
            b,
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

    fn alloc_mask(&mut self, width: usize, height: usize) -> MaskHandle;
    fn fill_mask(&mut self, bitmap: MaskHandle, x: usize, y: usize, w: usize, h: usize, buf: &[u8]);
    fn free_mask(&mut self, bitmap: MaskHandle);

    fn clear(&mut self);
    fn fill(&mut self, shape: &dyn Shape, bb: Option<BoundingBox>, texture: &Texture, ssaa: SsaaConfig);

    fn stroke(&mut self, shape: &dyn Shape, bb: Option<BoundingBox>, width: f32, texture: &Texture, ssaa: SsaaConfig) {
        stroke::stroke(self, shape, bb, width, texture, ssaa);
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
    pub fn new(min: Point, max: Point) -> Self {
        Self { min, max }
    }

    pub fn adopt(&mut self, point: Point) {
        if point.x < self.min.x { self.min.x = point.x; }
        if point.x > self.max.x { self.max.x = point.x; }
        if point.y < self.min.y { self.min.y = point.y; }
        if point.y > self.max.y { self.max.y = point.y; }
    }

    pub fn restrict(&mut self, other: &Self) {
        if self.min.x < other.min.x { self.min.x = other.min.x; }
        if self.min.y < other.min.y { self.min.y = other.min.y; }
        if self.max.x > other.max.x { self.max.x = other.max.x; }
        if self.max.y > other.max.y { self.max.y = other.max.y; }
    }

    pub fn to_rect(&self) -> Rectangle {
        Rectangle::new([self.min, self.max])
    }

    pub fn contains(&self, point: Point) -> bool {
        (point.x >= self.min.x) && (point.x <= self.max.x) && (point.y >= self.min.y) && (point.y <= self.max.y)
    }

    pub fn is_empty(&self) -> bool {
        (self.max.x <= self.min.x) || (self.max.y <= self.min.y)
    }
}

impl PathApi for BoundingBox {
    fn segment(&mut self, segment: Segment) {
        self.adopt(segment.a);
        self.adopt(segment.b);
    }

    fn arc(&mut self, arc: Arc) {
        let rows = arc.rows();
        let cols = arc.cols();

        let min = Point::new(*cols.start(), *rows.start());
        let max = Point::new(*cols.end(), *rows.end());
        self.adopt(min);
        self.adopt(max);
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
