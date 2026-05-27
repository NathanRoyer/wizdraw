use rgb::RGBA;
use super::*;

pub struct TileIterator {
    fb_size: Vec2<usize>,
    next: Vec2<usize>,
    tile_size: Vec2<usize>,
    ssaa: SsaaConfig,
}

impl TileIterator {
    pub fn new(fb_size: Vec2<usize>, ssaa: SsaaConfig) -> Self {
        let tile_width = 256usize >> ssaa.log2_cols::<usize>();
        let tile_height = 128usize >> ssaa.log2_rows::<usize>();

        Self {
            fb_size,
            next: Vec2::new(0, 0),
            tile_size: Vec2::new(tile_width, tile_height),
            ssaa,
        }
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
    tile_size: Vec2<usize>,
    ssaa: SsaaConfig,
}

impl Iterator for TileIterator {
    type Item = Tile;

    fn next(&mut self) -> Option<Self::Item> {
        let mut tile = None;

        if self.next.x < self.fb_size.x {
            tile = Some(self.into_tile());
            self.next.x += self.tile_size.x;
        } else {
            if self.next.y < self.fb_size.y {
                self.next.y += self.tile_size.y;
                self.next.x = 0;
                tile = Some(self.into_tile());
                self.next.x += self.tile_size.x;
            }
        }

        tile
    }
}

// rendering
impl Tile {
    pub fn render(
        &mut self,
        pixels: &mut [Color],
        fb_size: Vec2<usize>,
        mask: &Mask,
        texture: &Texture,
        bitmaps: &Bitmaps,
    ) {
        let ssaa_rows: u8 = self.ssaa.rows();
        let ssaa_cols: u8 = self.ssaa.cols();
        let offsets = self.ssaa.offsets();

        let x_min = self.origin_u.x;
        let x_max = x_min + self.tile_size.x;
        let y_min = self.origin_u.y;
        let y_max = y_min + self.tile_size.y;

        let mut subp_base = Vec2::default();
        let mut mask_base = Vec2::default();

        for y in y_min..y_max {
            let fb_line_offset = y * fb_size.x;
            subp_base.y = y as f32;

            if y >= fb_size.y {
                break;
            }

            mask_base.x = 0;

            for x in x_min..x_max {
                subp_base.x = x as f32;

                if x >= fb_size.x {
                    break;
                }

                let mut hits = 0;
                let mut cmp = 1;

                for ssaa_y in 0..ssaa_rows {
                    for ssaa_x in 0..ssaa_cols {
                        let ssaa_offset = Vec2::new(ssaa_x, ssaa_y);
                        let mask_p = mask_base + ssaa_offset;
                        let hit = mask.get(mask_p.x, mask_p.y);
                        hits = (hits << 1) | (hit as u32);
                        cmp = cmp << 1;
                    }
                }

                if hits != 0 {
                    let mut src = RGBA::<u16>::new(0, 0, 0, 0);

                    for ssaa_y in 0..ssaa_rows {
                        for ssaa_x in 0..ssaa_cols {
                            cmp = cmp >> 1;

                            if (hits & cmp) > 0 {
                                let (v_offset, h_offsets) = offsets[ssaa_y as usize];
                                let h_offset = h_offsets[ssaa_x as usize];
                                let point = subp_base + Vec2::new(h_offset, v_offset);
                                src += texture.sample(point, bitmaps);
                            }
                        }
                    }

                    // divide by num_subpx
                    src = const_ssaa!(self.ssaa, src, /);

                    let px_i = fb_line_offset + x;
                    let dst = &mut pixels[px_i];
                    *dst = blend(src, *dst);
                }

                mask_base.x += ssaa_cols;
            }

            mask_base.y += ssaa_rows;
        }
    }
}

#[inline(always)]
fn blend(src: Color16, dst: Color) -> Color {
    match src.a {
        255 => return src.map(|c| c as u8),
        0 => return dst,
        _ => (),
    };

    let dst = rgb::RGBA::new(dst.r as u16, dst.g as u16, dst.b as u16, dst.a as u16);

    let dst_a = 255u16 - src.a;
    let out_r = (src.r * src.a + dst.r * dst_a) / 255u16;
    let out_g = (src.g * src.a + dst.g * dst_a) / 255u16;
    let out_b = (src.b * src.a + dst.b * dst_a) / 255u16;
    let out_a = (src.a * src.a + dst.a * dst_a) / 255u16;

    Color::new(out_r as u8, out_g as u8, out_b as u8, out_a as u8)
}
