pub use super::*;
use core::slice::from_raw_parts;

mod drm_kms;

pub enum Primitive {
    Rectangle {
        top_left: Point,
        size: Vec2<f32>,
    },
    Quad([Point; 4]),
}

pub enum OpenGlVersion {
    Es2x,
    Es3x,
}

/// This only uses the default framebuffer, which must have depth attachment
pub trait OpenGlContext {
    type Program;
    type Buffer;

    fn version(&self) -> OpenGlVersion;
    fn get_viewport_size(&self) -> Vec2<usize>;

    fn create_program(&mut self, vertex_shader: &str, fragment_shader: &str) -> Self::Program;
    fn delete_program(&mut self, program: Self::Program);

    fn create_uniform_buffer(&mut self, capacity: usize) -> Self::Buffer;
    fn fill_uniform_buffer(&mut self, buffer: &Self::Buffer, values: &[f32]);
    fn delete_uniform_buffer(&mut self, buffer: Self::Buffer);

    fn set_buffer_uniform(&mut self, program: &Self::Program, name: &str, buffer: &Self::Buffer);
    fn set_uint_uniform(&mut self, program: &Self::Program, name: &str, value: u32);
    fn set_float_uniform(&mut self, program: &Self::Program, name: &str, value: f32);

    fn clear(&mut self, color: bool, depth: bool);
    fn draw(&mut self, program: &Self::Program, primitive: Primitive, mask_colors: bool);
    fn swap_buffers(&mut self);
}

pub struct OpenGlCanvas<C: OpenGlContext> {
    ctx: C,
    fb_size: Vec2<usize>,
    program: C::Program,
    curves: C::Buffer,
}

impl<C: OpenGlContext> OpenGlCanvas<C> {
    pub fn new(mut gl_ctx: C) -> Self {
        let fb_size = gl_ctx.get_viewport_size();

        let (vs, fs) = match gl_ctx.version() {
            OpenGlVersion::Es2x => (include_str!("glsl/v-es2.glsl"), include_str!("glsl/f-es2.glsl")),
            OpenGlVersion::Es3x => (include_str!("glsl/v-es3.glsl"), include_str!("glsl/f-es3.glsl")),
        };

        let program = gl_ctx.create_program(vs, fs);
        let curves = gl_ctx.create_uniform_buffer(64);

        gl_ctx.set_buffer_uniform(&program, "path", &curves);
        gl_ctx.set_float_uniform(&program, "straight_threshold", 0.5);
        gl_ctx.set_float_uniform(&program, "aabb_safe_margin", 1.0);

        Self {
            fb_size,
            ctx: gl_ctx,
            program,
            curves,
        }
    }

    pub fn swap_buffers(&mut self) {
        self.ctx.swap_buffers();
    }
}

impl<C: OpenGlContext> Canvas for OpenGlCanvas<C> {
    type BitmapHandle = usize;

    fn framebuffer_size(&self) -> Vec2<usize> {
        self.fb_size
    }

    fn alloc_bitmap(&mut self, _width: usize, _height: usize) -> Self::BitmapHandle {
        0
    }

    fn fill_bitmap(&mut self, _bitmap: Self::BitmapHandle, _x: usize, _y: usize, _w: usize, _h: usize, _buf: &[Color]) {
        // todo
    }

    fn free_bitmap(&mut self, _bitmap: Self::BitmapHandle) {
        // todo
    }

    fn clear(&mut self) {
        self.ctx.clear(true, true);
    }

    fn fill_cbc(&mut self, cbc: &[CubicBezier], _texture: &Texture<Self::BitmapHandle>, holes: bool) {
        let cbc_len = cbc.len();
        if cbc_len > 64 {
            panic!("cbc too long! (todo)");
        }

        let cbc = unsafe {
            let f32_ptr = cbc.as_ptr() as *const f32;
            from_raw_parts(f32_ptr, cbc_len * 8)
        };

        self.ctx.fill_uniform_buffer(&self.curves, cbc);
        self.ctx.set_uint_uniform(&self.program, "show_holes", holes as u32);
        self.ctx.set_uint_uniform(&self.program, "path_len", cbc_len as u32);

        let primitive = Primitive::Rectangle {
            top_left: Point::new(0.0, 0.0),
            size: self.fb_size.map(|c| c as f32),
        };

        self.ctx.draw(&self.program, primitive, false);
    }
}
