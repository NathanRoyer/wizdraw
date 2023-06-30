#![doc = include_str!("../README.md")]

#![no_std]
#![cfg_attr(feature = "simd", feature(portable_simd))]

extern crate alloc;

use alloc::vec::Vec;
use vek::bezier::CubicBezier2;
use vek::vec::Vec2;
use vek::geom::Aabr;
use vek::geom::LineSegment2;

#[cfg_attr(feature = "simd", path = "simd.rs")]
#[cfg_attr(not(feature = "simd"), path = "sequential.rs")]
mod implementation;

#[doc(inline)]
pub use implementation::*;

#[cfg(not(feature = "f64"))]
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

#[inline(always)]
fn aabr(start: Vec2<Element>, end: Vec2<Element>, offset: isize) -> [isize; 4] {
    let aabr = Aabr::<Element> {
        min: Vec2::partial_min(start, end),
        max: Vec2::partial_max(start, end),
    };

    [
        (aabr.min.x as isize) - offset,
        (aabr.min.y as isize) - offset,
        (aabr.max.x as isize) + offset,
        (aabr.max.y as isize) + offset,
    ]
}

// modification of vek's projected_point
#[inline(always)]
fn projected_point(this: LineSegment2<Element>, p: Vec2<Element>) -> Vec2<Element> {
    let len_sq = this.start.distance_squared(this.end);

    if len_sq < Element::EPSILON {
        this.start
    } else {
        let t = ((p - this.start).dot(this.end - this.start) / len_sq).clamp(0.0, 1.0);
        this.start + (this.end - this.start) * t
    }
}

fn ssaa_point<const SSAA: usize>(x: isize, y: isize, sx: usize, sy: usize) -> Vec2<Element> {
    let sub_px_width = (SSAA as Element).recip();
    let half_sub_px_width = sub_px_width / 2.0;

    let mut x = (x as Element) + half_sub_px_width;
    let mut y = (y as Element) + half_sub_px_width;

    for _ in 0..sx { x += sub_px_width; }
    for _ in 0..sy { y += sub_px_width; }

    Vec2 {
        x,
        y,
    }
}

#[inline(never)]
fn ssaa_average<F: Fn(Vec2<Element>) -> bool, const SSAA: usize>
    (x: isize, y: isize, condition: F) -> u8
{
    let mut in_count = 0;

    for sx in 0..SSAA {
        for sy in 0..SSAA {
            if condition(ssaa_point::<SSAA>(x, y, sx, sy)) {
                in_count += 1;
            }
        }
    }

    ((255 * in_count) / (SSAA * SSAA)) as u8
}

#[inline(always)]
fn sub_segments(
    start: Vec2<Element>,
    end: Vec2<Element>,
    process_sub_segment: &mut impl FnMut(Vec2<Element>, Vec2<Element>),
    sub_segment_len: usize,
) {
    let length = start.distance(end);
    if length > Element::EPSILON {
        let sub_segments = length / (sub_segment_len as Element);

        let mut last = start;
        let mut next;
        let unit = (end - start) / sub_segments;

        let integer_multiples = {
            // this uses LLVM's fptoui function, which truncates the fp number
            sub_segments as usize
        };
        for _ in 0..integer_multiples {
            next = last + unit;
            process_sub_segment(last, next);
            last = next;
        }

        let fract = {
            // get fractional part
            // 15.689 - 15.000 => 0.689
            sub_segments - (integer_multiples as f32)
        };
        next = last + unit * fract;
        process_sub_segment(last, next);
    }
}

/// Strokes a path to a byte mask
///
/// The mask must have one byte per pixel.
/// The resulting bytes are closer to 255 if
/// the corresponding pixel is on the line, or
/// closer to 0 otherwise. It can be used as
/// an opacity byte, when blitting pixels.
///
/// You can specify a value for super-sample
/// anti-aliaising via `SSAA`. I suggest
/// setting it to a value between 2 and 4.
///
/// Note: First and last path points must be equal!
pub fn stroke<const SSAA: usize>(
    path: &[Vec2<Element>],
    mask: &mut [u8],
    mask_size: Vec2<usize>,
    line_width: Element,
) {
    mask.fill(0);
    let half_stroke_width = line_width / 2.0;
    let half_stroke_width_sq = half_stroke_width * half_stroke_width;

    let offset = (line_width as isize) + 2;

    let w = mask_size.x as isize;
    let h = mask_size.y as isize;

    let mut process_sub_segment = |start, end| {
        let segment = LineSegment2 {
            start,
            end,
        };

        let is_close_enough = |p| {
            // this is the hot spot
            let projected = projected_point(segment, p);
            let distance_sq = projected.distance_squared(p);
            distance_sq <= half_stroke_width_sq
        };

        let [min_x_px, min_y_px, max_x_px, max_y_px] = aabr(start, end, offset);

        for y_px in min_y_px..max_y_px {
            if !(0..h).contains(&y_px) {
                continue;
            }

            let cov_line = &mut mask[(y_px as usize) * mask_size.x..][..mask_size.x];

            let mut past = false;
            for x_px in min_x_px..max_x_px {
                let x = x_px as usize;
                if (0..w).contains(&x_px) {
                    let avg = ssaa_average::<_, SSAA>(x_px, y_px, is_close_enough);

                    if cov_line[x] < avg {
                        cov_line[x] = avg;
                        past = true;
                    }

                    if avg == 0 && past {
                        break;
                    }
                } else {
                    break;
                }
            }
        }
    };

    for segment in path.windows(2) {
        sub_segments(segment[0], segment[1], &mut process_sub_segment, 5);
    }
}
