#![no_std]
extern crate alloc;
use alloc::vec::Vec;

use vek::bezier::repr_c::CubicBezier2;
use vek::vec::repr_c::vec2::Vec2;
use vek::geom::repr_c::Aabr;
use vek::geom::repr_c::LineSegment2;
use vek::approx::RelativeEq;
use num_traits::real::Real;
use num_traits::cast::NumCast;

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
            // bug :thinking:
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
/// the original curve. `1.0` is a good value.
///
/// the const generic corresponds to how many
/// test points will be placed along segments;
/// these are used to tell the distance between
/// the segment and the curve. try values from
/// 3 to 10.
pub fn simplify<T: Real + DivByTwo, const D: usize>(
    curve: &CubicBezier2<T>,
    max_diff: T,
    segments: &mut Vec<Vec2<T>>,
) {
    segments.push(curve.start);
    let mut t = T::zero();
    let mut end_p = Vec2::zero();
    let mut to_end = false;
    while !to_end {
        to_end = find_longest_segment::<_, D>(curve, &mut t, max_diff, &mut end_p);
        segments.push(end_p);
    }
}

fn determinant<T: Real>(a: &Vec2<T>, b: &Vec2<T>) -> T {
    a.x * b.y - a.y * b.x
}

fn aabr<T: Real>(start: Vec2<T>, end: Vec2<T>) -> Aabr<T> {
    Aabr::<T> {
        min: Vec2::partial_min(start, end),
        max: Vec2::partial_max(start, end),
    }
}

fn is_inside<T: Real>(p: Vec2<T>, path: &[Vec2<T>]) -> bool {
    let mut winding_number = 0isize;
    for segment in path.windows(2) {
        let (s, e) = (segment[0], segment[1]);
        /*
        let dsp = s.distance_squared(p);
        let dep = e.distance_squared(p);
        if dsp < T::epsilon() || dep < T::epsilon() {
            return true;
        }
        */
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

/// Renders a path to a byte mask, either by
/// stroking or filling it.
///
/// The mask must have one byte per pixel.
/// The resulting bytes are closer to 255 if
/// the corresponding pixel is in the path, or
/// closer to 0 otherwise. It can be used as
/// an opacity byte, when blitting pixels.
///
/// Setting `None` to the `stroke` parameter
/// will result in a "Fill" operation, while
/// giving it an existing value specifies a
/// stroke width, and will result in a "Stroke"
/// operation.
///
/// Note: First and last path points must be equal!
pub fn rasterize<T: Real + RelativeEq>(
    path: &[Vec2<T>],
    mask: &mut [u8],
    mask_size: Vec2<usize>,
    stroke: Option<T>,
) {
    let two = T::from(2).unwrap();
    let half = T::one() / two;
    let (half_stroke_width, offset) = match stroke {
        Some(w) => (w / two, w.to_isize().unwrap() + 2),
        None => (two, 2),
    };
    let w = mask_size.x as isize;
    let h = mask_size.y as isize;
    for segment in path.windows(2) {
        let (start, end) = (segment[0], segment[1]);
        let seg = LineSegment2 {
            start,
            end,
        };
        let aabr = aabr(start, end);
        let min_x_px = aabr.min.x.to_isize().unwrap() - offset;
        let min_y_px = aabr.min.y.to_isize().unwrap() - offset;
        let max_x_px = aabr.max.x.to_isize().unwrap() + offset;
        let max_y_px = aabr.max.y.to_isize().unwrap() + offset;
        'outer: for y_px in min_y_px..max_y_px {
            if !(0..h).contains(&y_px) {
                continue;
            }
            let cov_line = &mut mask[(y_px as usize) * mask_size.x..][..mask_size.x];
            let mut past = false;
            for x_px in min_x_px..max_x_px {
                if (0..w).contains(&x_px) {
                    let p = Vec2::<T> {
                        x: T::from(x_px).unwrap(),
                        y: T::from(y_px).unwrap(),
                    };
                    if seg.distance_to_point(p) <= half_stroke_width {
                        let x = x_px as usize;
                        cov_line[x] = 255;
                        past = true;
                    } else if past {
                        continue 'outer;
                    }
                }
            }
        }
    }
    if stroke.is_none() {
        let fp = Vec2::from((half, half));
        let mut prev_x0 = match is_inside(fp, path) {
            true => 255,
            false => 0,
        };
        for y in 0..mask_size.y {
            let mut prev = 0;
            let i = y * mask_size.x;
            for x in 0..mask_size.x {
                if mask[i + x] != 0 {
                    let p = Vec2::<T> {
                        x: T::from(x).unwrap() + half,
                        y: T::from(y).unwrap() + half,
                    };
                    prev = match is_inside(p, path) {
                        true => 255,
                        false => 0,
                    };
                } else if x == 0 {
                    prev = prev_x0;
                }
                mask[i + x] = prev;
                if x == 0 {
                    prev_x0 = prev;
                }
            }
        }
    }
}

/// Trait used internally to speed up [`simplify`].
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
