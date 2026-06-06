use super::*;

struct PointCheck {
    point: Point,
    inside: bool,
}

fn apply(x: f32, col: f32, inside: &mut bool) {
    *inside ^= x < col;
}

impl PathApi for PointCheck {
    fn segment(&mut self, segment: Segment) {
        let precomp = SegmentPrecomp::new(segment);
        let col_check = |x: f32| apply(self.point.x, x, &mut self.inside);
        precomp.collision(self.point.y, col_check);
    }

    fn arc(&mut self, arc: Arc) {
        let precomp = ArcPrecomp::new(arc);
        let col_check = |x: f32| apply(self.point.x, x, &mut self.inside);
        precomp.collision(self.point.y, col_check);
    }
}

pub fn shape_contains(point: Point, shape: &dyn Shape) -> bool {
    let mut point_check = PointCheck {
        inside: false,
        point,
    };

    shape.draw(&mut point_check);
    point_check.inside
}






