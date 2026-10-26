use super::*;

#[derive(Copy, Clone, Debug)]
pub struct Polygon<P: AsRef<[Point]>> {
    pub radius: f32,
    pub points: P,
}

fn corner_arc_points(points: [Point; 3], radius: f64) -> Option<[Point64; 4]> {
    if radius == 0.0 {
        return None;
    }

    let to_f64 = |float: f32| float as f64;
    let [a, b, c] = points.map(|coord| coord.map(to_f64));

    let (b_to_a, b_to_c) = (a - b, c - b);
    let b_to_a_len = b_to_a.magnitude(); // calls f64::sqrt
    let b_to_c_len = b_to_c.magnitude(); // calls f64::sqrt
    let b_to_a_norm = b_to_a / b_to_a_len;
    let b_to_c_norm = b_to_c / b_to_c_len;

    let b_to_d = (b_to_a_norm + b_to_c_norm) / 2.0;

    if b_to_d.x == 0.0 && b_to_d.y == 0.0 {
        return None;
    }

    let b_to_d_norm = b_to_d.normalized(); // calls f64::sqrt

    let mut b_to_d_nrot = rotate_90deg(b_to_d_norm);
    let mut cos_adj_hyp = Vec2::dot(b_to_d_nrot, b_to_c_norm);

    // todo optimize
    if cos_adj_hyp < 0.0 {
        b_to_d_nrot = -b_to_d_nrot;
        cos_adj_hyp = Vec2::dot(b_to_d_nrot, b_to_c_norm);
    }

    if cos_adj_hyp == 0.0 {
        return None;
    }

    let hyp_len = radius / cos_adj_hyp;
    let center = b + b_to_d_norm * hyp_len;
    let j = center - b_to_d_norm * radius;

    let cos_adj_hyp = Vec2::dot(b_to_d_norm, b_to_c_norm);
    let adj_len = cos_adj_hyp * hyp_len;
    let double_adj_len = adj_len * 2.0;

    // this cancels artefacts ocurring when a point is too close to an adjacent segment
    if double_adj_len > b_to_a_len || double_adj_len > b_to_c_len {
        return None;
    }

    let i = b + b_to_a_norm * adj_len;
    let k = b + b_to_c_norm * adj_len;

    Some([i, j, k, center])
}

fn draw_quarter(points: [Point; 3], radius: f32, api: &mut dyn PathApi) {
    let [a, b, c] = points;

    let mid_ab = (a + b) / 2.0;
    let mid_bc = (b + c) / 2.0;
    let radius64 = radius as f64;

    if let Some([i, j, k, center]) = corner_arc_points(points, radius64) {
        api.segment((mid_ab, i.map(|c| c as f32)).into());
        api.arc(Arc::from_points2([i, j, k], center, radius64));
        api.segment((k.map(|c| c as f32), mid_bc).into());
    } else {
        api.segment((mid_ab, b).into());
        api.segment((b, mid_bc).into());
    }
}

impl<P: AsRef<[Point]>> Shape for Polygon<P> {
    fn draw(&self, api: &mut dyn PathApi) {
        let points = self.points.as_ref();
        let len = points.len();
        assert!(len >= 4);

        for trio in points.array_windows() {
            draw_quarter(*trio, self.radius, api);
        }

        let a = points[0];
        let b = points[1];
        let y = points[len - 2];
        let z = points[len - 1];

        draw_quarter([y, z, a], self.radius, api);
        draw_quarter([z, a, b], self.radius, api);
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Rectangle {
    pub radius: f32,
    pub points: [Point; 2],
}

impl Rectangle {
    pub fn new(points: [Point; 2]) -> Self {
        Self::rounded(points, 0.0)
    }

    pub fn rounded(points: [Point; 2], radius: f32) -> Self {
        Self { points, radius }
    }

    pub fn abcd(self) -> [Point; 4] {
        let [a, c] = self.points;
        let b = Point::new(c.x, a.y);
        let d = Point::new(a.x, c.y);
        [a, b, c, d]
    }
}

impl From<Rectangle> for Polygon<[Point; 4]> {
    fn from(rectangle: Rectangle) -> Self {
        Self {
            radius: rectangle.radius,
            points: rectangle.abcd(),
        }
    }
}

impl Shape for Rectangle {
    fn draw(&self, api: &mut dyn PathApi) {
        if self.radius <= 0.0 {
            // only draw the right and left sides
            let [a, b, c, d] = self.abcd();
            api.segment((a, d).into());
            api.segment((b, c).into());
        } else {
            Polygon::from(*self).draw(api);
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Disk {
    pub center: Point,
    pub radius: f32,
}

impl Shape for Disk {
    fn draw(&self, api: &mut dyn PathApi) {
        api.arc(Arc::disk(self.center, self.radius));
    }
}

////////////////////////////////////////////////////////

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

////////////////////////////////////////////////////////

pub struct Combiner<A: Shape, B: Shape> {
    pub a: A,
    pub b: B,
}

impl<A: Shape, B: Shape> Shape for Combiner<A, B> {
    fn draw(&self, api: &mut dyn PathApi) {
        self.a.draw(api);
        self.b.draw(api);
    }
}
