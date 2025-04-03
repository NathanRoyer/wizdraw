pub use super::*;
use core::array::from_fn;
use core::mem::swap;

use glow::{Context, HasContext, PixelUnpackData, PixelPackData, Renderbuffer};
use glow::{NativeShader, NativeTexture, NativeProgram, Framebuffer};

use glow::{
    VERTEX_SHADER, FRAGMENT_SHADER, TEXTURE_2D, TEXTURE_MAG_FILTER, TEXTURE_MIN_FILTER,
    NEAREST, RGB, UNSIGNED_SHORT_5_6_5, FRAMEBUFFER, BLEND, SRC_ALPHA, ONE_MINUS_SRC_ALPHA,
    DEPTH_TEST, FLOAT, ARRAY_BUFFER, DYNAMIC_DRAW, RENDERBUFFER, RGB565, COLOR_ATTACHMENT0,
    FRAMEBUFFER_COMPLETE, COLOR_BUFFER_BIT, TRIANGLE_STRIP, LINK_STATUS,
};

// todo
// mod drm_kms;


pub struct Es2Canvas {
    gl: Context,
    mask_program: NativeProgram,
    color_program: NativeProgram,
    mask_src: NativeTexture,
    mask_dst: NativeTexture,
    render_fb: Framebuffer,
    mask_fb: Framebuffer,
    fb_size: Vec2<i32>,
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
    gl.bind_texture(TEXTURE_2D, Some(tex));
    gl.tex_parameter_i32(TEXTURE_2D, TEXTURE_MIN_FILTER, NEAREST as i32);
    gl.tex_parameter_i32(TEXTURE_2D, TEXTURE_MAG_FILTER, NEAREST as i32);

    let side = 256;
    let level = 0;
    let format = RGB;
    let border = 0;
    let tex_type = UNSIGNED_SHORT_5_6_5;
    let data = PixelUnpackData::Slice(None);

    gl.tex_image_2d(
        TEXTURE_2D,
        level,
        format as i32,
        side,
        side,
        border,
        format,
        tex_type,
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
            gl.renderbuffer_storage(RENDERBUFFER, RGB565, fb_size.x, fb_size.y);

            // bind framebuffer and renderbuffer
            gl.bind_framebuffer(FRAMEBUFFER, Some(render_fb));
            gl.framebuffer_renderbuffer(FRAMEBUFFER, COLOR_ATTACHMENT0, RENDERBUFFER, Some(render_buffer));

            let status = gl.check_framebuffer_status(FRAMEBUFFER);
            if status != FRAMEBUFFER_COMPLETE {
                return Err(format!("Failed to create render buffer: status = {status}"));
            }

            gl.bind_framebuffer(FRAMEBUFFER, None);
            // std::println!("init: error={:?}", gl.get_error());

            // todo: check actual renderbuffer format
            // todo: maybe try OES_rgb8_rgba8 extension

            Ok(Self {
                gl,
                mask_program,
                color_program,
                mask_src,
                mask_dst,
                render_fb,
                mask_fb,
                fb_size,
            })
        }
    }

    fn tile_pass(&mut self, cbc: &[CubicBezier], x: i32, y: i32) {
        unsafe {
            self.gl.bind_framebuffer(FRAMEBUFFER, Some(self.mask_fb));
            // std::println!("bind_framebuffer: error={:?}", self.gl.get_error());

            self.gl.use_program(Some(self.mask_program));
            // std::println!("use_program: error={:?}", self.gl.get_error());

            let init_loc = self.gl.get_uniform_location(self.mask_program, "init");
            self.gl.uniform_1_i32(init_loc.as_ref(), 1);
            // std::println!("init: error={:?}", self.gl.get_error());

            let loc = self.gl.get_uniform_location(self.mask_program, "height");
            self.gl.uniform_1_f32(loc.as_ref(), self.fb_size.y as f32);
            // std::println!("height: error={:?}", self.gl.get_error());

            let loc = self.gl.get_uniform_location(self.mask_program, "offset");
            self.gl.uniform_2_f32(loc.as_ref(), x as f32, y as f32);
            // std::println!("offset: error={:?}", self.gl.get_error());

            self.gl.viewport(0, 0, 256, 256);
            // std::println!("viewport: error={:?}", self.gl.get_error());

            for (i, curve) in cbc.iter().enumerate() {
                let coords = [
                    curve.c1.x, curve.c1.y,
                    curve.c2.x, curve.c2.y,
                    curve.c3.x, curve.c3.y,
                    curve.c4.x, curve.c4.y,
                ];

                let loc = self.gl.get_uniform_location(self.mask_program, "input_curve");
                self.gl.uniform_1_f32_slice(loc.as_ref(), &coords);
                // std::println!("input_curve: error={:?}", self.gl.get_error());

                self.gl.bind_texture(TEXTURE_2D, Some(self.mask_src));
                self.gl.framebuffer_texture_2d(FRAMEBUFFER, COLOR_ATTACHMENT0, TEXTURE_2D, Some(self.mask_dst), 0);
                // std::println!("framebuffer_texture_2d: error={:?}", self.gl.get_error());

                // use the square from Self::init()
                let (offset, count) = (0, 4);
                self.gl.draw_arrays(TRIANGLE_STRIP, offset, count);
                // std::println!("draw_arrays: error={:?}", self.gl.get_error());

                swap(&mut self.mask_src, &mut self.mask_dst);

                if i == 0 {
                    self.gl.uniform_1_i32(init_loc.as_ref(), 0);
                    // std::println!("init_2: error={:?}", self.gl.get_error());
                }
            }
        }

        // color pass
        unsafe {
            self.gl.use_program(Some(self.color_program));
            // std::println!("[color] use_program: error={:?}", self.gl.get_error());

            let loc = self.gl.get_uniform_location(self.color_program, "height");
            self.gl.uniform_1_f32(loc.as_ref(), self.fb_size.y as f32);
            // std::println!("[color] height: error={:?}", self.gl.get_error());

            let loc = self.gl.get_uniform_location(self.color_program, "offset");
            self.gl.uniform_2_f32(loc.as_ref(), x as f32, y as f32);
            // std::println!("[color] offset: error={:?}", self.gl.get_error());

            self.gl.bind_texture(TEXTURE_2D, Some(self.mask_src));
            self.gl.bind_framebuffer(FRAMEBUFFER, Some(self.render_fb));
                // std::println!("[color] framebuffer_texture_2d: error={:?}", self.gl.get_error());

            self.gl.viewport(x, y, 256, 256);

            // use the square from Self::init()
            let (offset, count) = (0, 4);
            self.gl.draw_arrays(TRIANGLE_STRIP, offset, count);
            // std::println!("color pass: error={:?}", self.gl.get_error());
        }
    }

    pub fn read_rgb565(&self, pixels: &mut [u8]) {
        assert_eq!((self.fb_size.x * self.fb_size.y * 2) as usize, pixels.len());
        unsafe {
            self.gl.flush();
            self.gl.finish();
            // std::println!("finish: error={:?}", self.gl.get_error());

            self.gl.bind_framebuffer(FRAMEBUFFER, Some(self.render_fb));
            let data = PixelPackData::Slice(Some(pixels));
            self.gl.read_buffer(COLOR_ATTACHMENT0);
            // std::println!("read_buffer: error={:?}", self.gl.get_error());

            self.gl.read_pixels(0, 0, self.fb_size.x, self.fb_size.y, RGB, UNSIGNED_SHORT_5_6_5, data);
            // std::println!("read_pixels: error={:?}", self.gl.get_error());
        }
    }
}

impl Canvas for Es2Canvas {
    fn framebuffer_size(&self) -> Vec2<usize> {
        self.fb_size.map(|n| n as _)
    }

    fn alloc_bitmap(&mut self, _width: usize, _height: usize) -> BitmapHandle { BitmapHandle(0) }
    fn fill_bitmap(&mut self, _bitmap: BitmapHandle, _x: usize, _y: usize, _w: usize, _h: usize, _buf: &[Color]) { }
    fn free_bitmap(&mut self, _bitmap: BitmapHandle) { }

    fn clear(&mut self) {
        unsafe {
            self.gl.bind_framebuffer(FRAMEBUFFER, Some(self.render_fb));
            self.gl.viewport(0, 0, self.fb_size.x, self.fb_size.y);
            self.gl.clear_color(0.0, 0.0, 0.0, 1.0);
            self.gl.clear(COLOR_BUFFER_BIT);
            // std::println!("clear: error={:?}", self.gl.get_error());
        }
    }

    fn fill_cbc(&mut self, cbc: &[CubicBezier], _texture: &Texture, _ssaa: SsaaConfig) {
        let mut y = 0;
        while y < self.fb_size.y {
            let mut x = 0;
            while x < self.fb_size.x {
                self.tile_pass(cbc, x, y);
                x += 256;
            }
            y += 256;
        }

        unsafe {
            self.gl.flush();
            // std::println!("flush: error={:?}", self.gl.get_error());
        }
    }
}
