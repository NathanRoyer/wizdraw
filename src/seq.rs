use super::*;

// |num| 1.0 / num.sqrt()
#[inline(always)]
fn fast_inv_sqrt(num: f32) -> f32 {
    f32::from_bits(0x5f37_5a86 - (num.to_bits() >> 1))
}

#[inline(always)]
fn is_curve_straight(curve: CubicBezier) -> bool {
    let close_enough = |p: Point| {
        // https://en.wikipedia.org/wiki/Distance_from_a_point_to_a_line#Line_defined_by_two_points

        let l = curve.c4 - curve.c1;
        let a = l.x * (curve.c1.y - p.y);
        let b = l.y * (curve.c1.x - p.x);

        // distance from p to projected point
        let distance = (a - b).abs() * fast_inv_sqrt(l.x * l.x + l.y * l.y);

        distance < STRAIGHT_THRESHOLD
    };

    close_enough(curve.c2) && close_enough(curve.c3)
}

#[inline(always)]
fn is_p_in_aabb(p: Point, bb: BoundingBox) -> bool {
    (bb.x.0 - AABB_SAFE_MARGIN) <= p.x &&
    (bb.y.0 - AABB_SAFE_MARGIN) <= p.y &&
    (bb.x.1 + AABB_SAFE_MARGIN) >= p.x &&
    (bb.y.1 + AABB_SAFE_MARGIN) >= p.y
}

// Computes a winding number addition based on [S -> E] segment and point P
#[inline(always)]
fn use_segment_for_pip(p: Point, s: Point, e: Point) -> i32 {
    let v1 = p - s;
    let v2 = e - s;
    let d = v1.x * v2.y - v1.y * v2.x;

    let b1 = s.y <= p.y;
    let b2 = e.y > p.y;
    let b3 = d > f32::EPSILON;

    let dec = ( b1) & ( b2) & ( b3);
    let inc = (!b1) & (!b2) & (!b3);

    (inc as i32) - (dec as i32)
}

pub fn subpixel_is_in_path(pixel: Point, path: &[CubicBezier]) -> bool {
    let mut path = path.iter();
    let mut maybe_curve = path.next();
    let mut winding_number: i32 = 0;
    let mut done: f32 = 0.0;
    let mut trial: f32 = 1.0;

    while let Some(curve) = maybe_curve {
        let subcurve = curve.subcurve(done, trial);
        let subcurve_aabb = subcurve.aabb();

        let p_is_in_subcurve_aabb = is_p_in_aabb(pixel, subcurve_aabb);
        let use_as_is = !p_is_in_subcurve_aabb || is_curve_straight(subcurve);

        if use_as_is {

            winding_number += use_segment_for_pip(pixel, subcurve.c1, subcurve.c4);

            // did we complete this curve?
            if trial == 1.0 {
                maybe_curve = path.next();
                done = 0.0;
            } else {
                done += (1.0 - done) * trial;
                trial = 1.0;
            }

        } else {
            trial *= 0.5;
        }
    }

    winding_number != 0
}

pub fn pixel_opacity<const P: usize>(p: Point, path: &[CubicBezier]) -> u8 {
    let mut res = 0.0;

    for offset in ssaa_subpixel_map::<P>() {
        if subpixel_is_in_path(p + Point::from(*offset), path) {
            res += (u8::MAX as f32) / (P as f32);
        }
    }

    res as u8
}
