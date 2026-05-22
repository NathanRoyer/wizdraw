use super::*;

impl Arc {
    pub fn as_five_f32s(&self) -> Option<[f32; 5]> {
        if self.radius <= 0.0 {
            // this arc has no impact on the path
            return None;
        }

        let nonzero_radius_clockwise = match self.clockwise {
            true => -self.radius,
            false => self.radius,
        };

        Some([
            nonzero_radius_clockwise, // nonzero => arc
            self.center.x,    // a
            self.center.y,    // b
            self.start_angle, // c
            self.stop_angle,  // d
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

    pub fn arc(&mut self, arc: &Arc) {
        if let Some(encoded) = arc.as_five_f32s() {
            self.data.extend_from_slice(&encoded);
        }
    }

    pub fn segment(&mut self, segment: &Segment) {
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
                start_angle: c,
                stop_angle: d,
                clockwise: first_f32 < 0.0,
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

pub struct Striker<S: Shape> {
    pub shape: S,
    pub width: f32,
}

struct StrikerApi<'a, S: Shape> {
    inner: &'a Striker<S>,
    real_api: &'a mut dyn PathApi,
}

impl<S: Shape> PathApi for StrikerApi<'_, S> {
    fn segment(&mut self, mut segment: Segment) {
        if segment.a == segment.b {
            return;
        }

        let a_to_b = segment.b - segment.a;
        let rotated = Vec2::new(-a_to_b.y, a_to_b.x);
        let normal = rotated.normalized() * self.inner.width;

        let mut clone = segment;
        clone.a += normal;
        clone.b += normal;
        self.real_api.segment(clone);

        segment.a -= normal;
        segment.b -= normal;
        self.real_api.segment(segment);
    }

    fn arc(&mut self, mut arc: Arc) {
        arc.radius += self.inner.width;
        self.real_api.arc(arc);
        arc.radius -= self.inner.width;
        arc.radius -= self.inner.width;
        self.real_api.arc(arc);
    }
}

impl<S: Shape> Shape for Striker<S> {
    fn draw(&self, api: &mut dyn PathApi) {
        let mut wrapper = StrikerApi {
            inner: self,
            real_api: api,
        };

        self.shape.draw(&mut wrapper);
    }
}

/////////////////////////////////////////////////////////

impl Arc {
    pub fn from_points(points: [Point; 3]) -> Option<Self> {
        let [a, b, c] = points;

        let a_to_b = b - a;
        let b_to_c = c - b;
        let mid_ab = (a + b) * 0.5;
        let mid_bc = (b + c) * 0.5;
        let ab_normal = Point::new(-a_to_b.y, a_to_b.x);
        let bc_normal = Point::new(-b_to_c.y, b_to_c.x);

        let mid_ab_n = mid_ab + ab_normal;
        let mid_bc_n = mid_bc + bc_normal;
        let y_factor = mid_bc.y - mid_bc_n.y;
        let x_factor = mid_bc.x - mid_bc_n.x;
        let ab_m_bc = mid_ab - mid_bc;
        let ab_m_abn = mid_ab - mid_ab_n;

        let m = ab_m_bc.x * y_factor - ab_m_bc.y * x_factor;
        let n = ab_m_abn.x * y_factor - ab_m_abn.y * x_factor;

        if n == 0.0 {
            return None;
        }

        let center = mid_ab + ab_normal * (m / n);

        let center_to_a = a - center;
        let center_to_b = b - center;
        let center_to_c = c - center;

        let radius = center_to_a.magnitude();

        // negation: screen coordinates
        let a_angle = f32::atan2(-center_to_a.y, center_to_a.x);
        let b_angle = f32::atan2(-center_to_b.y, center_to_b.x);
        let c_angle = f32::atan2(-center_to_c.y, center_to_c.x);

        let sorted = c_angle < a_angle;

        let (min, max) = match sorted {
            false => (a_angle, c_angle),
            true => (c_angle, a_angle),
        };

        let clockwise = (min..=max).contains(&b_angle) ^ sorted;

        Some(Self {
            center,
            radius,
            start_angle: c_angle,
            stop_angle: a_angle,
            clockwise,
        })
    }
}

#[test]
fn test_points_to_arc() {
    use core::f32::consts::*;

    let angle = FRAC_PI_4;
    let a = Point::new(0.0, 1.0);
    let b = Point::new(angle.cos(), angle.sin());
    let c = Point::new(1.0, 0.0);
    let d = (a + c) / 2.0;

    let arc_1 = Arc {
        center: Point::new(0.0, 0.0),
        radius: 1.0,
        start_angle: 0.0,
        stop_angle: -FRAC_PI_2,
        clockwise: true,
    };

    let arc_2 = Arc {
        center: Point::new(0.0, 0.0),
        radius: 1.0,
        start_angle: -FRAC_PI_2,
        stop_angle: 0.0,
        clockwise: false,
    };

    assert_eq!(Arc::from_points([a, d, c]), None);
    assert_eq!(Arc::from_points([a, b, c]), Some(arc_1));
    assert_eq!(Arc::from_points([c, b, a]), Some(arc_2));
}

/////////////////////////////////////////////////////////

fn aligned(points: [Point; 3]) -> bool {
    let [a, b, c] = points;
    let m = (b.y - a.y) * (c.x - a.x);
    let n = (c.y - a.y) * (b.x - a.x);
    (m - n).abs() < 0.0001
}

impl Path {
    pub fn cubic_bezier(&mut self, points: [Point; 4], max_error: f32) {
        let [a, b, c, d] = points;

        if aligned([a, b, d]) && aligned([a, c, d]) {
            self.segment(&Segment { a, b: d });
            return;
        }

        let sample = |t: f32| {
            let ab = Point::lerp_unclamped_precise(a, b, t);
            let bc = Point::lerp_unclamped_precise(b, c, t);
            let cd = Point::lerp_unclamped_precise(c, d, t);
            let abc = Point::lerp_unclamped_precise(ab, bc, t);
            let bcd = Point::lerp_unclamped_precise(bc, cd, t);
            Point::lerp_unclamped_precise(abc, bcd, t)
        };

        let mut t = 0.0;
        let mut n = 1.0;
        let mut p1 = sample(t);

        while t < 1.0 {
            let frac_n_2 = n * 0.5;
            let frac_n_4 = n * 0.25;

            let p2_t = t + frac_n_2;
            let p3_t = t + n;

            let p2 = sample(p2_t);
            let p3 = sample(p3_t);

            let Some(arc) = Arc::from_points([p1, p2, p3]) else {
                self.segment(&Segment { a: p1, b: p3 });
                p1 = p3;
                t = p3_t;
                n = 1.0 - t;
                continue;
            };

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

            self.arc(&arc);
            p1 = p3;
            t = p3_t;
            n = 1.0 - t;
        }
    }
}
