use super::*;

pub fn quad(
    top_left: Point,
    top_right: Point,
    bottom_left: Point,
    bottom_right: Point,
) -> [CubicBezier; 4] {
    [
        CubicBezier {
            c1: top_left,
            c2: top_left,
            c3: top_right,
            c4: top_right,
        },
        CubicBezier {
            c1: top_right,
            c2: top_right,
            c3: bottom_right,
            c4: bottom_right,
        },
        CubicBezier {
            c1: bottom_right,
            c2: bottom_right,
            c3: bottom_left,
            c4: bottom_left,
        },
        CubicBezier {
            c1: bottom_left,
            c2: bottom_left,
            c3: top_left,
            c4: top_left,
        },
    ]
}

pub fn rectangle(origin: Point, size: Vec2<f32>) -> [CubicBezier; 4] {
    let top_left = origin;
    let top_right = Point::new(origin.x + size.x, origin.y);
    let bottom_left = Point::new(origin.x, origin.y + size.y);
    let bottom_right = origin + size;
    quad(top_left, top_right, bottom_left, bottom_right)
}


