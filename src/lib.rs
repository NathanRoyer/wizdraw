#![doc = include_str!("../README.md")]

#![no_std]
#![cfg_attr(feature = "simd", feature(portable_simd))]

extern crate alloc;

#[cfg_attr(feature = "simd", path = "simd.rs")]
#[cfg_attr(not(feature = "simd"), path = "sequential.rs")]
mod implementation;

#[doc(inline)]
pub use implementation::*;

#[cfg(not(feature = "f32"))]
pub type Element = f32;

#[cfg(feature = "f64")]
pub type Element = f64;
