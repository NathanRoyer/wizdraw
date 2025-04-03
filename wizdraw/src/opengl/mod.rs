pub use super::*;
use rgb::ComponentMap;
use core::array::from_fn;
use core::mem::{swap, take};

use glow::{Context, HasContext, PixelUnpackData, PixelPackData};
use glow::{NativeShader, NativeTexture, NativeProgram, Framebuffer};

use glow::{
    VERTEX_SHADER, FRAGMENT_SHADER, TEXTURE_2D, TEXTURE_MAG_FILTER, TEXTURE_MIN_FILTER,
    NEAREST, RGBA, FRAMEBUFFER, BLEND, SRC_ALPHA, ONE_MINUS_SRC_ALPHA, DEPTH_TEST, FLOAT,
    ARRAY_BUFFER, DYNAMIC_DRAW, RENDERBUFFER, RGB5_A1, COLOR_ATTACHMENT0, LINK_STATUS,
    FRAMEBUFFER_COMPLETE, COLOR_BUFFER_BIT, TRIANGLE_STRIP, TEXTURE0, TEXTURE1,
};

use glow::UNSIGNED_SHORT_5_5_5_1 as RGBA5551;

// todo
// mod drm_kms;

pub struct TexTile {
    offset: Vec2<usize>,
    tex_id: NativeTexture,
}

pub struct TexData {
    tiles: Box<[TexTile]>,
    size: Vec2<i32>,
}

pub struct Es2Canvas {
    gl: Context,
    mask_program: NativeProgram,
    color_program: NativeProgram,
    mask_src: NativeTexture,
    mask_dst: NativeTexture,
    render_fb: Framebuffer,
    mask_fb: Framebuffer,
    fb_size: Vec2<i32>,
    tex_buf: Box<[u8]>,
    textures: Vec<TexData>,
}

impl Canvas for Es2Canvas {
    fn framebuffer_size(&self) -> Vec2<usize> {
        self.fb_size.map(|n| n as _)
    }

    fn alloc_bitmap(&mut self, width: usize, height: usize) -> BitmapHandle {
        let index = self.textures.len();
        let mut tiles = Vec::new();

        for y in (0..width).step_by(256) {
            for x in (0..height).step_by(256) {
                let Ok(tex_id) = (unsafe { init_texture(&self.gl) }) else {
                    // error!
                    return BitmapHandle(usize::MAX);
                };

                let tile = TexTile {
                    offset: Vec2::new(x, y),
                    tex_id,
                };

                tiles.push(tile);
            }
        }

        let tex_data = TexData {
            tiles: tiles.into(),
            size: Vec2::new(width as i32, height as i32),
        };

        self.textures.push(tex_data);
        BitmapHandle(index)
    }

    fn fill_bitmap(&mut self, bitmap: BitmapHandle, x: usize, y: usize, w: usize, h: usize, buf: &[Color]) {
        let tiles = &self.textures[bitmap.0].tiles;

        let max_x = x + w;
        let max_y = y + h;

        for tile in tiles {
            let tile_max_x = tile.offset.x + 256;
            let tile_max_y = tile.offset.y + 256;

            let x_overlap = (x < tile_max_x) & (max_x >= tile.offset.x);
            let y_overlap = (y < tile_max_y) & (max_y >= tile.offset.y);

            if !(x_overlap & y_overlap) {
                continue;
            }

            let x_full_coverage = (x <= tile.offset.x) & (max_x >= tile_max_x);
            let y_full_coverage = (y <= tile.offset.y) & (max_y >= tile_max_y);

            if !(x_full_coverage & y_full_coverage) {
                unsafe {
                    let dst = PixelPackData::Slice(Some(&mut self.tex_buf));
                    self.gl.active_texture(TEXTURE0);
                    self.gl.bind_texture(TEXTURE_2D, Some(tile.tex_id));
                    self.gl.get_tex_image(TEXTURE_2D, 0, RGBA, RGBA5551, dst);
                    let _ = self.gl.get_error();
                }
            }

            let x_start = x.max(tile.offset.x);
            let y_start = y.max(tile.offset.y);
            let x_stop = max_x.min(tile_max_x);
            let y_stop = max_y.min(tile_max_y);

            for tex_y in y_start..y_stop {
                for tex_x in x_start..x_stop {
                    let (dst_x, dst_y) = (tex_x - tile.offset.x, tex_y - tile.offset.y);
                    let (src_x, src_y) = (tex_x - x, tex_y - y);
                    let src_color = buf[src_y * w + src_x];
                    let [rg, gba] = into_rgba5551(src_color);
                    let dst_index = (dst_y * 256 + dst_x) * 2;
                    self.tex_buf[dst_index + 0] = rg;
                    self.tex_buf[dst_index + 1] = gba;
                }
            }

            unsafe {
                let src = PixelUnpackData::Slice(Some(&self.tex_buf));
                self.gl.tex_image_2d(TEXTURE_2D, 0, RGBA as i32, 256, 256, 0, RGBA, RGBA5551, src);
                debug(&self.gl, "tex_image_2d");
            }
        }
    }

    fn free_bitmap(&mut self, bitmap: BitmapHandle) {
        let tiles = take(&mut self.textures[bitmap.0].tiles);
        for tex_tile in tiles {
            unsafe { self.gl.delete_texture(tex_tile.tex_id) };
        }
    }

    fn clear(&mut self) {
        unsafe {
            self.gl.bind_framebuffer(FRAMEBUFFER, Some(self.render_fb));
            self.gl.viewport(0, 0, self.fb_size.x, self.fb_size.y);
            self.gl.clear_color(0.0, 0.0, 0.0, 1.0);
            self.gl.clear(COLOR_BUFFER_BIT);
            debug(&self.gl, "clear");
        }
    }

    fn fill_cbc(&mut self, path: &[CubicBezier], texture: &Texture, _ssaa: SsaaConfig) {
        let mut shape_aabb = BoundingBox::default();

        for curve in path {
            shape_aabb = shape_aabb.union(curve.aabb());
        }

        for y in (0..self.fb_size.y).step_by(256) {
            for x in (0..self.fb_size.x).step_by(256) {
                let (x_f32, y_f32) = (x as f32, y as f32);
                let (tile_max_x, tile_max_y) = (x_f32 + 256.0, y_f32 + 256.0);

                let x_overlap = (shape_aabb.min.x < tile_max_x) & (shape_aabb.max.x >= x_f32);
                let y_overlap = (shape_aabb.min.y < tile_max_y) & (shape_aabb.max.y >= y_f32);

                if !(x_overlap & y_overlap) {
                    continue;
                }

                self.tile_pass(path, x, y, texture);
            }
        }

        unsafe {
            self.gl.flush();
            debug(&self.gl, "flush");
        }
    }
}

fn into_rgba5551(color: Color) -> [u8; 2] {
    let color = color.map(u16::from);
    let mut word = 0u16;
    word |= (color.r & 0xf8) << 11;
    word |= (color.g & 0xf8) << 6;
    word |= (color.b & 0xf8) << 1;
    word |= color.a >> 7;
    word.to_le_bytes()
}

unsafe fn init_shader(gl: &Context, shader_type: u32, src: &str) -> Result<NativeShader, String> {
    let shader = gl.create_shader(shader_type)?;
    gl.shader_source(shader, src);
    gl.compile_shader(shader);
    if gl.get_shader_compile_status(shader) {
        Ok(shader)
    } else {
        let errors = gl.get_shader_info_log(shader);
        Err(format!("Compilation Failed: {}", errors))
    }
}

unsafe fn init_texture(gl: &Context) -> Result<NativeTexture, String> {
    let tex = gl.create_texture()?;
    gl.active_texture(TEXTURE0);
    gl.bind_texture(TEXTURE_2D, Some(tex));
    gl.tex_parameter_i32(TEXTURE_2D, TEXTURE_MIN_FILTER, NEAREST as i32);
    gl.tex_parameter_i32(TEXTURE_2D, TEXTURE_MAG_FILTER, NEAREST as i32);

    let side = 256;
    let level = 0;
    let format = RGBA;
    let border = 0;
    let data = PixelUnpackData::Slice(None);

    gl.tex_image_2d(
        TEXTURE_2D,
        level,
        format as i32,
        side,
        side,
        border,
        format,
        RGBA5551,
        data,
    );

    Ok(tex)
}

unsafe fn init_program(gl: &Context, v_shader: &str, f_shader: &str) -> Result<NativeProgram, String> {
    let program = gl.create_program()?;
    let vertex_shader = init_shader(&gl, VERTEX_SHADER, v_shader)?;
    let fragment_shader = init_shader(&gl, FRAGMENT_SHADER, f_shader)?;
    gl.attach_shader(program, vertex_shader);
    gl.attach_shader(program, fragment_shader);

    gl.link_program(program);
    gl.validate_program(program);
    let warning = gl.get_program_info_log(program);
    if !warning.is_empty() {
        return Err(format!("Compilation Warning: {}", warning));
    }

    if gl.get_program_parameter_i32(program, LINK_STATUS) == 0 {
        gl.delete_program(program);
        return Err(format!("Linkage Error"));
    }

    Ok(program)
}

impl Es2Canvas {
    pub fn init(gl: Context, width: i32, height: i32) -> Result<Self, String> {
        let fb_size = Vec2::new(width, height);

        unsafe {
            let v_shader = include_str!("mask-vertex-shader.glsl");
            let f_shader = include_str!("mask-fragment-shader.glsl");
            let mask_program = init_program(&gl, v_shader, f_shader)?;

            let v_shader = include_str!("color-vertex-shader.glsl");
            let f_shader = include_str!("color-fragment-shader.glsl");
            let color_program = init_program(&gl, v_shader, f_shader)?;

            gl.use_program(Some(color_program));
            let loc = gl.get_uniform_location(color_program, "opacity");
            gl.uniform_1_i32(loc.as_ref(), 0);
            debug(&gl, "opacity texture");

            let loc = gl.get_uniform_location(color_program, "bmp_tile");
            gl.uniform_1_i32(loc.as_ref(), 1);
            debug(&gl, "bmp_tile texture");

            let Some(mask_attr) = gl.get_attrib_location(mask_program, "a_position") else {
                return Err("Failed to locate a_position attribute".into());
            };

            let Some(color_attr) = gl.get_attrib_location(color_program, "a_position") else {
                return Err("Failed to locate a_position attribute".into());
            };

            let position_buffer = gl.create_buffer()?;
            gl.bind_buffer(ARRAY_BUFFER, Some(position_buffer));
            gl.enable_vertex_attrib_array(mask_attr);
            gl.enable_vertex_attrib_array(color_attr);

            let (normalize, stride, offset) = (false, 0, 0);
            gl.vertex_attrib_pointer_f32(mask_attr, 2, FLOAT, normalize, stride, offset);
            gl.vertex_attrib_pointer_f32(color_attr, 2, FLOAT, normalize, stride, offset);

            // a square covering full viewport
            let pos: [f32; 8] = [ -1.0, -1.0, -1.0, 1.0, 1.0, -1.0, 1.0, 1.0 ];
            let pos_u8: [u8; 8 * 4] = from_fn(|i| pos[i / 4].to_ne_bytes()[i % 4]);
            gl.buffer_data_u8_slice(ARRAY_BUFFER, &pos_u8, DYNAMIC_DRAW);

            let mask_src = init_texture(&gl)?;
            let mask_dst = init_texture(&gl)?;
            let mask_fb = gl.create_framebuffer()?;

            gl.enable(BLEND);
            gl.blend_func(SRC_ALPHA, ONE_MINUS_SRC_ALPHA);

            gl.disable(DEPTH_TEST);
            gl.depth_mask(false);

            let render_fb = gl.create_framebuffer()?;
            let render_buffer = gl.create_renderbuffer()?;

            // allocate renderbuffer storage
            gl.bind_renderbuffer(RENDERBUFFER, Some(render_buffer));
            gl.renderbuffer_storage(RENDERBUFFER, RGB5_A1, fb_size.x, fb_size.y);

            // bind framebuffer and renderbuffer
            gl.bind_framebuffer(FRAMEBUFFER, Some(render_fb));
            gl.framebuffer_renderbuffer(FRAMEBUFFER, COLOR_ATTACHMENT0, RENDERBUFFER, Some(render_buffer));

            let status = gl.check_framebuffer_status(FRAMEBUFFER);
            if status != FRAMEBUFFER_COMPLETE {
                return Err(format!("Failed to create render buffer: status = {status}"));
            }

            gl.bind_framebuffer(FRAMEBUFFER, None);
            debug(&gl, "init");

            // todo: check actual renderbuffer format
            // todo: maybe try OES_rgb8_rgba8 extension

            let tex_buf = vec![0; 256 * 256 * 2].into();

            Ok(Self {
                gl,
                mask_program,
                color_program,
                mask_src,
                mask_dst,
                render_fb,
                mask_fb,
                fb_size,
                tex_buf,
                textures: Vec::new(),
            })
        }
    }

    fn tile_pass(&mut self, path: &[CubicBezier], x: i32, y: i32, texture: &Texture) {
        debug(&self.gl, "tile_pass");
        unsafe {
            self.gl.bind_framebuffer(FRAMEBUFFER, Some(self.mask_fb));
            debug(&self.gl, "bind_framebuffer");

            self.gl.use_program(Some(self.mask_program));
            debug(&self.gl, "use_program");

            let init_loc = self.gl.get_uniform_location(self.mask_program, "init");
            self.gl.uniform_1_i32(init_loc.as_ref(), 1);
            debug(&self.gl, "init");

            let loc = self.gl.get_uniform_location(self.mask_program, "height");
            self.gl.uniform_1_f32(loc.as_ref(), self.fb_size.y as f32);
            debug(&self.gl, "height");

            let loc = self.gl.get_uniform_location(self.mask_program, "offset");
            self.gl.uniform_2_f32(loc.as_ref(), x as f32, y as f32);
            debug(&self.gl, "offset");

            self.gl.viewport(0, 0, 256, 256);
            debug(&self.gl, "viewport");

            for (i, curve) in path.iter().enumerate() {
                let coords = [
                    curve.c1.x, curve.c1.y,
                    curve.c2.x, curve.c2.y,
                    curve.c3.x, curve.c3.y,
                    curve.c4.x, curve.c4.y,
                ];

                let loc = self.gl.get_uniform_location(self.mask_program, "input_curve");
                self.gl.uniform_1_f32_slice(loc.as_ref(), &coords);
                debug(&self.gl, "input_curve");

                self.gl.active_texture(TEXTURE0);
                self.gl.bind_texture(TEXTURE_2D, Some(self.mask_src));
                self.gl.framebuffer_texture_2d(FRAMEBUFFER, COLOR_ATTACHMENT0, TEXTURE_2D, Some(self.mask_dst), 0);
                debug(&self.gl, "framebuffer_texture_2d");

                // use the square from Self::init()
                let (offset, count) = (0, 4);
                self.gl.draw_arrays(TRIANGLE_STRIP, offset, count);
                debug(&self.gl, "draw_arrays");

                swap(&mut self.mask_src, &mut self.mask_dst);

                if i == 0 {
                    self.gl.uniform_1_i32(init_loc.as_ref(), 0);
                    debug(&self.gl, "init_2");
                }
            }
        }

        let (mode, param_1, param_2, bitmap) = match texture {
            Texture::SolidColor(color) => {
                let color = color.map(|c| c as f32);
                (0, color.into(), [0.0; 4], None)
            },
            Texture::Gradient(_slice) => todo!(),
            Texture::Debug => (2, [0.0; 4], [0.0; 4], None),
            Texture::Bitmap {
                top_left,
                scale,
                repeat,
                bitmap,
            } => {
                let r = *repeat as u32 as f32;
                let param_1 = [top_left.x, top_left.y, *scale, r];
                (3, param_1, [0.0; 4], Some(bitmap))
            },
            Texture::QuadBitmap {
                top_left: _top_left,
                btm_left: _btm_left,
                top_right: _top_right,
                btm_right: _btm_right,
                bitmap: _bitmap,
            } => {
                todo!()
            },
        };

        // color pass
        unsafe {
            self.gl.use_program(Some(self.color_program));
            debug(&self.gl, "[color] use_program");

            let loc = self.gl.get_uniform_location(self.color_program, "height");
            self.gl.uniform_1_f32(loc.as_ref(), self.fb_size.y as f32);
            debug(&self.gl, "[color] height");

            let loc = self.gl.get_uniform_location(self.color_program, "offset");
            self.gl.uniform_2_f32(loc.as_ref(), x as f32, y as f32);
            debug(&self.gl, "[color] offset");

            let loc = self.gl.get_uniform_location(self.color_program, "mode");
            self.gl.uniform_1_i32(loc.as_ref(), mode);
            debug(&self.gl, "[color] mode");

            let loc = self.gl.get_uniform_location(self.color_program, "param_1");
            self.gl.uniform_4_f32_slice(loc.as_ref(), &param_1);
            debug(&self.gl, "[color] param_1");

            self.gl.active_texture(TEXTURE0);
            self.gl.bind_texture(TEXTURE_2D, Some(self.mask_src));
            self.gl.bind_framebuffer(FRAMEBUFFER, Some(self.render_fb));
                debug(&self.gl, "[color] framebuffer_texture_2d");

            self.gl.viewport(x, y, 256, 256);

            if let Some(bitmap) = bitmap {
                let bitmap = &self.textures[bitmap.0];

                let loc = self.gl.get_uniform_location(self.color_program, "bmp_size");
                self.gl.uniform_2_f32(loc.as_ref(), bitmap.size.x as _, bitmap.size.y as _);
                debug(&self.gl, "[color] bmp_size");

                for tile in &bitmap.tiles {
                    self.gl.active_texture(TEXTURE1);
                    self.gl.bind_texture(TEXTURE_2D, Some(tile.tex_id));

                    let loc = self.gl.get_uniform_location(self.color_program, "bmp_tile_offset");
                    self.gl.uniform_2_f32(loc.as_ref(), tile.offset.x as _, tile.offset.y as _);
                    debug(&self.gl, "[color] bmp_tile_offset");

                    // use the square from Self::init()
                    let (offset, count) = (0, 4);
                    self.gl.draw_arrays(TRIANGLE_STRIP, offset, count);
                    debug(&self.gl, "[color] bitmap draw");
                }
            } else {
                self.gl.active_texture(TEXTURE1);
                self.gl.bind_texture(TEXTURE_2D, None);

                // use the square from Self::init()
                let (offset, count) = (0, 4);
                self.gl.draw_arrays(TRIANGLE_STRIP, offset, count);
                debug(&self.gl, "[color] basic draw");
            }
        }
    }

    pub fn read_rgba5551(&self, pixels: &mut [u8]) {
        assert_eq!((self.fb_size.x * self.fb_size.y * 2) as usize, pixels.len());
        unsafe {
            self.gl.flush();
            self.gl.finish();
            debug(&self.gl, "finish");

            self.gl.bind_framebuffer(FRAMEBUFFER, Some(self.render_fb));
            let data = PixelPackData::Slice(Some(pixels));
            self.gl.read_buffer(COLOR_ATTACHMENT0);
            debug(&self.gl, "read_buffer");

            self.gl.read_pixels(0, 0, self.fb_size.x, self.fb_size.y, RGBA, RGBA5551, data);
            debug(&self.gl, "read_pixels");
        }
    }
}

fn debug(_gl: &Context, _operation: &str) {
    #[cfg(feature = "gl-debug")]
    unsafe {
        let error = _gl.get_error();
        if error != 0 {
            std::println!("{_operation}: error={error}");
        }
    }
}
