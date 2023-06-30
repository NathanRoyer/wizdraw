use super::{Element, aabr, sub_segments};

use core::{ops::Range, simd::{Simd, SimdPartialOrd, SimdPartialEq}, array::from_fn};
use vek::vec::Vec2;

#[cfg(not(feature = "f64"))]
type SimdElement = core::simd::f32x16;
#[cfg(feature = "f64")]
type SimdElement = core::simd::f64x8;

#[cfg(not(feature = "f64"))]
const SIMD_EL_COUNT: usize = 16;
#[cfg(feature = "f64")]
const SIMD_EL_COUNT: usize = 8;

type SimdBool = core::simd::Simd<u16, SIMD_EL_COUNT>;

const SIMD_ZERO: SimdElement = SimdElement::from_array([0.0; SIMD_EL_COUNT]);
const SIMD_EPSILON: SimdElement = SimdElement::from_array([Element::EPSILON; SIMD_EL_COUNT]);

fn splat_x_y(p: Vec2<Element>) -> (SimdElement, SimdElement) {
    (
        SimdElement::from_array([p.x; SIMD_EL_COUNT]),
        SimdElement::from_array([p.y; SIMD_EL_COUNT])
    )
}

#[inline(always)]
fn is_inside(p_x: SimdElement, p_y: SimdElement, path: &[Vec2<Element>]) -> SimdBool {
    let zero = Simd::from_array([0i32; SIMD_EL_COUNT]);
    let mut winding_number = zero;

    let (mut s_x, mut s_y) = splat_x_y(path[0]);
    for i in 1..path.len() {
        let (e_x, e_y) = splat_x_y(path[i]);

        let v1_x = p_x - s_x;
        let v1_y = p_y - s_y;
        let v2_x = e_x - s_x;
        let v2_y = e_y - s_y;
        let d = v1_x * v2_y - v1_y * v2_x;

        let cond_a = s_y.simd_le(p_y);
        let cond_b = e_y.simd_gt(p_y);
        let cond_c = d.simd_gt(SIMD_EPSILON);

        let dec_mask = ( cond_a) & ( cond_b) & ( cond_c);
        let inc_mask = (!cond_a) & (!cond_b) & (!cond_c);

        // to_int flips the sign, so dec/inc are inverted
        winding_number += dec_mask.to_int();
        winding_number -= inc_mask.to_int();

        s_x = e_x;
        s_y = e_y;
    }

    // will return { inside => 1, outside => 0 }
    (-winding_number.simd_ne(zero).to_int()).cast()
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
pub fn fill<const SSAA: usize, const SSAA_SQ: usize>(
    path: &[Vec2<Element>],
    mask: &mut [u8],
    mask_size: Vec2<usize>,
) {
    assert_eq!(SSAA * SSAA, SSAA_SQ, "SSAA_SQ must be the square of SSAA");

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

            let min_x_px = min_x_px.max(0) as usize;
            let max_x_px = max_x_px.min(w as _) as usize;
            cov_line[min_x_px..max_x_px].fill(255);
        }
    };

    for segment in path.windows(2) {
        sub_segments(segment[0], segment[1], &mut process_sub_segment, 5);
    }

    let mut ssaa_coords = SsaaPathProcessor::<_, SSAA, SSAA_SQ>::new(|x, y| is_inside(x, y, path));

    let mut line_start = 0;
    for y in 0..mask_size.y {
        let mut go_back = 0;
        for x in 0..mask_size.x {
            let not_last_point = x != (mask_size.x - 1);
            if mask[line_start + x] == 0 && not_last_point {
                go_back += 1;
            } else {
                let last_point = line_start + x;
                let range = (last_point - go_back)..(last_point + 1);
                ssaa_coords.set(x as isize, y as isize, range);
                ssaa_coords.flush(mask, false);
                go_back = 0;
            }
        }

        line_start += mask_size.x;
    }

    ssaa_coords.flush(mask, true);
}

struct SsaaPathProcessor<F: Fn(SimdElement, SimdElement) -> SimdBool, const SSAA: usize, const SSAA_SQ: usize> {
    range: [Range<usize>; SIMD_EL_COUNT],
    src_x: [SimdElement; SSAA_SQ],
    src_y: [SimdElement; SSAA_SQ],
    pixel: usize,
    condition: F,
}

impl<F: Fn(SimdElement, SimdElement) -> SimdBool, const SSAA: usize, const SSAA_SQ: usize> SsaaPathProcessor<F, SSAA, SSAA_SQ> {
    fn new(condition: F) -> Self {
        Self {
            range: from_fn(|_| 0..1),
            src_x: [SIMD_ZERO; SSAA_SQ],
            src_y: [SIMD_ZERO; SSAA_SQ],
            pixel: 0,
            condition,
        }
    }

    /// Insert coordinates (pixel < 8 && sub_pixel < SSAA_SQ)
    #[inline(always)]
    fn set(&mut self, x: isize, y: isize, range: Range<usize>) {
        assert!(self.pixel < SIMD_EL_COUNT);
        let sub_px_width = (SSAA as Element).recip();
        let half_sub_px_width = sub_px_width / 2.0;

        for sx in 0..SSAA {
            for sy in 0..SSAA {
                let mut x = (x as Element) + half_sub_px_width;
                let mut y = (y as Element) + half_sub_px_width;

                for _ in 0..sx { x += sub_px_width; }
                for _ in 0..sy { y += sub_px_width; }

                let sub_pixel = sy * SSAA + sx;
                self.src_x[sub_pixel][self.pixel] = x;
                self.src_y[sub_pixel][self.pixel] = y;
            }
        }

        self.range[self.pixel] = range;
        self.pixel += 1;
    }

    fn flush(&mut self, mask: &mut [u8], force: bool) {
        if self.pixel == SIMD_EL_COUNT || force {
            let mut in_count = Simd::from_array([0 as u16; SIMD_EL_COUNT]);
            let u8_max = Simd::from_array([u8::MAX as u16; SIMD_EL_COUNT]);
            let ssaasq = Simd::from_array([SSAA_SQ as u16; SIMD_EL_COUNT]);

            for sub_pixel in 0..SSAA_SQ {
                let x = self.src_x[sub_pixel];
                let y = self.src_y[sub_pixel];

                in_count += (self.condition)(x, y);
            }

            // scale ratio on 0..255
            let values = ((u8_max * in_count) / ssaasq).cast();

            while self.pixel > 0 {
                self.pixel -= 1;
                let range = self.range[self.pixel].clone();
                mask[range].fill(values[self.pixel]);
            }
        }
    }
}
