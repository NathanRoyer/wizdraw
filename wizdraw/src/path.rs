use super::*;

impl Arc {
    pub fn as_five_f32s(&self) -> Option<[f32; 5]> {
        if self.radius <= 0.0 {
            // this arc has no impact on the path
            return None;
        }

        let nonzero_radius_wraps = match self.wraps {
            true => -self.radius,
            false => self.radius,
        };

        Some([
            nonzero_radius_wraps, // nonzero => arc
            self.center.x, // a
            self.center.y, // b
            f32::from_bits(self.min_angle_magic), // c
            f32::from_bits(self.max_angle_magic), // d
        ])
    }
}

impl Segment {
    pub fn as_five_f32s(&self) -> [f32; 5] {
        [
            0.0, // zero => segment
            self.a.x, // a
            self.a.y, // b
            self.b.x, // c
            self.b.y, // d
        ]
    }
}

/////////////////////////////////////////////////////////

/// Packs Arcs and Segments as a vector of `f32`
#[derive(Clone, Debug, Default)]
pub struct Path {
    data: Vec<f32>,
}

impl Path {
    pub fn clear(&mut self) {
        self.data.clear()
    }
}

impl PathApi for Path {
    fn arc(&mut self, arc: Arc) {
        if let Some(encoded) = arc.as_five_f32s() {
            self.data.extend_from_slice(&encoded);
        }
    }

    fn segment(&mut self, segment: Segment) {
        let encoded = segment.as_five_f32s();
        self.data.extend_from_slice(&encoded);
    }
}

impl Shape for Path {
    fn draw(&self, api: &mut dyn PathApi) {
        for chunk in self.data.chunks_exact(5) {
            let first_f32 = chunk[0];
            let (a, b) = (chunk[1], chunk[2]);
            let (c, d) = (chunk[3], chunk[4]);

            if first_f32 == 0.0 {
                let segment = Segment {
                    a: Point::new(a, b),
                    b: Point::new(c, d),
                };

                api.segment(segment);
                continue;
            }

            let arc = Arc {
                radius: first_f32.abs(),
                center: Point::new(a, b),
                min_angle_magic: c.to_bits(),
                max_angle_magic: d.to_bits(),
                wraps: first_f32 < 0.0,
            };

            api.arc(arc);
        }
    }
}

/////////////////////////////////////////////////////////

pub struct Transformer<S: Shape> {
    pub shape: S,
    pub scale: f32,
    pub translation: Vec2<f32>,
}

struct TransformerApi<'a, S: Shape> {
    inner: &'a Transformer<S>,
    real_api: &'a mut dyn PathApi,
}

impl<S: Shape> PathApi for TransformerApi<'_, S> {
    fn segment(&mut self, mut segment: Segment) {
        segment.a *= self.inner.scale;
        segment.b *= self.inner.scale;
        segment.a += self.inner.translation;
        segment.b += self.inner.translation;
        self.real_api.segment(segment);
    }

    fn arc(&mut self, mut arc: Arc) {
        arc.radius *= self.inner.scale;
        arc.center *= self.inner.scale;
        arc.center += self.inner.translation;
        self.real_api.arc(arc);
    }
}

impl<S: Shape> Shape for Transformer<S> {
    fn draw(&self, api: &mut dyn PathApi) {
        let mut wrapper = TransformerApi {
            inner: self,
            real_api: api,
        };

        self.shape.draw(&mut wrapper);
    }
}

/////////////////////////////////////////////////////////

impl Arc {
    pub fn from_points2<F: Into<f64>>(points: [Vec2<F>; 3], center: Vec2<F>, radius: F) -> Self {
        let to_f64 = |float: F| float.into();
        let [a, b, c] = points.map(|coord| coord.map(to_f64));
        let center = center.map(to_f64);
        let radius = radius.into();

        let center_to_a = a - center; // (-1, -1)
        let center_to_b = b - center; // (0, -1)
        let center_to_c = c - center; // (1, -1)

        let c2a_cos = center_to_a.x / radius; // -0.707
        let c2b_cos = center_to_b.x / radius; // 0
        let c2c_cos = center_to_c.x / radius; // +0.707

        // y is positive => angle is negative
        let c2a_neg = center_to_a.y > 0.0; // false
        let c2b_neg = center_to_b.y > 0.0; // false
        let c2c_neg = center_to_c.y > 0.0; // false

        // is the angle of c2a inferior to the angle of c2c
        let sorted = match (c2a_neg, c2c_neg) {
            (true, false) => true,
            (true, true) => c2a_cos < c2c_cos,
            (false, false) => c2a_cos > c2c_cos,
            (false, true) => false,
        };

        let to_f32 = |float: f64| float as f32;

        let (min_cos, max_cos, min_neg, max_neg) = match sorted {
            true => (c2a_cos, c2c_cos, c2a_neg, c2c_neg),
            false => (c2c_cos, c2a_cos, c2c_neg, c2a_neg),
        };

        let min_angle_magic = encode_magic_2(min_cos, min_neg);
        let max_angle_magic = encode_magic_2(max_cos, max_neg);

        let range = cpu::cos_range(min_angle_magic, max_angle_magic, !c2b_neg);
        let wraps = !range.contains(&c2b_cos);

        Self {
            center: center.map(to_f32),
            radius: radius as f32,
            min_angle_magic,
            max_angle_magic,
            wraps,
        }
    }

    pub fn from_points(points: [Point; 3]) -> Option<Self> {
        let to_f64 = |float: f32| float as f64;
        let [a, b, c] = points.map(|coord| coord.map(to_f64));

        let a_to_b = b - a;
        let b_to_c = c - b;
        let mid_ab = (a + b) / 2.0;
        let mid_bc = (b + c) / 2.0;
        let ab_90deg = rotate_90deg(a_to_b);
        let bc_90deg = rotate_90deg(b_to_c);

        let mid_ab_n = mid_ab + ab_90deg;
        let mid_bc_n = mid_bc + bc_90deg;
        let ab_m_abn = mid_ab - mid_ab_n;
        let ab_m_bc = mid_ab - mid_bc;
        let factor = mid_bc - mid_bc_n;

        let m = ab_m_bc.x * factor.y - ab_m_bc.y * factor.x;
        let n = ab_m_abn.x * factor.y - ab_m_abn.y * factor.x;

        if n == 0.0 {
            return None;
        }

        let center = mid_ab + ab_90deg * (m / n);
        let radius = (b - center).magnitude();
        Some(Self::from_points2([a, b, c], center, radius))
    }
}

/////////////////////////////////////////////////////////

impl Path {
    pub fn cubic_bezier(&mut self, points: [Point; 4], max_error: f32) {
        let [a, b, c, d] = points;

        if aligned([a, b, d]) && aligned([a, c, d]) {
            self.segment((a, d).into());
            return;
        }

        let sample = |t: f32| {
            let lerp = Vec2::lerp_unclamped_precise;
            let ab = lerp(a, b, t);
            let bc = lerp(b, c, t);
            let cd = lerp(c, d, t);
            let abc = lerp(ab, bc, t);
            let bcd = lerp(bc, cd, t);
            lerp(abc, bcd, t)
        };

        let mut t = 0.0;
        let mut n = 1.0;
        let mut p1 = a;
        let mut arc_fail = false;

        while t < 1.0 {
            let frac_n_2 = n * 0.5;
            let frac_n_4 = n * 0.25;

            let p2_t = t + frac_n_2;
            let p3_t = t + n;

            let p2 = sample(p2_t);
            let p3 = sample(p3_t);

            let Some(arc) = Arc::from_points([p1, p2, p3]) else {
                if arc_fail {
                    arc_fail = false;
                    self.segment((p1, p3).into());
                    p1 = p3;
                    t = p3_t;
                    n = 1.0 - t;
                } else {
                    arc_fail = true;
                    n = frac_n_4;
                }

                continue;
            };

            arc_fail = false;
            let radius_squared = arc.radius * arc.radius;

            let e1 = sample(p2_t - frac_n_4);
            let e2 = sample(p2_t + frac_n_4);

            let e1_d_squared = Point::distance_squared(e1, arc.center);
            let e2_d_squared = Point::distance_squared(e2, arc.center);
            let e1_error_squared = (e1_d_squared - radius_squared).abs();
            let e2_error_squared = (e2_d_squared - radius_squared).abs();
            let error_squared = e1_error_squared + e2_error_squared;

            if error_squared >= max_error {
                n = frac_n_2;
                continue;
            }

            if arc.radius > 4000.0 || n < 0.01 {
                self.segment((p1, p3).into());
            } else {
                // println!("d² = {}", p1.distance_squared(p3).round());
                // self.segment((arc.center, p1).into());
                // self.segment((arc.center, p2).into());
                // self.segment((p2, p1).into());
                self.arc(arc);
            }

            p1 = p3;
            t = p3_t;
            n = 1.0 - t;
        }
    }
}
