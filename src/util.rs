use super::*;

/// Debugging texture showing a continuous rainbow
///
/// Stripes are diagonal and separated by a transparent line.
pub fn rainbow(x: usize, y: usize) -> Color {
    let i = ((x + y) % 128) >> 4;

    [
        Color::new(255,   0,   0, 255),
        Color::new(255, 127,   0, 255),
        Color::new(255, 255,   0, 255),
        Color::new(  0, 255,   0, 255),
        Color::new(  0,   0, 255, 255),
        Color::new( 75,   0, 130, 255),
        Color::new(148,   0, 211, 255),
        Color::new(255, 255, 255, 100),
    ][i]
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
/// The author's advice: let Rust manage the vector's capacity but re-use the vector between frames.
#[cfg(feature = "stroke")]
pub fn stroke_path(line: &[CubicBezier], width: f32, output: &mut Vec<CubicBezier>, max_error: f32) {
    output.clear();

    if line.is_empty() {
        return;
    }

    let closed = line.first().unwrap().c1 == line.last().unwrap().c4;
    let mut end_index = [0; 2];

    let normal_factor = width * 0.5;

    for side in 0..2 {

        // make room for end connectors
        output.push(Default::default());

        let get_curve = |i| match side {
            0 => line.get(i),
            1 => line.get(line.len().overflowing_sub(i + 1).0),
            _ => unreachable!(),
        }.cloned().map(|c: CubicBezier| c.reversed(side == 1));

        let mut prev_offset: Option<CubicBezier> = None;
        let mut curve_index = 0;
        let mut maybe_curve = get_curve(curve_index);
        let mut trial: f32 = 1.0;

        while let Some(rem_sc) = maybe_curve {
            let (trial_sc, future_sc) = rem_sc.split(trial);

            let this_offset = trial_sc.offset(normal_factor);
            let max_offset_error = trial_sc.max_offset_error(&this_offset, normal_factor, 8);

            if max_offset_error <= max_error {
                if let Some(prev_offset) = prev_offset {
                    if prev_offset.c4 != this_offset.c1 {
                        let ctrl_len = prev_offset.c4.distance(this_offset.c1) * 0.5;
                        let ctrl_v1 = (prev_offset.c4 - prev_offset.c3).normalized() * ctrl_len;
                        let ctrl_v2 = (this_offset.c1 - this_offset.c2).normalized() * ctrl_len;

                        output.push(CubicBezier {
                            c1: prev_offset.c4,
                            c2: prev_offset.c4 + ctrl_v1,
                            c3: this_offset.c1 + ctrl_v2,
                            c4: this_offset.c1,
                        });
                    }
                }

                output.push(this_offset);
                prev_offset = Some(this_offset);

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

        end_index[side] = output.len() - 1;
    }

    let ec_index = [0, end_index[0] + 1];
    let this_index = ec_index.map(|i| i + 1);
    let prev_index = match closed {
        true => end_index,
        false => [end_index[1], end_index[0]],
    };

    for side in 0..2 {

        let this_offset = output[this_index[side]];
        let prev_offset = output[prev_index[side]];

        let ctrl_len = prev_offset.c4.distance(this_offset.c1) * 0.5;
        let ctrl_v1 = (prev_offset.c4 - prev_offset.c3).normalized() * ctrl_len;
        let ctrl_v2 = (this_offset.c1 - this_offset.c2).normalized() * ctrl_len;

        output[ec_index[side]] = CubicBezier {
            c1: prev_offset.c4,
            c2: prev_offset.c4 + ctrl_v1,
            c3: this_offset.c1 + ctrl_v2,
            c4: this_offset.c1,
        };
    }
}