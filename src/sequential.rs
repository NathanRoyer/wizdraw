use super::{Element, aabr, sub_segments, ssaa_average};

use vek::vec::Vec2;

#[inline(always)]
fn is_inside(p: Vec2<Element>, path: &[Vec2<Element>]) -> bool {
    let mut winding_number = 0isize;

    for segment in path.windows(2) {
        let (s, e) = (segment[0], segment[1]);

        let v1 = p - s;
        let v2 = e - s;
        let d = v1.x * v2.y - v1.y * v2.x;
        if s.y <= p.y {
            if e.y > p.y && d > Element::EPSILON {
                winding_number -= 1;
            }
        } else {
            if e.y <= p.y && d < Element::EPSILON {
                winding_number += 1;
            }
        }
    }

    winding_number != 0
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
                let last_point = line_start + x;
                let range = (last_point - go_back)..=last_point;
                let avg = ssaa_average::<_, SSAA>(x as isize, y as isize, is_inside_path);
                mask[range].fill(avg);
                go_back = 0;
            }
        }

        line_start += mask_size.x;
    }
}
