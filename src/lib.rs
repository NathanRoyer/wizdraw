#![doc = include_str!("../README.md")]

#![no_std]
#![cfg_attr(feature = "simd", feature(portable_simd))]

extern crate alloc;

use alloc::vec::Vec;
use vek::bezier::CubicBezier2;
use vek::vec::Vec2;

#[cfg_attr(feature = "simd", path = "simd.rs")]
#[cfg_attr(not(feature = "simd"), path = "sequential.rs")]
mod implementation;

#[doc(inline)]
pub use implementation::*;

#[cfg(not(feature = "f32"))]
pub type Element = f32;

#[cfg(feature = "f64")]
pub type Element = f64;

fn find_longest_segment<const D: usize>(
    curve: &CubicBezier2<Element>,
    t: &mut Element,
    max_diff: Element,
    end_p: &mut Vec2<Element>,
) -> bool {
    let max_diff_sq = max_diff * max_diff;
    let div: Element = (D + 1) as Element;

    let start = *t;
    let mut trial = 1.0 - start;
    *t = start + trial;

    let start_p = curve.evaluate(start);
    let mut to_end = true;
    'outer: loop {
        if trial < Element::EPSILON {
            // shouldn't happen
            *t = 1.0 - start;
            to_end = true;
        }
        *end_p = curve.evaluate(*t);
        let p_unit = (*end_p - start_p) / div;
        let t_unit = trial / div;
        let mut p_tmp = p_unit;
        let mut t_tmp = t_unit;
        for _ in 0..D {
            let a = start_p + p_tmp;
            let b = curve.evaluate(start + t_tmp);
            if a.distance_squared(b) > max_diff_sq {
                trial /= 2.0;
                *t = start + trial;
                to_end = false;
                continue 'outer;
            }
            p_tmp = p_tmp + p_unit;
            t_tmp = t_tmp + t_unit;
        }
        // success
        break;
    }
    to_end
}

/// Transforms a cubic bezier curve to linear
/// segments and appends them to a vector
///
/// `max_diff` tells the maximum distance
/// between the approximated segments and
/// the original curve. Try values in `1.0..0.1`.
///
/// the const generic corresponds to how many
/// test points will be placed along segments;
/// these are used to tell the distance between
/// the segment and the curve. try values from
/// 3 to 10.
pub fn push_cubic_bezier_segments<const D: usize>(
    curve: &CubicBezier2<Element>,
    max_diff: Element,
    points: &mut Vec<Vec2<Element>>,
) {
    points.push(curve.start);
    let mut t = 0.0;
    let mut end_p = Vec2::zero();
    let mut to_end = false;
    while !to_end {
        to_end = find_longest_segment::<D>(curve, &mut t, max_diff, &mut end_p);
        points.push(end_p);
    }
}
