use super::*;
use vek::num_traits::Euclid;

const TRANSPARENT: Color16 = Color16::new(0, 0, 0, 0);

pub trait Sampler {
    const FLAT: bool = false;
    fn sample(&self, point: Point) -> Color16;
}

//////////////////////////////////////////////////////////

pub struct SolidSampler(Color16);

impl Sampler for SolidSampler {
    const FLAT: bool = true;
    fn sample(&self, _point: Point) -> Color16 {
        self.0
    }
}

//////////////////////////////////////////////////////////

pub struct RainbowSampler;

impl Sampler for RainbowSampler {
    #[inline(always)]
    fn sample(&self, point: Point) -> Color16 {
        const RAINBOW: [Color16; 8] = [
            Color16::new(255,   0,   0, 255),
            Color16::new(255, 127,   0, 255),
            Color16::new(255, 255,   0, 255),
            Color16::new(  0, 255,   0, 255),
            Color16::new(  0,   0, 255, 255),
            Color16::new( 75,   0, 130, 255),
            Color16::new(148,   0, 211, 255),
            Color16::new( 50,  50,  50, 255),
        ];

        let point = point.map(|f| f as usize);
        let i = ((point.x + point.y) & 127) >> 4;

        RAINBOW[i]
    }
}

//////////////////////////////////////////////////////////

pub struct MaskSampler<'a> {
    mask: &'a MaskImage,
    top_left: Point,
    scaled_size: Vec2<f32>,
    inv_scale: f32,
    repeat: bool,
    zero: Option<Color16>,
    full: Color16,
}

impl<'a> MaskSampler<'a> {
    #[inline(always)]
    fn sample(&self, texture_offset: Point) -> Color16 {
        let x = texture_offset.x as usize;
        let y = texture_offset.y as usize;
        let i = y * self.mask.size.x + x;
        let f_factor = self.mask.pixels.get(i).copied().unwrap_or(0);
        let z_factor = 255 - f_factor;
        let mut full = self.full;

        if let Some(zero) = self.zero {
            let full_part = full.map(|c| (f_factor as u16) * c / 255u16);
            let zero_part = zero.map(|c| (z_factor as u16) * c / 255u16);
            full_part + zero_part
        } else {
            full.a = (f_factor as u16) * full.a / 255u16;
            full
        }
    }

    fn new(
        masks: &'a [MaskImage],
        mask_id: MaskHandle,
        top_left: Point,
        repeat: bool,
        scale: f32,
        zero: Option<Color>,
        full: Color,
    ) -> Option<Self> {
        let mask = masks.get(mask_id.0)?;
        let float_size = mask.size.map(|uint| uint as f32);
        let scaled_size = float_size * scale;
        let inv_scale = 1.0 / scale;

        Some(Self {
            mask,
            top_left,
            repeat,
            inv_scale,
            scaled_size,
            zero: zero.map(|z| Color::map(&z, |c| c as u16)),
            full: full.map(|c| c as u16),
        })
    }
}

impl Sampler for MaskSampler<'_> {
    #[inline(always)]
    fn sample(&self, point: Point) -> Color16 {
        let mut offset = point - self.top_left;

        if self.repeat {
            offset = offset.rem_euclid(&self.scaled_size);
        } else {
            let invalid_x = 0.0 > offset.x || offset.x > self.scaled_size.x;
            let invalid_y = 0.0 > offset.y || offset.y > self.scaled_size.y;

            if invalid_x || invalid_y {
                // out of bounds
                return TRANSPARENT;
            }
        }

        self.sample(offset * self.inv_scale)
    }
}

//////////////////////////////////////////////////////////

impl Bitmap {
    #[inline(always)]
    fn sample(&self, texture_offset: Point) -> Color16 {
        let x = texture_offset.x as usize;
        let y = texture_offset.y as usize;
        let i = y * self.size.x + x;

        match self.pixels.get(i) {
            Some(c) => Color16::from(*c),
            None => TRANSPARENT,
        }
    }
}

pub struct ScaledSampler<'a> {
    bitmap: &'a Bitmap,
    top_left: Point,
    scaled_size: Vec2<f32>,
    inv_scale: f32,
    repeat: bool,
}

impl<'a> ScaledSampler<'a> {
    fn new(
        bitmaps: &'a [Bitmap],
        bmp_id: BitmapHandle,
        top_left: Point,
        repeat: bool,
        scale: f32,
    ) -> Option<Self> {
        let bitmap = bitmaps.get(bmp_id.0)?;
        let float_size = bitmap.size.map(|uint| uint as f32);
        let scaled_size = float_size * scale;
        let inv_scale = 1.0 / scale;

        Some(Self {
            bitmap,
            top_left,
            repeat,
            inv_scale,
            scaled_size,
        })
    }
}

impl Sampler for ScaledSampler<'_> {
    #[inline(always)]
    fn sample(&self, point: Point) -> Color16 {
        let mut offset = point - self.top_left;

        if self.repeat {
            offset = offset.rem_euclid(&self.scaled_size);
        } else {
            let invalid_x = 0.0 > offset.x || offset.x > self.scaled_size.x;
            let invalid_y = 0.0 > offset.y || offset.y > self.scaled_size.y;

            if invalid_x || invalid_y {
                // out of bounds
                return TRANSPARENT;
            }
        }

        self.bitmap.sample(offset * self.inv_scale)
    }
}

//////////////////////////////////////////////////////////

pub struct QuadSampler<'a> {
    bitmap: &'a Bitmap,
    quad: [Point; 4],
    bili: BiliPrecomp,
    bb: BoundingBox,
}

impl<'a> QuadSampler<'a> {
    fn new(
        bitmaps: &'a [Bitmap],
        bmp_id: BitmapHandle,
        quad: [Point; 4],
    ) -> Option<Self> {
        let bitmap = bitmaps.get(bmp_id.0)?;
        let mut bb = BoundingBox::default();
        for point in quad { bb.adopt(point); }

        let bili = BiliPrecomp::new(quad);

        Some(Self {
            bitmap,
            quad,
            bili,
            bb,
        })
    }
}

impl Sampler for QuadSampler<'_> {
    #[inline(always)]
    fn sample(&self, point: Point) -> Color16 {
        if !self.bb.contains(point) {
            return TRANSPARENT;
        }

        if !point_in_quad(point, self.quad) {
            return TRANSPARENT;
        }

        let Some(uv) = self.bili.inverse_bilinear(point) else {
            return TRANSPARENT;
        };

        let w = self.bitmap.size.x as f32;
        let h = self.bitmap.size.y as f32;
        self.bitmap.sample(Point::new(uv.x * w, uv.y * h))
    }
}

//////////////////////////////////////////////////////////

pub enum SamplerEnum<'a> {
    Solid(SolidSampler),
    Mask(MaskSampler<'a>),
    Rainbow(RainbowSampler),
    ScaledBitmap(ScaledSampler<'a>),
    QuadBitmap(QuadSampler<'a>),
}

impl Texture<'_> {
    pub(super) fn sampler<'a>(&'a self, bitmaps: &'a [Bitmap], masks: &'a [MaskImage]) -> SamplerEnum<'a> {
        let rainbow = SamplerEnum::Rainbow(RainbowSampler);

        match self {
            Texture::SolidColor(color) => SamplerEnum::Solid(SolidSampler(Color16::from(*color))),
            Texture::Gradient(_slice) => todo!(),
            Texture::Debug => rainbow,

            Texture::Mask { top_left, scale, repeat, mask, zero, full } => {
                let sampler = MaskSampler::new(masks, *mask, *top_left, *repeat, *scale, *zero, *full);
                sampler.map(SamplerEnum::Mask).unwrap_or(rainbow)
            },

            Texture::Bitmap { top_left, scale, repeat, bitmap } => {
                let sampler = ScaledSampler::new(bitmaps, *bitmap, *top_left, *repeat, *scale);
                sampler.map(SamplerEnum::ScaledBitmap).unwrap_or(rainbow)
            },

            Texture::QuadBitmap { top_left, btm_left, top_right, btm_right, bitmap } => {
                let quad = [*top_left, *top_right, *btm_right, *btm_left];
                let sampler = QuadSampler::new(bitmaps, *bitmap, quad);
                sampler.map(SamplerEnum::QuadBitmap).unwrap_or(rainbow)
            },
        }
    }
}

//////////////////////////////////////////////////////////

// https://www.reedbeta.com/blog/quadrilateral-interpolation-part-2/
// https://iquilezles.org/articles/ibilinear/
// https://www.gamedev.net/forums/topic/596392-uv-coordinate-on-a-2d-quadrilateral/

fn wedge(a: Point, b: Point) -> f32 {
    a.x * b.y - a.y * b.x
}

#[derive(Copy, Clone, Debug)]
struct BiliPrecomp {
    tl: Point,
    e: Vec2<f32>,
    f: Vec2<f32>,
    g: Vec2<f32>,
    k2: f32,
    ik2: f32,
    k2_4: f32,
    k1a: f32,
}

impl BiliPrecomp {
    fn new(quad: [Point; 4]) -> Self {
        let [tl, tr, br, bl] = quad;

        let e = tr - tl;
        let f = bl - tl;
        let g = tl - tr + br - bl;

        let k2 = wedge(g, f);
        let ik2 = 0.5 / k2;
        let k2_4 = k2 * 4.0;
        let k1a = wedge(e, f);

        Self { tl, e, f, g, k2, ik2, k2_4, k1a }
    }

    fn inverse_bilinear(self, point: Point) -> Option<Point> {
        let h = point - self.tl;

        let k1 = self.k1a + wedge(h, self.g);
        let k0 = wedge(h, self.e);

        if self.k2.abs() < 0.001 {
            // if edges are parallel, this is a linear equation
            let u = (h.x * k1 + self.f.x * k0) / (self.e.x * k1 - self.g.x * k0);
            let v = -k0 / k1;
            return Some(Point::new(u, v));
        }

        // otherwise, it's a quadratic
        let d = k1 * k1 - k0 * self.k2_4;

        if d < 0.0 {
            return None;
        }

        let w = f32::sqrt(d);

        let mut v = (-k1 - w) * self.ik2;
        let mut u = (h.x - self.f.x * v) / (self.e.x + self.g.x * v);

        if u < 0.0 || u > 1.0 || v < 0.0 || v > 1.0 {
            v = (-k1 + w) * self.ik2;
            u = (h.x - self.f.x * v) / (self.e.x + self.g.x * v);
        }

        Some(Point::new(u, v))
    }
}


#[inline(always)]
fn point_in_quad(point: Point, quad: [Point; 4]) -> bool {
    let [a, b, c, d] = quad;
    let b1 = toggle_in_shape(point, a, b);
    let b2 = toggle_in_shape(point, b, c);
    let b3 = toggle_in_shape(point, c, d);
    let b4 = toggle_in_shape(point, d, a);
    (b1 ^ b2) ^ (b3 ^ b4)
}

// Computes a one bit winding number increment/decrement
#[inline(always)]
fn toggle_in_shape(p: Point, s: Point, e: Point) -> bool {
    let v1 = p - s;
    let v2 = e - s;

    let b1 = s.y <= p.y;
    let b2 = e.y > p.y;
    let b3 = (v1.x * v2.y) > (v1.y * v2.x);

    let dec = ( b1) & ( b2) & ( b3);
    let inc = (!b1) & (!b2) & (!b3);

    inc != dec
}
