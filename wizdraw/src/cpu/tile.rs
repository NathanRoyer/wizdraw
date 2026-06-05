use super::*;

pub struct TileIterator {
    stop: Vec2<usize>,
    next: Vec2<usize>,
    tile_size: Vec2<usize>,
    ssaa: SsaaConfig,
    min_x: usize,
}

impl TileIterator {
    pub fn new(fb_size: Vec2<usize>, ssaa: SsaaConfig, bb: BoundingBox) -> Self {
        let tile_width = 256usize >> ssaa.log2_cols::<usize>();
        let tile_height = 128usize >> ssaa.log2_rows::<usize>();
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
            ssaa,
            min_x: min.x,
        }
    }

    pub fn stop(&self) -> Vec2<usize> {
        self.stop
    }

    fn into_tile(&self) -> Tile {
        let origin_f = self.next.map(|u| u as f32);

        Tile {
            origin_u: self.next,
            origin_f,
            tile_size: self.tile_size,
            ssaa: self.ssaa,
        }
    }
}

pub struct Tile {
    /// top left corner of the tile
    origin_u: Vec2<usize>,
    pub origin_f: Vec2<f32>,
    pub tile_size: Vec2<usize>,
    ssaa: SsaaConfig,
}

impl Iterator for TileIterator {
    type Item = Tile;

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
impl Tile {
    // #[inline(never)]
    pub fn render(
        &mut self,
        pixels: &mut [Color],
        stride: usize,
        stop: Vec2<usize>,
        mask: &Mask,
        texture: &Texture,
        bitmaps: &Bitmaps,
    ) {
        use sampler::Sampler;

        let ssaa_rows: u8 = self.ssaa.rows();
        let ssaa_cols: u8 = self.ssaa.cols();
        let max_hits: u32 = (1 << self.ssaa.as_mul::<u32>()) - 1;
        let offsets = self.ssaa.offsets();

        let x_min = self.origin_u.x;
        let y_min = self.origin_u.y;
        let x_max = x_min + self.tile_size.x;
        let y_max = y_min + self.tile_size.y;

        let mut subp_base = Vec2::default();
        let mut mask_base = Vec2::<u8>::default();

        let sampler = texture.sampler(bitmaps);

        for y in y_min..y_max {
            let fb_line_offset = y * stride;
            subp_base.y = y as f32;

            if y >= stop.y {
                break;
            }

            if mask.empty_rows(mask_base.y, ssaa_rows) {
                mask_base.y = mask_base.y.wrapping_add(ssaa_rows);
                continue;
            }

            mask_base.x = 0;

            for x in x_min..x_max {
                subp_base.x = x as f32;

                if x >= stop.x {
                    break;
                }

                let mut hit_mask = 0;

                for ssaa_y in 0..ssaa_rows {
                    let y = mask_base.y + ssaa_y;
                    let ssaa_hit = mask.get(mask_base.x, y, ssaa_cols);
                    hit_mask = (hit_mask << ssaa_cols) | ssaa_hit;
                }

                mask_base.x = mask_base.x.wrapping_add(ssaa_cols);

                if hit_mask != 0 {
                    let mut src = Color16::new(0, 0, 0, 0);

                    if let Texture::SolidColor(color) = texture {
                        src = color.map(|c| c as u16);
                    } else {
                        for ssaa_y in 0..ssaa_rows {
                            for ssaa_x in 0..ssaa_cols {
                                let (v_offset, h_offsets) = offsets[ssaa_y as usize];
                                let h_offset = h_offsets[ssaa_x as usize];
                                let offset = Vec2::new(h_offset, v_offset);
                                src += sampler.sample(subp_base + offset);
                            }
                        }

                        // divide by num_subpx
                        src = const_ssaa!(self.ssaa, src, /);
                    }

                    if src.a != 0 {
                        let px_i = fb_line_offset + x;
                        let dst = &mut pixels[px_i];

                        if hit_mask == max_hits && src.a == 255 {
                            *dst = src.map(|c| c as u8);
                        } else {
                            src.a *= hit_mask.count_ones() as u16;
                            src.a = const_ssaa!(self.ssaa, src.a, /);
                            blend(src, dst);
                        }
                    }
                }
            }

            mask_base.y = mask_base.y.wrapping_add(ssaa_rows);
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
