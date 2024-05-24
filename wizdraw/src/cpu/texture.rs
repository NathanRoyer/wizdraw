use super::*;
use vek::num_traits::Euclid;
use super::seq::use_segment_for_pip;

impl Texture<'_> {
    pub(super) fn sample<const P: usize>(
        &self,
        pixel: Point,
        bitmaps: &Bitmaps,
    ) -> Color {
        use rgb::ComponentMap;

        let mut res = rgb::RGBA::new(0, 0, 0, 0);

        for offset in ssaa_subpixel_map::<P>() {
            let point = pixel + Point::from(*offset);
            let color = self.sample_once(point, bitmaps);
            res += color.map(|byte| byte as u16);
        }

        res.map(|word| (word / (P as u16)) as u8)
    }

    fn sample_once(
        &self,
        pixel: Point,
        bitmaps: &Bitmaps,
    ) -> Color {
        match self {
            Texture::SolidColor(color) => *color,
            Texture::Gradient(_slice) => todo!(),
            Texture::Debug => rainbow(pixel),
            Texture::Bitmap {
                top_left,
                scale,
                repeat,
                bitmap,
            } => {
                let bmp = bitmaps.get(*bitmap);
                bmp.sample_scaled(pixel, *top_left, *scale, *repeat)
            },
            Texture::QuadBitmap {
                top_left,
                btm_left,
                top_right,
                btm_right,
                bitmap,
            } => {
                let bmp = bitmaps.get(*bitmap);
                let sample = bmp.sample_quad(
                    pixel,
                    *top_left,
                    *btm_left,
                    *top_right,
                    *btm_right,
                );
                sample.unwrap_or(TRANSPARENT)
            },
        }
    }
}

pub fn rainbow(point: Point) -> Color {
    let point = point.map(|f| f as usize);
    let i = ((point.x + point.y) % 128) >> 4;

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

impl Bitmap {
    fn sample(&self, texture_offset: Point) -> Color {
        let x = texture_offset.x as usize;
        let y = texture_offset.y as usize;
        let i = y * self.size.x + x;

        self.pixels.get(i).copied().unwrap_or(TRANSPARENT)
    }

    pub fn sample_scaled(
        &self,
        pixel: Point,
        top_left: Point,
        scale: f32,
        repeat: bool,
    ) -> Color {
        let float_size = self.size.map(|uint| uint as f32);
        let scaled_size = float_size * scale;

        let offset = pixel - top_left;
        let offset = match repeat {
            true => offset.rem_euclid(&scaled_size),
            false => offset,
        };

        let invalid_x = 0.0 > offset.x || offset.x > scaled_size.x;
        let invalid_y = 0.0 > offset.y || offset.y > scaled_size.y;

        if invalid_x || invalid_y {
            // out of bounds
            return TRANSPARENT;
        }

        self.sample((offset / scaled_size) * float_size)
    }

    pub fn sample_quad(
        &self,
        point: Point,
        top_left: Point,
        btm_left: Point,
        top_right: Point,
        btm_right: Point,
    ) -> Option<Color> {
        let quad = [
            top_left,
            top_right,
            btm_right,
            btm_left,
        ];

        // get arrays of X/Y
        let quad_x = quad.map(|anchor| anchor.x);
        let quad_y = quad.map(|anchor| anchor.y);

        // early return (failed AABB check)
        if is_p_ooaabb(point, quad_x, quad_y) {
            return None;
        }

        // compute winding number
        let mut last_corner = btm_left;
        let mut wind_num = 0i32;
        for next_corner in quad {
            wind_num += use_segment_for_pip(point, last_corner, next_corner);
            last_corner = next_corner;
        }

        // return if point not in quad
        if wind_num % 2 == 0 {
            return None;
        }

        let uv = inverse_bilinear(point, top_left, top_right, btm_right, btm_left)?;

        let w = self.size.x as f32;
        let h = self.size.y as f32;
        Some(self.sample(Point::new(uv.x * w, uv.y * h)))
    }
}

// https://www.reedbeta.com/blog/quadrilateral-interpolation-part-2/
// https://iquilezles.org/articles/ibilinear/
// https://www.gamedev.net/forums/topic/596392-uv-coordinate-on-a-2d-quadrilateral/
//
fn inverse_bilinear(
    pt: Point,
    tl: Point,
    tr: Point,
    br: Point,
    bl: Point,
) -> Option<Point> {
    let wedge = |a: Point, b: Point| {
        a.x * b.y - a.y * b.x
    };

    let e = tr - tl;
    let f = bl - tl;
    let g = tl - tr + br - bl;
    let h = pt - tl;
        
    let k2 = wedge(g, f);
    let k1 = wedge(e, f) + wedge(h, g);
    let k0 = wedge(h, e);

    //            epsilon?
    if k2.abs() < 0.001 {
        // if edges are parallel, this is a linear equation

        let u = (h.x * k1 + f.x * k0) / (e.x * k1 - g.x * k0);
        let v = -k0 / k1;
        Some(Point::new(u, v))

    } else {
        // otherwise, it's a quadratic

        let d = k1 * k1 - 4.0 * k0 * k2;

        if d < 0.0 {
            return None;
        }

        let w = d.sqrt();

        let ik2 = 0.5 / k2;
        let mut v = (-k1 - w) * ik2;
        let mut u = (h.x - f.x * v) / (e.x + g.x * v);
        
        if u < 0.0 || u > 1.0 || v < 0.0 || v > 1.0 {
            v = (-k1 + w) * ik2;
            u = (h.x - f.x * v) / (e.x + g.x * v);
        }

        Some(Point::new(u, v))
    }
}

fn is_p_ooaabb(p: Point, quad_x: [f32; 4], quad_y: [f32; 4]) -> bool {
    // find AABB
    let (min_x, max_x) = min_max(quad_x);
    let (min_y, max_y) = min_max(quad_y);

    // check AABB
    let x_ooaabb = min_x > p.x || p.x > max_x;
    let y_ooaabb = min_y > p.y || p.y > max_y;

    x_ooaabb || y_ooaabb
}
