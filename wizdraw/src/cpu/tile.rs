use super::sampler::Sampler;
use super::*;

pub struct TileIterator<S> {
    stop: Vec2<usize>,
    next: Vec2<usize>,
    tile_size: Vec2<usize>,
    min_x: usize,
    _s: PhantomData<S>,
}

impl<S: ConstSsaa> TileIterator<S> {
    pub fn new(fb_size: Vec2<usize>, bb: BoundingBox) -> Self {
        let tile_width = 256usize >> (S::LOG2_COLS as usize);
        let tile_height = 128usize >> (S::LOG2_ROWS as usize);
        let min = bb.min.map(|c| c.floor() as usize);
        let max = bb.max.map(|c| c.ceil() as usize);
        let min = Vec2::max(Vec2::new(0, 0), min);

        let tile_size = Vec2::new(tile_width, tile_height);
        let shape_size = Vec2::new(max.x.saturating_sub(min.x), max.y.saturating_sub(min.y));
        let shape_size = Vec2::max(Vec2::new(0, 0), shape_size);

        let stop = Vec2::min(fb_size, max);
        let tile_size = Vec2::min(tile_size, shape_size);

        Self {
            stop,
            next: min,
            tile_size,
            min_x: min.x,
            _s: PhantomData,
        }
    }

    pub fn stop(&self) -> Vec2<usize> {
        self.stop
    }

    fn into_tile(&self) -> Tile<S> {
        let origin_f = self.next.map(|u| u as f32);

        Tile {
            origin_u: self.next,
            origin_f,
            tile_size: self.tile_size,
            _s: PhantomData,
        }
    }
}

pub struct Tile<S> {
    /// top left corner of the tile
    origin_u: Vec2<usize>,
    pub origin_f: Vec2<f32>,
    pub tile_size: Vec2<usize>,
    _s: PhantomData<S>,
}

impl<S: ConstSsaa> Iterator for TileIterator<S> {
    type Item = Tile<S>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut tile = None;

        if self.next.x < self.stop.x {
            tile = Some(self.into_tile());
            self.next.x += self.tile_size.x;
        } else {
            if self.next.y < self.stop.y {
                self.next.y += self.tile_size.y;
                self.next.x = self.min_x;
                tile = Some(self.into_tile());
                self.next.x += self.tile_size.x;
            }
        }

        tile
    }
}

// rendering
impl<S: ConstSsaa> Tile<S> {
    pub fn render<T: Sampler>(
        &mut self,
        pixels: &mut [Color],
        stride: usize,
        stop: Vec2<usize>,
        mask: &Mask,
        sampler: &T,
    ) {
        let x_min = self.origin_u.x;
        let y_min = self.origin_u.y;
        let x_max = usize::min(stop.x, x_min + self.tile_size.x);
        let y_max = usize::min(stop.y, y_min + self.tile_size.y);

        let mut mask_base = Vec2::<u8>::default();

        for y in y_min..y_max {
            let fb_line_offset = y * stride;
            let y_base = y as f32 + S::Y_BIAS;

            if mask.empty_rows(mask_base.y, S::ROWS) {
                mask_base.y = mask_base.y.wrapping_add(S::ROWS);
                continue;
            }

            mask_base.x = 0;

            for x in x_min..x_max {
                let x_base = x as f32 + S::X_BIAS;
                let mut hit_mask = 0;

                for ssaa_y in 0..S::ROWS {
                    let y = mask_base.y + ssaa_y;
                    let ssaa_hit = mask.get(mask_base.x, y, S::COLS);
                    hit_mask = (hit_mask << S::COLS) | ssaa_hit;
                }

                mask_base.x = mask_base.x.wrapping_add(S::COLS);

                if hit_mask != 0 {
                    let mut src = Color16::new(0, 0, 0, 0);

                    if T::FLAT {
                        src = sampler.sample(Point::default());
                    } else {
                        let mut offset = Vec2::new(x_base, y_base);

                        for _ in 0..S::ROWS {
                            for _ in 0..S::COLS {
                                src += sampler.sample(offset);
                                offset.x += S::X_STEP;
                            }

                            offset.y += S::Y_STEP;
                            offset.x = x_base;
                        }

                        // divide by num_subpx
                        src = src / (S::SUBP as u16);
                    }

                    if src.a != 0 {
                        let px_i = fb_line_offset + x;
                        let dst = &mut pixels[px_i];
                        let full_hit = hit_mask == S::MASK;

                        if full_hit && src.a == 255 {
                            *dst = src.map(|c| c as u8);
                        } else {
                            src.a *= hit_mask.count_ones() as u16;
                            src.a = src.a / (S::SUBP as u16);
                            blend(src, dst);
                        }
                    }
                }
            }

            mask_base.y = mask_base.y.wrapping_add(S::ROWS);
        }
    }
}

#[inline(always)]
fn blend(src: Color16, dst: &mut Color) {
    if src.a == 255 {
        *dst = src.map(|c| c as u8);
        return;
    }

    let dst_a = 255u16 - src.a;

    let lerp = |src_c: u16, dst_c: u8| {
        let dst_c = dst_c as u16;
        let sum = src_c * src.a + dst_c * dst_a;
        (sum / 255u16) as u8
    };

    dst.r = lerp(src.r, dst.r);
    dst.g = lerp(src.g, dst.g);
    dst.b = lerp(src.b, dst.b);
    dst.a = lerp(src.a, dst.a);
}
