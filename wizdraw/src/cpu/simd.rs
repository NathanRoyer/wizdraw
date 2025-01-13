use super::*;

use core::simd::prelude::*;
use core::array::from_fn;
use vek::vec::Vec2;

const LANES: usize = 16;
pub const S_TILE_W: usize = TILE_W / LANES;

type SimdI32 = Simd<i32, LANES>;

// IntPoint
pub type SimdPoint = Vec2<SimdI32>;

pub fn prepare_coords(row_coords: &[IntPoint; TILE_W]) -> [SimdPoint; S_TILE_W] {
    let x = row_coords.map(|c| c.x);
    let y = row_coords.map(|c| c.y);

    let mut x = x.chunks(LANES);
    let mut y = y.chunks(LANES);

    from_fn(|_i| {
        let simd_x = SimdI32::from_slice(x.next().unwrap());
        let simd_y = SimdI32::from_slice(y.next().unwrap());
        SimdPoint::new(simd_x, simd_y)
    })
}

// Computes a one bit winding number increment/decrement
#[inline(always)]
fn simd_toggle_in_shape(
    p: SimdPoint,
    s: SimdPoint,
    e: SimdPoint,
) -> MaskRow {
    let v1 = p - s;
    let v2 = e - s;

    let cond_a = s.y.simd_le(p.y);
    let cond_b = e.y.simd_gt(p.y);
    let cond_c = (v1.x * v2.y).simd_gt(v1.y * v2.x);

    let dec_mask = ( cond_a) & ( cond_b) & ( cond_c);
    let inc_mask = (!cond_a) & (!cond_b) & (!cond_c);

    (dec_mask ^ inc_mask).to_bitmask() as MaskRow
}

#[inline(always)]
pub(super) fn process_row(
    y: usize,
    simd_coords: &[SimdPoint; S_TILE_W],
    start: IntPoint,
    end: IntPoint,
) -> MaskRow {
    let row_offset = IntPoint::new(0, y as i32 * PX_WIDTH);
    let row_offset = simd_point(row_offset);
    let start = simd_point(start);
    let end = simd_point(end);
    let mut xor_mask = 0;

    for i in 0..S_TILE_W {
        let shifted = simd_coords[i] + row_offset;
        let toggles = simd_toggle_in_shape(shifted, start, end);
        xor_mask ^= toggles << (LANES * i);
    }

    xor_mask
}

#[inline(always)]
fn simd_point(seq_p: IntPoint) -> SimdPoint {
    let x = SimdI32::splat(seq_p.x);
    let y = SimdI32::splat(seq_p.y);
    SimdPoint::new(x, y)
}
