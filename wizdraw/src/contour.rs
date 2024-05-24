use super::*;
use alloc::vec::Vec;

const DEG_90: f32 = core::f32::consts::PI * 0.5;

impl CubicBezier {
    fn reversed(self, actually: bool) -> Self {
        match actually {
            true => CubicBezier {
                c1: self.c4,
                c2: self.c3,
                c3: self.c2,
                c4: self.c1,
            },
            false => self,
        }
    }

    fn offset(&self, normal_factor: f32) -> Self {
        let side1 = travel(self.c1, self.c2, 0.5);
        let side2 = travel(self.c2, self.c3, 0.5);
        let side3 = travel(self.c3, self.c4, 0.5);

        let this_norm_c1 = (self.c2 - self.c1).normalized().rotated_z(DEG_90) * normal_factor;
        let this_norm_c2 = (side2 - side1).normalized().rotated_z(DEG_90) * normal_factor;
        let this_norm_c3 = (side3 - side2).normalized().rotated_z(DEG_90) * normal_factor;
        let this_norm_c4 = (self.c4 - self.c3).normalized().rotated_z(DEG_90) * normal_factor;


        CubicBezier {
            c1: self.c1 + this_norm_c1,
            c2: self.c2 + this_norm_c2,
            c3: self.c3 + this_norm_c3,
            c4: self.c4 + this_norm_c4,
        }
    }

    // along normal
    fn eval_and_offset(&self, t: f32, normal_factor: f32) -> Point {
        let side1 = travel(self.c1, self.c2, t);
        let side2 = travel(self.c2, self.c3, t);
        let side3 = travel(self.c3, self.c4, t);

        let diag1 = travel(side1, side2, t);
        let diag2 = travel(side2, side3, t);

        let split_point = travel(diag1, diag2, t);
        let offset = (diag2 - diag1).normalized().rotated_z(DEG_90) * normal_factor;

        split_point + offset
    }

    // used by util::contour
    fn max_offset_error(&self, offset_curve: &Self, offset: f32, steps: usize) -> f32 {
        let step_inc = 1.0 / (steps as f32);
        let mut t = step_inc;
        let mut max_error = 0.0;

        for _ in 0..(steps - 1) {
            let expected = self.eval_and_offset(t, offset);
            let actual = offset_curve.eval_and_offset(t, 0.0);
            let error = expected.distance(actual);

            if max_error < error {
                max_error = error;
            }

            t += step_inc;
        }

        max_error
    }

    fn segment(start: Point, end: Point) -> Self {
        Self {
            c1: start,
            c2: start,
            c3: end,
            c4: end,
        }
    }
}

/// Creates a Contour composite bezier curve based on another one.
///
/// Input paths which don't start where they end are valid.
///
/// The `width` parameter is the stroke width.
///
/// The `max_error` parameter is used to check that some approximations of the
/// implemented algorithm are correct enough; you can start with `1.0` and lower it
/// if you're unsatisfied with the results; going below `0.1` is probably useless.
///
/// This function allocates if `output`'s capacity wasn't enough. The output can get much
/// bigger than the input (in number of curves), especially if `max_error` is low.
/// With `max_error` = `1.0`, The output can typically get 2-4x bigger than the input.
///
/// Author's advice: let Rust manage the vector's capacity but re-use the vector between frames.
pub fn contour(shape: &[CubicBezier], width: f32, output: &mut Vec<CubicBezier>, max_error: f32) {
    output.clear();

    if shape.is_empty() {
        return;
    }

    let closed = shape.first().unwrap().c1 == shape.last().unwrap().c4;
    // index in output where the last curve lies
    let mut end_index = [0; 2];

    let normal_factor = width * 0.5;

    for side in 0..2 {
        // make room for end connectors
        output.push(Default::default());

        let get_curve = |i| match side {
            0 => shape.get(i),
            1 => shape.get(shape.len().overflowing_sub(i + 1).0),
            _ => unreachable!(),
        }.cloned().map(|c: CubicBezier| c.reversed(side == 1));

        let mut previous: Option<CubicBezier> = None;
        let mut curve_index = 0;
        let mut maybe_curve = get_curve(curve_index);
        let mut trial: f32 = 1.0;

        while let Some(rem_sc) = maybe_curve {
            let (trial_sc, future_sc) = rem_sc.split(trial);

            let shifted = trial_sc.offset(normal_factor);
            let max_offset_error = trial_sc.max_offset_error(&shifted, normal_factor, 8);

            if max_offset_error <= max_error {
                // previous is some for all curves but the first
                if let Some(previous) = previous {
                    // does it end where we start?
                    if previous.c4 != shifted.c1 {
                        // insert a connector to bridge previous end and next start
                        output.push(connector(previous, shifted));
                    }
                }

                output.push(shifted);
                previous = Some(shifted);

                // did we complete this curve?
                if trial == 1.0 {
                    curve_index += 1;
                    maybe_curve = get_curve(curve_index);
                } else {
                    maybe_curve = Some(future_sc);
                    trial = 1.0;
                }

            } else {
                trial *= 0.5;
            }
        }

        if closed {
            output.push(Default::default());
        }

        end_index[side] = output.len() - 1;
    }

    let [outer_end_i, inner_end_i] = end_index;

    if closed {
        let outer_start = output[1];
        let outer_end = output[outer_end_i - 1];
        output[0] = connector(outer_end, outer_start);

        let i = outer_end_i + 1;
        let inner_start = output[i + 1];
        let inner_end = output[inner_end_i - 1];
        output[i] = connector(inner_end, inner_start);

        output[outer_end_i] = CubicBezier::segment(outer_end.c4, inner_end.c4);
        output[inner_end_i] = CubicBezier::segment(inner_end.c4, outer_end.c4);
    } else {
        let outer_start = output[1];
        let outer_end = output[inner_end_i];
        output[0] = connector(outer_end, outer_start);

        let i = outer_end_i + 1;
        let inner_start = output[i + 1];
        let inner_end = output[outer_end_i];
        output[i] = connector(inner_end, inner_start);
    }
}

#[inline(always)]
fn connector(previous: CubicBezier, shifted: CubicBezier) -> CubicBezier {
    let ctrl_len = previous.c4.distance(shifted.c1) * 0.5;
    let ctrl_v1 = (previous.c4 - previous.c3).normalized() * ctrl_len;
    let ctrl_v2 = (shifted.c1 - shifted.c2).normalized() * ctrl_len;

    CubicBezier {
        // extend Pc3 -> Pc4 towards Nc
        // extend Nc2 -> Nc1 towards Pc
        c1: previous.c4,
        c2: previous.c4 + ctrl_v1,
        c3: shifted.c1 + ctrl_v2,
        c4: shifted.c1,
    }
}
