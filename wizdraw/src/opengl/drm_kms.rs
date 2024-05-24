use super::*;

use std::{fs::{OpenOptions, File}, os::fd::{AsFd, BorrowedFd}, path::Path};
use gbm::{Device as GbmDevice, Surface, BufferObjectFlags, Format, AsRaw};
use drm::control::connector::{Handle as Connector, Interface, State};
use drm::{Device as _, control::crtc::Handle as Crtc};
use drm::control::{Device as _, framebuffer::Handle as Framebuffer, Mode};
use khronos_egl as egl;
use glow::{HasContext, Context};

pub struct GpuFile(File);

impl AsFd for GpuFile {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}

impl GpuFile {
    pub fn open<P: AsRef<Path>>(card_path: P) -> std::io::Result<Self> {
        Ok(Self(OpenOptions::new().read(true).write(true).open(card_path)?))
    }
}

impl drm::Device for GpuFile {}
impl drm::control::Device for GpuFile {}

pub struct ConnectorInfo {
    pub handle: Connector,
    pub interface: Interface,
    pub modes: Vec<Mode>,
    pub connected: bool,
}

pub struct Output {
    connector: Connector,
    fb_position: Vec2<usize>,
    mode: Mode,
}

#[test]
fn yo_bruv() {
    assert!(DrmKms::new("/dev/dri/card1").and_then(|mut c| c.test()).is_some());
}

static EGL: egl::Instance<egl::Static> = egl::Instance::new(egl::Static);

pub struct DrmKms {
    device: GbmDevice<GpuFile>,
    gl_ctx: Context,
    window: Option<Surface<Framebuffer>>,
    framebuffers: Vec<Framebuffer>,
    layout: Vec<(Output, Crtc)>,
    egl_config: egl::Config,
}

impl DrmKms {
    pub fn new<P: AsRef<Path>>(card_path: P) -> Option<Self> {
        let device = GbmDevice::new(GpuFile::open(card_path).ok()?).ok()?;

        let egl_display = unsafe { EGL.get_display(device.as_raw() as egl::NativeDisplayType)? };
        EGL.initialize(egl_display).ok()?;
        EGL.bind_api(egl::OPENGL_ES_API).ok()?;

        let config_attributes = [
            egl::RENDERABLE_TYPE, egl::OPENGL_ES2_BIT,
            egl::RED_SIZE,        8,
            egl::GREEN_SIZE,      8,
            egl::BLUE_SIZE,       8,
            egl::ALPHA_SIZE,      8,
            egl::NONE,
        ];

        let egl_config = EGL.choose_first_config(egl_display, &config_attributes).ok()??;

        let context_attributes = [
            egl::CONTEXT_MAJOR_VERSION, 2,
            egl::CONTEXT_MINOR_VERSION, 0,
            egl::NONE,
        ];

        let egl_ctx = EGL.create_context(egl_display, egl_config, None, &context_attributes).ok()?;
        EGL.make_current(egl_display, None, None, Some(egl_ctx)).ok()?;

        let lkp = |proc: &str| EGL.get_proc_address(proc).unwrap() as *const _;
        let gl_ctx = unsafe { Context::from_loader_function(lkp) };

        Some(Self {
            device,
            gl_ctx,
            window: None,
            egl_config,
            framebuffers: Vec::new(),
            layout: Vec::new(),
        })
    }

    fn reset_fb(&mut self, size: Vec2<usize>) -> Option<()> {
        let egl_ctx = EGL.get_current_context().unwrap();
        let egl_display = EGL.get_current_display().unwrap();

        if let Some(window) = self.window.take() {
            // GBM surface is destroyed automatically on drop (I think?)
            // because GBM wraps it in a PtrDrop object which implements Drop
            core::mem::drop(window);

            // now that the window has been dropped, we can free the old framebuffers
            while let Some(fb) = self.framebuffers.pop() {
                self.device.destroy_framebuffer(fb).ok()?;
            }

            // as well as the EGL surface
            let egl_surface = EGL.get_current_surface(egl::DRAW).unwrap();
            EGL.make_current(egl_display, None, None, Some(egl_ctx)).ok()?;
            EGL.destroy_surface(egl_display, egl_surface).ok()?;
        }

        let (w, h) = (size.x as _, size.y as _);
        let flags = BufferObjectFlags::SCANOUT | BufferObjectFlags::RENDERING;
        let window = self.device.create_surface::<Framebuffer>(w, h, Format::Argb8888, flags).ok()?;
        let window_surf_raw = window.as_raw() as _;

        let win_attrib = [egl::ATTRIB_NONE];
        let surf = unsafe { EGL.create_platform_window_surface(egl_display, self.egl_config, window_surf_raw, &win_attrib) };
        let surf = Some(surf.ok()?);
        EGL.make_current(egl_display, surf, surf, Some(egl_ctx)).ok()?;
        EGL.swap_interval(egl_display, 2).unwrap();

        let rdr = EGL.query_context(egl_display, egl_ctx, egl::RENDER_BUFFER).unwrap();
        println!("rendering to back buffer: {:?}", rdr == egl::BACK_BUFFER);

        self.window = Some(window);

        Some(())
    }

    pub fn swap_buffers(&mut self) -> Option<()> {
        if let Some(window) = &self.window {
            let egl_display = EGL.get_current_display().unwrap();
            let egl_surface = EGL.get_current_surface(egl::DRAW).unwrap();
            EGL.swap_buffers(egl_display, egl_surface).unwrap();

            let mut buffer = unsafe { window.lock_front_buffer().unwrap() };

            let fb = if let Some(fb) = buffer.userdata().unwrap() {
                *fb
            } else {
                let fb = self.device.add_framebuffer(&buffer, 24, 32).unwrap();
                buffer.set_userdata(fb).unwrap();
                self.framebuffers.push(fb);
                fb
            };

            let target = drm::VblankWaitTarget::Relative(1);
            let flags = drm::VblankWaitFlags::empty();
            if self.device.wait_vblank(target, flags, 0, 0).ok().is_none() {
                println!("failed to wait for vblank");
            }

            for (output, crtc) in &self.layout {
                let flags = drm::control::PageFlipFlags::empty();
                if let Err(err) = self.device.page_flip(*crtc, fb, flags, None) {
                    if err.raw_os_error().unwrap() == 16 /* Resource Busy */ {

                        // need to initialize CRTC
                        let pos = (output.fb_position.x as _, output.fb_position.y as _);
                        self.device.set_crtc(*crtc, Some(fb), pos, &[output.connector], Some(output.mode)).unwrap();
                        println!("CRTC reset");

                    } else  {
                        println!("Couldn't page-flip: {:?}", err);
                    }
                }
            }
        } else {
            println!("NO WINDOW!");
        }

        Some(())
    }

    pub fn list_connectors(&self) -> Option<Vec<ConnectorInfo>> {
        let res_handles = self.device.resource_handles().ok()?;
        let mut connectors = Vec::with_capacity(res_handles.connectors.len());

        for handle in res_handles.connectors() {
            let info = self.device.get_connector(*handle, false).ok()?;

            connectors.push(ConnectorInfo {
                handle: *handle,
                interface: info.interface(),
                modes: info.modes().to_vec(),
                connected: info.state() != State::Disconnected,
            });
        }

        Some(connectors)
    }

    pub fn set_layout(&mut self, fb_size: Vec2<usize>, mut outputs: Vec<Output>) -> Option<()> {
        let res_handles = self.device.resource_handles().ok()?;

        // assign encoders

        let mut encoders = Vec::new();
        let conn = |handle| self.device.get_connector(handle, false).ok();
        let conn_current_enc = |handle| conn(handle).and_then(|c| c.current_encoder());
        let conn_enc = |handle| conn(handle).map(|c| c.encoders().to_vec());

        // sort connectors based on number of encoders that they support
        outputs.sort_by(|a, b| {
            let a = conn_enc(a.connector).unwrap().len();
            let b = conn_enc(b.connector).unwrap().len();
            a.cmp(&b)
        });

        let mut i = 0;
        while let Some(output) = outputs.get(i) {
            // if one is already set up, use it
            if let Some(encoder) = conn_current_enc(output.connector) {
                if !encoders.iter().any(|(_o, e)| *e == encoder) {
                    encoders.push((outputs.remove(i), encoder));
                    println!("re-using an encoder");
                    continue;
                }
            }

            i += 1;
        }

        'outer: for output in outputs {
            for encoder in conn_enc(output.connector)? {
                if !encoders.iter().any(|(_o, e)| *e == encoder) {
                    encoders.push((output, encoder));
                    continue 'outer;
                }
            }

            return None;
        }

        // assign CRTCs

        self.layout.clear();

        let enc = |handle| self.device.get_encoder(handle).ok();
        let enc_current_enc = |handle| enc(handle).and_then(|e| e.crtc());
        let enc_crtc = |handle| Some(res_handles.filter_crtcs(enc(handle)?.possible_crtcs()));

        // sort encoders based on number of CRTCs that they support
        encoders.sort_by(|(_o1, a), (_o2, b)| {
            let a = enc_crtc(*a).unwrap().len();
            let b = enc_crtc(*b).unwrap().len();
            a.cmp(&b)
        });

        let mut i = 0;
        while let Some((_output, enc)) = encoders.get(i) {
            // if one is already set up, use it
            if let Some(crtc) = enc_current_enc(*enc) {
                if !self.layout.iter().any(|(_o, c)| *c == crtc) {
                    let (output, _enc) = encoders.remove(i);
                    self.layout.push((output, crtc));
                    println!("re-using a CRTC");
                    continue;
                }
            }

            i += 1;
        }

        'outer: for (output, enc) in encoders {
            for crtc in enc_crtc(enc)? {
                // could re-use CRTCs that have the same position
                if !self.layout.iter().any(|(_o, c)| *c == crtc) {
                    self.layout.push((output, crtc));
                    continue 'outer;
                }
            }

            return None;
        }

        // reset CRTCs

        for crtc in res_handles.crtcs() {
            self.device.set_crtc(*crtc, None, (0, 0), &[], None).ok()?;
        }

        self.reset_fb(fb_size)
    }

    pub fn auto_assign_output(&mut self) -> Option<Vec2<usize>> {
        let mut outputs = Vec::new();
        let mut fb_size = Vec2::new(0, 0);

        for connector in self.list_connectors()? {
            if connector.connected {
                for mode in &connector.modes {
                    println!("available mode: {:?}", mode);
                }
                let mode = connector.modes[0];
                let size = Vec2::<u16>::from(mode.size());

                fb_size.x = fb_size.x.max(size.x as _);
                fb_size.y = fb_size.y.max(size.y as _);

                println!("Going to use connector: {:?}", connector.interface);
                println!("In mode: {:?}", mode);

                outputs.push(Output {
                    connector: connector.handle,
                    fb_position: Vec2::new(0, 0),
                    mode,
                });
            }
        }

        println!("fb_size: {:?}", fb_size);

        self.set_layout(fb_size, outputs)?;

        Some(fb_size)
    }

    pub fn test(&mut self) -> Option<()> {
        println!("GLES: {:?}", self.gl_ctx.version());
        let fb_size = self.auto_assign_output()?;

        let program;

        unsafe {
            let vshader = self.gl_ctx.create_shader(glow::VERTEX_SHADER).ok()?;
            let fshader = self.gl_ctx.create_shader(glow::FRAGMENT_SHADER).ok()?;

            self.gl_ctx.shader_source(vshader, include_str!("glsl/v-es2.glsl"));
            self.gl_ctx.shader_source(fshader, include_str!("glsl/f-es2.glsl"));

            self.gl_ctx.compile_shader(vshader);
            self.gl_ctx.compile_shader(fshader);

            assert!(self.gl_ctx.get_shader_compile_status(vshader));
            assert!(self.gl_ctx.get_shader_compile_status(fshader));

            program = self.gl_ctx.create_program().ok()?;
            self.gl_ctx.attach_shader(program, vshader);
            self.gl_ctx.attach_shader(program, fshader);
            self.gl_ctx.link_program(program);

            assert!(self.gl_ctx.get_program_link_status(program));

            let pos_loc = self.gl_ctx.get_attrib_location(program, "a_position")?;
            let pos_buf = self.gl_ctx.create_buffer().ok()?;
            self.gl_ctx.bind_buffer(glow::ARRAY_BUFFER, Some(pos_buf));

            let p1 = Point::new(-1.0, -1.0);
            let p2 = Point::new( 1.0,  1.0);

            let positions = [
                p1.x, p1.y,
                p1.x, p2.y,
                p2.x, p1.y,
                p2.x, p2.y,
            ];

            let u8_ptr = positions.as_ptr() as *const u8;
            let pos_u8 = core::slice::from_raw_parts(u8_ptr, positions.len() * 4);

            self.gl_ctx.buffer_data_u8_slice(glow::ARRAY_BUFFER, pos_u8, glow::DYNAMIC_DRAW);

            let vertex_array = self.gl_ctx.create_vertex_array().ok()?;
            self.gl_ctx.bind_vertex_array(Some(vertex_array));
            self.gl_ctx.enable_vertex_attrib_array(pos_loc);

            let (normalize, stride, offset) = (false, 0, 0);
            self.gl_ctx.vertex_attrib_pointer_f32(pos_loc, 2, glow::FLOAT, normalize, stride, offset);

            self.gl_ctx.use_program(Some(program));

            let set = |name, value| {
                let loc = self.gl_ctx.get_uniform_location(program, name)?;
                Some(self.gl_ctx.uniform_1_f32(Some(&loc), value))
            };

            set("straight_threshold", 0.5)?;
            set("aabb_safe_margin", 1.0)?;

            let loc = self.gl_ctx.get_uniform_location(program, "path_len")?;
            self.gl_ctx.uniform_1_i32(Some(&loc), 2);

            self.gl_ctx.viewport(0, 0, fb_size.x as _, fb_size.y as _);

            // self.gl_ctx.clear_color(0.0, 1.0, 1.0, 1.0);
        };

        for i in 0..(30 * 5) {
            EGL.wait_native(egl::CORE_NATIVE_ENGINE).unwrap();
            EGL.wait_client().unwrap();

            unsafe {
                let loc = self.gl_ctx.get_uniform_location(program, "step")?;
                self.gl_ctx.uniform_1_i32(Some(&loc), i);

                // self.gl_ctx.clear(glow::COLOR_BUFFER_BIT);
                self.gl_ctx.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
                self.gl_ctx.finish();
                self.gl_ctx.flush();
            };

            EGL.wait_gl().unwrap();
            EGL.wait_client().unwrap();

            self.swap_buffers().unwrap();
        }

        Some(())
    }
}
