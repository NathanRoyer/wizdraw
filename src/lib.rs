#![doc = include_str!("../README.md")]

#![no_std]
extern crate alloc;
use alloc::vec::Vec;

use vek::bezier::CubicBezier2;
use vek::vec::Vec2;
use vek::geom::Aabr;
use vek::geom::LineSegment2;
use vek::approx::RelativeEq;
use core::ops::Add;
use num_traits::real::Real;
use num_traits::cast::NumCast;
use num_traits::{Zero, One};

fn find_longest_segment<T: Real + DivByTwo, const D: usize>(
    curve: &CubicBezier2<T>,
    t: &mut T,
    max_diff: T,
    end_p: &mut Vec2<T>,
) -> bool {
    let max_diff_sq = max_diff * max_diff;
    let div: T = <T as NumCast>::from(D + 1).unwrap();

    let start = *t;
    let mut trial = T::one() - start;
    *t = start + trial;

    let start_p = curve.evaluate(start);
    let mut to_end = true;
    'outer: loop {
        if trial < T::epsilon() {
            // shouldn't happen
            *t = T::one() - start;
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
                trial.div_by_two();
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
pub fn push_cubic_bezier_segments<T: Real + DivByTwo, const D: usize>(
    curve: &CubicBezier2<T>,
    max_diff: T,
    points: &mut Vec<Vec2<T>>,
) {
    points.push(curve.start);
    let mut t = T::zero();
    let mut end_p = Vec2::zero();
    let mut to_end = false;
    while !to_end {
        to_end = find_longest_segment::<_, D>(curve, &mut t, max_diff, &mut end_p);
        points.push(end_p);
    }
}

#[inline(always)]
fn aabr<T: Real>(start: Vec2<T>, end: Vec2<T>, offset: isize) -> [isize; 4] {
    let aabr = Aabr::<T> {
        min: Vec2::partial_min(start, end),
        max: Vec2::partial_max(start, end),
    };

    [
        aabr.min.x.to_isize().unwrap() - offset,
        aabr.min.y.to_isize().unwrap() - offset,
        aabr.max.x.to_isize().unwrap() + offset,
        aabr.max.y.to_isize().unwrap() + offset,
    ]
}

#[inline(always)]
fn determinant<T: Real>(a: &Vec2<T>, b: &Vec2<T>) -> T {
    a.x * b.y - a.y * b.x
}

#[inline(always)]
fn is_inside<T: Real>(p: Vec2<T>, path: &[Vec2<T>]) -> bool {
    let mut winding_number = 0isize;

    for segment in path.windows(2) {
        let (s, e) = (segment[0], segment[1]);

        let v1 = p - s;
        let v2 = e - s;
        let d = determinant(&v1, &v2);
        if s.y <= p.y {
            if e.y > p.y && d > T::epsilon() {
                winding_number -= 1;
            }
        } else {
            if e.y <= p.y && d < T::epsilon() {
                winding_number += 1;
            }
        }
    }

    winding_number != 0
}

// directly copied from vek's source; only added #[inline(always)]
#[inline(always)]
fn projected_point<T>(this: LineSegment2<T>, p: Vec2<T>) -> Vec2<T> where T: Real + Add<T, Output=T> + RelativeEq {
    let len_sq = this.start.distance_squared(this.end);

    if len_sq.relative_eq(&Zero::zero(), T::default_epsilon(), T::default_max_relative()) {
        this.start
    } else {
        let t = ((p - this.start).dot(this.end - this.start) / len_sq)
            .max(Zero::zero())
            .min(One::one());
        this.start + (this.end - this.start) * t
    }
}

fn ssaa_point<T: Real, const SSAA: usize>(x: isize, y: isize, sx: usize, sy: usize) -> Vec2<T> {
    let sub_px_width = T::one() / T::from(SSAA).unwrap();
    let half_sub_px_width = sub_px_width / T::from(2).unwrap();

    let x_offset = sub_px_width * T::from(sx).unwrap() + half_sub_px_width;
    let y_offset = sub_px_width * T::from(sy).unwrap() + half_sub_px_width;

    Vec2 {
        x: T::from(x).unwrap() + x_offset,
        y: T::from(y).unwrap() + y_offset,
    }
}

#[inline(never)]
fn ssaa_average<T: Real, F: Fn(Vec2<T>) -> bool, const SSAA: usize>
    (x: isize, y: isize, condition: F) -> u8
{
    let mut in_count = 0;

    for sx in 0..SSAA {
        for sy in 0..SSAA {
            if condition(ssaa_point::<_, SSAA>(x, y, sx, sy)) {
                in_count += 1;
            }
        }
    }

    ((255 * in_count) / (SSAA * SSAA)) as u8
}

#[inline(always)]
fn sub_segments<T: Real + RelativeEq>(
    start: Vec2<T>,
    end: Vec2<T>,
    process_sub_segment: &mut impl FnMut(Vec2<T>, Vec2<T>),
    sub_segment_len: usize,
) {
    let length = start.distance(end);
    if length > T::epsilon() {
        let sub_segments = length / T::from(sub_segment_len).unwrap();

        let mut last = start;
        let mut next;
        let unit = (end - start) / sub_segments;

        let integer_multiples = sub_segments.trunc().to_usize().unwrap();
        for _ in 0..integer_multiples {
            next = last + unit;
            process_sub_segment(last, next);
            last = next;
        }

        next = last + unit * sub_segments.fract();
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
pub fn stroke<T: Real + RelativeEq, const SSAA: usize>(
    path: &[Vec2<T>],
    mask: &mut [u8],
    mask_size: Vec2<usize>,
    line_width: T,
) {
    mask.fill(0);
    let half_stroke_width = line_width / T::from(2).unwrap();
    let half_stroke_width_sq = half_stroke_width.powi(2);

    let offset = line_width.to_isize().unwrap() + 2;

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
                    let avg = ssaa_average::<_, _, SSAA>(x_px, y_px, is_close_enough);

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

/// Fills a path to a byte mask
///
/// The mask must have one byte per pixel.
/// The resulting bytes are closer to 255 if
/// the corresponding pixel is in the path, or
/// closer to 0 otherwise. It can be used as
/// an opacity byte, when blitting pixels.
///
/// You can specify a value for super-sample
/// anti-aliaising via `SSAA`. I suggest
/// setting it to a value between 2 and 4.
///
/// Note: First and last path points must be equal!
pub fn fill<T: Real + RelativeEq, const SSAA: usize>(
    path: &[Vec2<T>],
    mask: &mut [u8],
    mask_size: Vec2<usize>,
) {
    mask.fill(0);
    let w = mask_size.x as isize;
    let h = mask_size.y as isize;

    let mut process_sub_segment = |start, end| {
        let [min_x_px, min_y_px, max_x_px, max_y_px] = aabr(start, end, 2);

        for y_px in min_y_px..max_y_px {
            if !(0..h).contains(&y_px) {
                continue;
            }

            let cov_line = &mut mask[(y_px as usize) * mask_size.x..][..mask_size.x];

            for x_px in min_x_px..max_x_px {
                if (0..w).contains(&x_px) {
                    let x = x_px as usize;
                    if cov_line[x] == 0 {
                        cov_line[x] = 255;
                    }
                }
            }
        }
    };

    for segment in path.windows(2) {
        sub_segments(segment[0], segment[1], &mut process_sub_segment, 5);
    }

    let is_inside_path = |p| is_inside(p, path);

    let mut line_start = 0;
    for y in 0..mask_size.y {
        let mut go_back = 0;
        for x in 0..mask_size.x {
            let not_last_point = x != (mask_size.x - 1);
            if mask[line_start + x] == 0 && not_last_point {
                go_back += 1;
            } else {
                let avg = ssaa_average::<_, _, SSAA>(x as isize, y as isize, is_inside_path);
                mask[line_start + x] = avg;

                for i in 1..=go_back {
                    mask[line_start + x - i] = avg;
                }

                go_back = 0;
            }
        }

        line_start += mask_size.x;
    }
}

/// Trait used internally to speed up [`push_cubic_bezier_segments`].
pub trait DivByTwo {
    fn div_by_two(&mut self);
}

impl DivByTwo for f32 {
    fn div_by_two(&mut self) {
        *self /= 2.0;
    }
}

impl DivByTwo for f64 {
    fn div_by_two(&mut self) {
        *self /= 2.0;
    }
}
