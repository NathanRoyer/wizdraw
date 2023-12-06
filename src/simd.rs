use super::*;

use core::simd::{Mask, Simd, SimdInt, SimdFloat, SimdPartialEq, SimdPartialOrd};
use core::simd::{LaneCount as Lc, SupportedLaneCount as Slc};
use vek::vec::Vec2;

// L = SIMD lanes

type SimdF32<const L: usize> = Simd<f32, L>;
type SimdU32<const L: usize> = Simd<u32, L>;
type SimdI32<const L: usize> = Simd<i32, L>;
type SimdBool<const L: usize> = Mask<i32, L>;
type SimdPoint<const L: usize> = Vec2<SimdF32<L>>;

#[derive(Copy, Clone, Debug)]
pub struct SimdCubicBezier<const L: usize> where Lc<L>: Slc {
    c1: SimdPoint<L>,
    c2: SimdPoint<L>,
    c3: SimdPoint<L>,
    c4: SimdPoint<L>,
}

// |num| 1.0 / num.sqrt()
// #[inline(always)]
fn fast_inv_sqrt<const L: usize>(num: SimdF32<L>) -> SimdF32<L> where Lc<L>: Slc {
    let simd_inv_sqrt = simd_u32(0x5f37_5a86);

    SimdF32::from_bits(simd_inv_sqrt - (num.to_bits() >> simd_u32(1)))
}

// #[inline(always)]
fn is_curve_straight<const L: usize>(curve: SimdCubicBezier<L>) -> SimdBool<L> where Lc<L>: Slc {
    let simd_straight_threshold_x2 = simd_f32(STRAIGHT_THRESHOLD * 2.0);

    let distance = |p: SimdPoint<L>| -> SimdF32<L> {
        // https://en.wikipedia.org/wiki/Distance_from_a_point_to_a_line#Line_defined_by_two_points

        let l = curve.c4 - curve.c1;
        let a = l.x * (curve.c1.y - p.y);
        let b = l.y * (curve.c1.x - p.x);

        // distance from p to projected point
        (a - b).abs() * fast_inv_sqrt(l.x * l.x + l.y * l.y)
    };

    (distance(curve.c2) + distance(curve.c3)).simd_lt(simd_straight_threshold_x2).into()
}

// Computes a winding number addition based on [s -> e] segment
// #[inline(always)]
fn use_segment_for_pip<const L: usize>(
    p: SimdPoint<L>,
    s: SimdPoint<L>,
    e: SimdPoint<L>,
) -> SimdI32<L> where Lc<L>: Slc {
    let simd_epsilon = simd_f32(f32::EPSILON);

    let v1 = p - s;
    let v2 = e - s;
    let d = v1.x * v2.y - v1.y * v2.x;

    let cond_a = s.y.simd_le(p.y);
    let cond_b = e.y.simd_gt(p.y);
    let cond_c = d.simd_gt(simd_epsilon);

    let dec_mask = ( cond_a) & ( cond_b) & ( cond_c);
    let inc_mask = (!cond_a) & (!cond_b) & (!cond_c);

    // to_int gives -1 for true
    dec_mask.to_int() - inc_mask.to_int()
}

pub fn subpixel_opacity<const L: usize>(
    pixel: SimdPoint<L>,
    path: &[CubicBezier],
    step_inc: f32,
    holes: bool,
) -> f32 where Lc<L>: Slc {
    let path_len = simd_u32(path.len() as u32);
    let simd_f1 = simd_f32(1.0);
    let simd_05 = simd_f32(0.5);
    let simd_i0 = simd_i32(0);

    let mut curve_index = simd_u32(0);
    let mut winding_number = simd_i0;
    let mut trial = simd_f1;
    let mut rem_sc = SimdCubicBezier::init(path.first().cloned().unwrap_or_default());

    loop {
        let valid: SimdBool<L> = curve_index.simd_lt(path_len).into();

        if !valid.any() {
            break;
        }

        let (trial_sc, future_sc) = rem_sc.split(trial);
        let use_as_is = (!trial_sc.is_p_in_aabb(pixel)) | is_curve_straight(trial_sc);

        if use_as_is.any() {
            let winding_number_inc = use_segment_for_pip(pixel, trial_sc.c1, trial_sc.c4);
            let end_of_curve = trial.simd_eq(simd_f1);

            let inc_curve_index = use_as_is & end_of_curve;
            curve_index += (-(inc_curve_index).to_int()).cast();

            winding_number += (use_as_is & valid).select(winding_number_inc, simd_i0);

            let mut advance_rem_sc = use_as_is;
            for i in 0..L {
                let ci = curve_index[i] as usize;
                if inc_curve_index.test(i) && ci < path.len() {
                    let pc = path[ci];
                    rem_sc.c1.x[i] = pc.c1.x;
                    rem_sc.c1.y[i] = pc.c1.y;
                    rem_sc.c2.x[i] = pc.c2.x;
                    rem_sc.c2.y[i] = pc.c2.y;
                    rem_sc.c3.x[i] = pc.c3.x;
                    rem_sc.c3.y[i] = pc.c3.y;
                    rem_sc.c4.x[i] = pc.c4.x;
                    rem_sc.c4.y[i] = pc.c4.y;
                    advance_rem_sc.set(i, false);
                }
            }

            rem_sc.c1.x = advance_rem_sc.select(future_sc.c1.x, rem_sc.c1.x);
            rem_sc.c1.y = advance_rem_sc.select(future_sc.c1.y, rem_sc.c1.y);
            rem_sc.c2.x = advance_rem_sc.select(future_sc.c2.x, rem_sc.c2.x);
            rem_sc.c2.y = advance_rem_sc.select(future_sc.c2.y, rem_sc.c2.y);
            rem_sc.c3.x = advance_rem_sc.select(future_sc.c3.x, rem_sc.c3.x);
            rem_sc.c3.y = advance_rem_sc.select(future_sc.c3.y, rem_sc.c3.y);
            rem_sc.c4.x = advance_rem_sc.select(future_sc.c4.x, rem_sc.c4.x);
            rem_sc.c4.y = advance_rem_sc.select(future_sc.c4.y, rem_sc.c4.y);
        }

        trial = match use_as_is.all() {
            true => simd_f1,
            false => use_as_is.select(simd_f1, trial * simd_05),
        };

    }

    let mut res = 0.0;

    for w in winding_number.as_array() {
        let num = match holes {
            true => *w % 2,
            false => *w,
        };

        if num != 0 {
            res += step_inc;
        }
    }

    res
}

pub fn pixel_opacity<const P: usize>(p: Point, path: &[CubicBezier], holes: bool) -> u8 {
    let simd_lanes_to_use = P.min(MAX_SIMD_LANES);
    let mut res = 0.0;

    let steps = P / simd_lanes_to_use;
    let step_inc = 255.0 / (P as f32);
    let mut spm_offset = 0;

    for _ in 0..steps {
        res += match simd_lanes_to_use {
            1  => subpixel_opacity::< 1>(simd_p(p) + simd_spm::<P,  1>(spm_offset) * simd_f32(2.0), path, step_inc, holes),
            2  => subpixel_opacity::< 2>(simd_p(p) + simd_spm::<P,  2>(spm_offset) * simd_f32(2.0), path, step_inc, holes),
            4  => subpixel_opacity::< 4>(simd_p(p) + simd_spm::<P,  4>(spm_offset) * simd_f32(2.0), path, step_inc, holes),
            8  => subpixel_opacity::< 8>(simd_p(p) + simd_spm::<P,  8>(spm_offset) * simd_f32(2.0), path, step_inc, holes),
            16 => subpixel_opacity::<16>(simd_p(p) + simd_spm::<P, 16>(spm_offset) * simd_f32(2.0), path, step_inc, holes),
            _ => panic!("unsupported SIMD configuration"),
        };

        spm_offset += simd_lanes_to_use;
    }

    res as u8
}

impl<const L: usize> SimdCubicBezier<L> where Lc<L>: Slc {
    fn init(curve: CubicBezier) -> Self {
        Self {
            c1: SimdPoint::new(simd_f32(curve.c1.x), simd_f32(curve.c1.y)),
            c2: SimdPoint::new(simd_f32(curve.c2.x), simd_f32(curve.c2.y)),
            c3: SimdPoint::new(simd_f32(curve.c3.x), simd_f32(curve.c3.y)),
            c4: SimdPoint::new(simd_f32(curve.c4.x), simd_f32(curve.c4.y)),
        }
    }

    fn split(
        &self,
        t: SimdF32<L>,
    ) -> (Self, Self) {

        // #[inline(always)]
        fn travel<const L: usize>(
            a: SimdPoint<L>,
            b: SimdPoint<L>,
            t: SimdF32<L>,
        ) -> SimdPoint<L> where Lc<L>: Slc {
            SimdPoint {
                x: a.x + (b.x - a.x) * t,
                y: a.y + (b.y - a.y) * t,
            }
        }

        // step 1: take 2nd half of self

        let side1 = travel(self.c1, self.c2, t);
        let side2 = travel(self.c2, self.c3, t);
        let side3 = travel(self.c3, self.c4, t);

        let diag1 = travel(side1, side2, t);
        let diag2 = travel(side2, side3, t);

        let split_point = travel(diag1, diag2, t);

        let first_part = Self {
            c1: self.c1,
            c2: side1,
            c3: diag1,
            c4: split_point,
        };

        let second_part = Self {
            c1: split_point,
            c2: diag2,
            c3: side3,
            c4: self.c4,
        };

        (first_part, second_part)
    }

    // #[inline(always)]
    fn is_p_in_aabb(&self, p: SimdPoint<L>) -> SimdBool<L> {
        let simd_aabb_safe_margin = simd_f32(AABB_SAFE_MARGIN);
        let mut inside;

        let min_x = self.c1.x.simd_min(self.c2.x).simd_min(self.c3.x).simd_min(self.c4.x);
        inside  = (min_x - simd_aabb_safe_margin).simd_le(p.x);

        let max_x = self.c1.x.simd_max(self.c2.x).simd_max(self.c3.x).simd_max(self.c4.x);
        inside &= (max_x + simd_aabb_safe_margin).simd_ge(p.x);

        let min_y = self.c1.y.simd_min(self.c2.y).simd_min(self.c3.y).simd_min(self.c4.y);
        inside &= (min_y - simd_aabb_safe_margin).simd_le(p.y);

        let max_y = self.c1.y.simd_max(self.c2.y).simd_max(self.c3.y).simd_max(self.c4.y);
        inside &= (max_y + simd_aabb_safe_margin).simd_ge(p.y);

        inside
    }
}

const fn simd_u32<const L: usize>(n: u32) -> SimdU32<L> where Lc<L>: Slc {
    SimdU32::from_array([n; L])
}

const fn simd_f32<const L: usize>(n: f32) -> SimdF32<L> where Lc<L>: Slc {
    SimdF32::from_array([n; L])
}

const fn simd_i32<const L: usize>(n: i32) -> SimdI32<L> where Lc<L>: Slc {
    SimdI32::from_array([n; L])
}

const fn simd_p<const L: usize>(p: Point) -> SimdPoint<L> where Lc<L>: Slc {
    SimdPoint::new(simd_f32(p.x), simd_f32(p.y))
}

const fn simd_spm<const P: usize, const L: usize>(offset: usize) -> SimdPoint<L> where Lc<L>: Slc {
    let xy = ssaa_subpixel_map::<P>();

    let mut x = [0.0; 16];
    let mut y = [0.0; 16];
    let mut i = 0;
    while i < L {
        x[i] = xy[offset + i].0;
        y[i] = xy[offset + i].1;
        i += 1;
    }

    SimdPoint::new(SimdF32::from_slice(&x), SimdF32::from_slice(&y))
}
