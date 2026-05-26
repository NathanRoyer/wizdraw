use rgb::ComponentMap;
use core::array::from_fn;
use core::mem::{swap, take};

use glow::{Context, HasContext, PixelUnpackData, PixelPackData};
use glow::{Shader, Texture as GlTexture, Program, Framebuffer};

use glow::{
    VERTEX_SHADER, FRAGMENT_SHADER, TEXTURE_2D, TEXTURE_MAG_FILTER, TEXTURE_MIN_FILTER,
    LINEAR, RGBA, FRAMEBUFFER, BLEND, FUNC_ADD, SRC_ALPHA, ONE_MINUS_SRC_ALPHA, DEPTH_TEST,
    FLOAT, ARRAY_BUFFER, COLOR_BUFFER_BIT, RENDERBUFFER, RGBA8, COLOR_ATTACHMENT0,
    DYNAMIC_DRAW, FRAMEBUFFER_COMPLETE, TRIANGLE_STRIP, TEXTURE0, TEXTURE1,
};

use glow::UNSIGNED_SHORT_5_5_5_1 as RGBA5551;
use glow::UNSIGNED_BYTE as RGBA8888;

use super::*;

// todo
// mod drm_kms;

struct TexTile {
    offset: Vec2<usize>,
    tex_id: GlTexture,
}

struct TexData {
    tiles: Box<[TexTile]>,
    size: Vec2<i32>,
}

pub struct Canvas {
    gl: Context,
    mask_program: Program,
    color_program: Program,
    mask_src: GlTexture,
    mask_dst: GlTexture,
    render_fb: Option<Framebuffer>,
    mask_fb: Framebuffer,
    fb_size: Vec2<i32>,
    color_buf: Vec<Color>,
    textures: Vec<TexData>,
}

impl super::Canvas for Canvas {
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
        let (max_x, max_y) = (x + w, y + h);

        self.color_buf.resize(256 * 256, Color::default());

        for tile in tiles {
            let tile_max = tile.offset + Vec2::new(256, 256);

            let x_overlap = (max_x >= tile.offset.x) & (x < tile_max.x);
            let y_overlap = (max_y >= tile.offset.y) & (y < tile_max.y);

            if !(x_overlap & y_overlap) {
                continue;
            }

            let x_start = x.max(tile.offset.x);
            let y_start = y.max(tile.offset.y);
            let x_stop = max_x.min(tile_max.x);
            let y_stop = max_y.min(tile_max.y);

            let mut i = 0;
            for tex_y in y_start..y_stop {
                for tex_x in x_start..x_stop {
                    let (src_x, src_y) = (tex_x - x, tex_y - y);
                    let src_i = src_y * w + src_x;

                    self.color_buf[i] = buf[src_i];
                    i += 1;
                }
            }

            unsafe {
                self.gl.active_texture(TEXTURE0);
                self.gl.bind_texture(TEXTURE_2D, Some(tile.tex_id));

                let (x, y) = ((x_start % 256) as i32, (y_start % 256) as i32);
                let (w, h) = ((x_stop - x_start) as i32, (y_stop - y_start) as i32);

                let src = rgb::bytemuck::cast_slice(&*self.color_buf);
                let src = PixelUnpackData::Slice(Some(src));
                self.gl.tex_sub_image_2d(TEXTURE_2D, 0, x, y, w, h, RGBA, RGBA8888, src);

                debug(&self.gl, "tex_sub_image_2d");
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
            self.gl.bind_framebuffer(FRAMEBUFFER, self.render_fb);
            self.gl.viewport(0, 0, self.fb_size.x, self.fb_size.y);
            self.gl.clear_color(0.0, 0.0, 0.0, 0.0);
            self.gl.clear(COLOR_BUFFER_BIT);
            debug(&self.gl, "clear");
        }
    }

    fn draw(&mut self, shape: &dyn Shape, texture: &Texture, _ssaa: SsaaConfig) {
        let mut bb = BoundingBox::default();

        shape.draw(&mut bb);

        let height = self.fb_size.y as f32;

        for y in (0..self.fb_size.y).step_by(256) {
            for x in (0..self.fb_size.x).step_by(256) {
                let y_flipped = height - (y as f32);
                let (x_min, y_min) = (x as f32, y_flipped - 256.0);
                let (x_max, y_max) = (x_min + 256.0, y_flipped);

                let x_overlap = (bb.min.x < x_max) & (bb.max.x >= x_min);
                let y_overlap = (bb.min.y < y_max) & (bb.max.y >= y_min);

                if !(x_overlap & y_overlap) {
                    continue;
                }

                self.tile_pass(shape, x, y, texture);
            }
        }

        unsafe {
            self.gl.flush();
            debug(&self.gl, "flush");
        }
    }
}

unsafe fn init_shader(gl: &Context, shader_type: u32, src: &str) -> Result<Shader, String> {
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

unsafe fn init_texture(gl: &Context) -> Result<GlTexture, String> {
    let tex = gl.create_texture()?;
    gl.active_texture(TEXTURE0);
    gl.bind_texture(TEXTURE_2D, Some(tex));
    gl.tex_parameter_i32(TEXTURE_2D, TEXTURE_MIN_FILTER, LINEAR as i32);
    gl.tex_parameter_i32(TEXTURE_2D, TEXTURE_MAG_FILTER, LINEAR as i32);

    static TEX_BUF: [u8; 262144] = [0u8; 262144];

    let side = 256;
    let data = PixelUnpackData::Slice(Some(&TEX_BUF));

    gl.tex_image_2d(
        TEXTURE_2D,
        0,
        RGBA as i32,
        side,
        side,
        0,
        RGBA,
        RGBA8888,
        data,
    );

    debug(&gl, "init_texture");
    Ok(tex)
}

unsafe fn init_program(gl: &Context, v_shader: &str, f_shader: &str, name: &str) -> Result<Program, String> {
    let vertex_shader = init_shader(&gl, VERTEX_SHADER, v_shader)?;
    let fragment_shader = init_shader(&gl, FRAGMENT_SHADER, f_shader)?;

    let program = gl.create_program()?;
    gl.attach_shader(program, vertex_shader);
    gl.attach_shader(program, fragment_shader);
    gl.link_program(program);
    gl.validate_program(program);

    let warning = gl.get_program_info_log(program);
    if !warning.is_empty() {
        return Err(format!("Compilation Warning: {} ({name})", warning));
    }

    /*if !gl.get_program_completion_status(program) {
        return Err(format!("Incomplete Program ({name})"));
    }*/

    if !gl.get_program_validate_status(program) {
        return Err(format!("Validation Error ({name})"));
    }

    if !gl.get_program_link_status(program) {
        return Err(format!("Linkage Error ({name})"));
    }

    Ok(program)
}

const DRAW_THRESHOLD: i32 = 80;

struct PathImpl<'a> {
    canvas: &'a mut Canvas,
    input_buf: [f32; 80],
    input_len: i32,
    blank: bool,
}

impl PathImpl<'_> {
    fn extend(&mut self, encoded: [f32; 5]) {
        let start = self.input_len as usize;
        let end = start + 5;

        let dst = &mut self.input_buf[start..end];
        dst.copy_from_slice(&encoded);

        self.input_len += 5;

        if self.input_len == DRAW_THRESHOLD {
            self.draw_call();
        }
    }

    fn draw_call(&mut self) {
        unsafe {
            let buf_len = self.input_len as usize;
            let input_buf = &self.input_buf[..buf_len];

            let loc = self.canvas.gl.get_uniform_location(self.canvas.mask_program, "input_buf");
            self.canvas.gl.uniform_1_f32_slice(loc.as_ref(), input_buf);
            debug(&self.canvas.gl, "input_buf");

            let loc = self.canvas.gl.get_uniform_location(self.canvas.mask_program, "input_len");
            self.canvas.gl.uniform_1_i32(loc.as_ref(), self.input_len);
            debug(&self.canvas.gl, "input_len");

            self.canvas.gl.active_texture(TEXTURE0);
            self.canvas.gl.bind_texture(TEXTURE_2D, Some(self.canvas.mask_src));
            self.canvas.gl.framebuffer_texture_2d(FRAMEBUFFER, COLOR_ATTACHMENT0, TEXTURE_2D, Some(self.canvas.mask_dst), 0);
            debug(&self.canvas.gl, "framebuffer_texture_2d");

            // use the square from Self::init()
            let (offset, count) = (0, 4);
            self.canvas.gl.draw_arrays(TRIANGLE_STRIP, offset, count);
            debug(&self.canvas.gl, "draw_arrays");

            swap(&mut self.canvas.mask_src, &mut self.canvas.mask_dst);

            if self.blank {
                let init_loc = self.canvas.gl.get_uniform_location(self.canvas.mask_program, "blank");
                self.canvas.gl.uniform_1_i32(init_loc.as_ref(), 0);
                debug(&self.canvas.gl, "blank_0");
                self.blank = false;
            }
        }

        self.input_len = 0;
    }
}

impl PathApi for PathImpl<'_> {
    fn segment(&mut self, segment: Segment) {
        self.extend(segment.as_five_f32s());
    }

    fn arc(&mut self, arc: Arc) {
        if arc.radius <= 0.0 {
            return;
        }

        let nonzero_radius_wraps = match arc.wraps {
            true => -arc.radius,
            false => arc.radius,
        };

        let (mut min_cos, min_neg) = decode_magic(arc.min_angle_magic);
        let (mut max_cos, max_neg) = decode_magic(arc.max_angle_magic);
        if min_neg { min_cos += 4.0; }
        if max_neg { max_cos += 4.0; }

        self.extend([
            nonzero_radius_wraps, // nonzero => arc
            arc.center.x, // a
            arc.center.y, // b
            min_cos as f32, // c
            max_cos as f32, // d
        ])
    }
}

impl Canvas {
    pub fn init(gl: Context, width: i32, height: i32, offscreen: bool) -> Result<Self, String> {
        let fb_size = Vec2::new(width, height);

        unsafe {
            let v_shader = include_str!("mask-vertex-shader.glsl");
            let f_shader = include_str!("mask-fragment-shader.glsl");
            let mask_program = init_program(&gl, v_shader, f_shader, "mask")?;

            let v_shader = include_str!("color-vertex-shader.glsl");
            let f_shader = include_str!("color-fragment-shader.glsl");
            let color_program = init_program(&gl, v_shader, f_shader, "color")?;

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
            gl.blend_equation(FUNC_ADD);
            gl.blend_func(SRC_ALPHA, ONE_MINUS_SRC_ALPHA);

            gl.disable(DEPTH_TEST);
            gl.depth_mask(false);

            let mut render_fb = None;

            if offscreen {
                render_fb = Some(gl.create_framebuffer()?);
                let render_buffer = gl.create_renderbuffer()?;

                // allocate renderbuffer storage
                gl.bind_renderbuffer(RENDERBUFFER, Some(render_buffer));
                gl.renderbuffer_storage(RENDERBUFFER, RGBA8, fb_size.x, fb_size.y);
                debug(&gl, "fb storage");

                // bind framebuffer and renderbuffer
                gl.bind_framebuffer(FRAMEBUFFER, render_fb);
                gl.framebuffer_renderbuffer(FRAMEBUFFER, COLOR_ATTACHMENT0, RENDERBUFFER, Some(render_buffer));

                let status = gl.check_framebuffer_status(FRAMEBUFFER);
                if status != FRAMEBUFFER_COMPLETE {
                    return Err(format!("Failed to create render buffer: status = {status}"));
                }

                gl.bind_framebuffer(FRAMEBUFFER, None);
            }

            debug(&gl, "init");

            // todo: check actual renderbuffer format
            // todo: maybe try OES_rgb8_rgba8 extension

            let color_buf = vec![Color::new(0, 0, 0, 0); 256 * 256].into();

            Ok(Self {
                gl,
                mask_program,
                color_program,
                mask_src,
                mask_dst,
                render_fb,
                mask_fb,
                fb_size,
                color_buf,
                textures: Vec::new(),
            })
        }
    }

    fn tile_pass(&mut self, shape: &dyn Shape, x: i32, y: i32, texture: &Texture) {
        debug(&self.gl, "tile_pass");
        unsafe {
            self.gl.bind_framebuffer(FRAMEBUFFER, Some(self.mask_fb));
            debug(&self.gl, "bind_framebuffer");

            self.gl.use_program(Some(self.mask_program));
            debug(&self.gl, "use_program");

            let init_loc = self.gl.get_uniform_location(self.mask_program, "blank");
            self.gl.uniform_1_i32(init_loc.as_ref(), 1);
            debug(&self.gl, "blank_1");

            let loc = self.gl.get_uniform_location(self.mask_program, "height");
            self.gl.uniform_1_f32(loc.as_ref(), self.fb_size.y as f32);
            debug(&self.gl, "height");

            let loc = self.gl.get_uniform_location(self.mask_program, "offset");
            self.gl.uniform_2_f32(loc.as_ref(), x as f32, y as f32);
            debug(&self.gl, "offset");

            self.gl.viewport(0, 0, 256, 256);
            debug(&self.gl, "viewport");

            let mut path_impl = PathImpl {
                canvas: self,
                input_buf: [0.0; 80],
                input_len: 0,
                blank: true,
            };

            shape.draw(&mut path_impl);

            if path_impl.input_len > 0 {
                path_impl.draw_call();
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
                top_left,
                btm_left,
                top_right,
                btm_right,
                bitmap,
            } => {
                let param_1 = [top_left.x, top_left.y, btm_left.x, btm_left.y];
                let param_2 = [top_right.x, top_right.y, btm_right.x, btm_right.y];
                (4, param_1, param_2, Some(bitmap))
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

            let loc = self.gl.get_uniform_location(self.color_program, "param_2");
            self.gl.uniform_4_f32_slice(loc.as_ref(), &param_2);
            debug(&self.gl, "[color] param_2");

            self.gl.active_texture(TEXTURE0);
            self.gl.bind_texture(TEXTURE_2D, Some(self.mask_src));
            self.gl.bind_framebuffer(FRAMEBUFFER, self.render_fb);
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

    fn read_fb(&self, pixels: &mut [u8], fmt: u32) {
        unsafe {
            self.gl.flush();
            self.gl.finish();
            debug(&self.gl, "finish");

            self.gl.bind_framebuffer(FRAMEBUFFER, self.render_fb);
            let data = PixelPackData::Slice(Some(pixels));
            self.gl.read_buffer(COLOR_ATTACHMENT0);
            debug(&self.gl, "read_buffer");

            self.gl.read_pixels(0, 0, self.fb_size.x, self.fb_size.y, RGBA, fmt, data);
            debug(&self.gl, "read_pixels");
        }
    }

    pub fn read_rgba5551(&self, pixels: &mut [u8]) {
        assert_eq!((self.fb_size.x * self.fb_size.y * 2) as usize, pixels.len());
        self.read_fb(pixels, RGBA5551);
    }

    /// Read all pixels of the framebuffer as `RGBA8888`
    pub fn read(&self, pixels: &mut [Color]) {
        assert_eq!((self.fb_size.x * self.fb_size.y) as usize, pixels.len());
        let pixels = rgb::bytemuck::cast_slice_mut(pixels);
        self.read_fb(pixels, RGBA8888);
    }

    /// Read all pixels of the framebuffer and apply a vertical flip
    pub fn read_and_flip(&mut self, pixels: &mut [Color]) {
        self.read(pixels);

        // vertical flip
        let w = self.fb_size.x as usize;
        let h = self.fb_size.y as usize;
        self.color_buf.resize(w, Color::default());
        for src_y in 0..(h / 2) {
            let dst_y = h - src_y - 1;
            let dst_s = dst_y * w;
            let src_s = src_y * w;
            let src_e = src_s + w;
            self.color_buf.copy_from_slice(&pixels[dst_s..][..w]);
            pixels.copy_within(src_s..src_e, dst_s);
            pixels[src_s..src_e].copy_from_slice(&self.color_buf);
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
