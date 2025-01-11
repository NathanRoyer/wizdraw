use super::*;

use core::simd::prelude::*;
use vek::vec::Vec2;

const LANES: usize = 16;

type SimdI32 = Simd<i32, LANES>;

// IntPoint
type SimdPoint = Vec2<SimdI32>;

// Computes a one bit winding number increment/decrement
#[inline(always)]
fn toggle_in_shape(
    p: SimdPoint,
    s: SimdPoint,
    e: SimdPoint,
) -> SimdI32 {
    let v1 = p - s;
    let v2 = e - s;

    let cond_a = s.y.simd_le(p.y);
    let cond_b = e.y.simd_gt(p.y);
    let cond_c = (v1.x * v2.y).simd_gt(v1.y * v2.x);

    let dec_mask = ( cond_a) & ( cond_b) & ( cond_c);
    let inc_mask = (!cond_a) & (!cond_b) & (!cond_c);

    (dec_mask ^ inc_mask).to_int()
}

#[inline(always)]
pub(super) fn process_row(
    point: IntPoint,
    start: IntPoint,
    end: IntPoint,
    mask_line: &mut [bool],
) {
    let start = simd_point(start);
    let end = simd_point(end);

    let mut point = simd_point(point);
    point.x += init();

    // keep in mind that a mask line is always a power of 2
    let mut x = 0;
    while x < mask_line.len() {
        let toggles = toggle_in_shape(point, start, end);

        for i in 0..LANES {
            mask_line[x] ^= toggles[i] != 0;
            x += 1;
        }

        point.x += simd_i32(MAX_SUBP * (LANES as i32));
    }

}

const fn init() -> SimdI32 {
    let mut array = [MAX_SUBP; LANES];
    let mut i = 0;

    while i < LANES {
        array[i] *= i as i32;
        i += 1;
    }

    SimdI32::from_array(array)
}

#[inline(always)]
fn simd_point(seq_p: IntPoint) -> SimdPoint {
    let x = SimdI32::splat(seq_p.x);
    let y = SimdI32::splat(seq_p.y);
    SimdPoint::new(x, y)
}

const fn simd_i32(n: i32) -> SimdI32 {
    SimdI32::from_array([n; LANES])
}
