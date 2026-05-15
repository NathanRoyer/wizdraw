use super::*;

#[derive(Copy, Clone, Debug)]
pub struct Corner {
    /// Position
    p: Point,
    /// Radius
    r: f32,
}

pub fn corner(p: Point, r: f32) -> Corner {
    Corner { p, r }
}

#[derive(Copy, Clone, Debug)]
struct Segment {
    s: Point,
    e: Point,
}

fn segment(s: Point, e: Point) -> Segment {
    Segment { s, e }
}

impl Segment {
    fn unit(self) -> Vec2<f32> {
        normalize_or_zero(self.e, self.s)
    }
}

impl From<Segment> for CubicBezier {
    fn from(segment: Segment) -> Self {
        CubicBezier {
            c1: segment.s,
            c2: segment.s,
            c3: segment.e,
            c4: segment.e,
        }
    }
}

fn quad_segments(
    top_left: Point,
    top_right: Point,
    btm_left: Point,
    btm_right: Point,
) -> [Segment; 4] {
    let tl_to_tr = segment(top_left, top_right);
    let tr_to_br = segment(top_right, btm_right);
    let br_to_bl = segment(btm_right, btm_left);
    let bl_to_tl = segment(btm_left, top_left);

    [tl_to_tr, tr_to_br, br_to_bl, bl_to_tl]
}

pub fn quad(
    top_left: Point,
    top_right: Point,
    btm_left: Point,
    btm_right: Point,
) -> [CubicBezier; 4] {
    quad_segments(top_left, top_right, btm_left, btm_right).map(CubicBezier::from)
}

pub fn rectangle(origin: Point, size: Vec2<f32>) -> [CubicBezier; 4] {
    let top_left = origin;
    let top_right = Point::new(origin.x + size.x, origin.y);
    let btm_left = Point::new(origin.x, origin.y + size.y);
    let btm_right = origin + size;
    quad(top_left, top_right, btm_left, btm_right)
}

fn corner_bezier(corner: Corner, prev: Segment, next: Segment) -> CubicBezier {
    CubicBezier {
        c1: corner.p - prev.unit() * corner.r,
        c2: corner.p,
        c3: corner.p,
        c4: corner.p + next.unit() * corner.r,
    }
}

pub fn rounded_quad(
    top_left: Corner,
    top_right: Corner,
    btm_left: Corner,
    btm_right: Corner,
) -> [CubicBezier; 8] {
    let segments = quad_segments(top_left.p, top_right.p, btm_left.p, btm_right.p);
    let [tl_to_tr, tr_to_br, br_to_bl, bl_to_tl] = segments;

    let tl_corner = corner_bezier(top_left, bl_to_tl, tl_to_tr);
    let tr_corner = corner_bezier(top_right, tl_to_tr, tr_to_br);
    let br_corner = corner_bezier(btm_right, tr_to_br, br_to_bl);
    let bl_corner = corner_bezier(btm_left, br_to_bl, bl_to_tl);

    let tl_to_tr = segment(tl_corner.c4, tr_corner.c1).into();
    let tr_to_br = segment(tr_corner.c4, br_corner.c1).into();
    let br_to_bl = segment(br_corner.c4, bl_corner.c1).into();
    let bl_to_tl = segment(bl_corner.c4, tl_corner.c1).into();

    [tl_corner, tl_to_tr, tr_corner, tr_to_br, br_corner, br_to_bl, bl_corner, bl_to_tl]
}

pub fn rounded_quad_uniradius(
    top_left: Point,
    top_right: Point,
    btm_left: Point,
    btm_right: Point,
    radius: f32,
) -> [CubicBezier; 8] {
    rounded_quad(
        corner(top_left, radius),
        corner(top_right, radius),
        corner(btm_left, radius),
        corner(btm_right, radius),
    )
}
