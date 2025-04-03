use wizdraw::{CubicBezier, Canvas, Texture, Point, SsaaConfig};

use std::time::Instant;

use std::fs::File;
use std::io::BufWriter;

use khronos_egl as egl;

fn main() {
    let w = 1280usize;
    let h = 720usize;

    let wf = w as f32;
    let hf = h as f32;

    let egl = unsafe {
        let lib = libloading::Library::new("libEGL.so.1").unwrap();
        egl::DynamicInstance::<egl::EGL1_4>::load_required_from(lib).unwrap()
    };

    let display = unsafe { egl.get_display(egl::DEFAULT_DISPLAY) }.unwrap();
    egl.initialize(display).unwrap();

    println!("Display Initialized");

    let attributes = [
        egl::RED_SIZE, 5,
        egl::GREEN_SIZE, 6,
        egl::BLUE_SIZE, 5,
        egl::NONE
    ];

    let maybe_config = egl.choose_first_config(display, &attributes).unwrap();
    let config = maybe_config.expect("No compatible EGL config");

    let ctx_attr = [
        egl::CONTEXT_MAJOR_VERSION, 2,
        // egl::CONTEXT_OPENGL_PROFILE_MASK, egl::CONTEXT_OPENGL_CORE_PROFILE_BIT,
        egl::NONE
    ];

    let ctx = egl.create_context(display, config, None, &ctx_attr).unwrap();
    egl.make_current(display, None, None, Some(ctx)).unwrap();

    let gl = unsafe {
        glow::Context::from_loader_function(|s| {
            match egl.get_proc_address(s) {
                Some(ptr) => ptr as *const _,
                None => std::ptr::null(),
            }
        })
    };

    let mut canvas = wizdraw::opengl::Es2Canvas::init(gl, w as _, h as _).unwrap();

    let path = [
        CubicBezier {
            c1: Point::new(0.250 * wf, 0.500 * hf),
            c2: Point::new(0.250 * wf, 0.100 * hf),
            c3: Point::new(0.750 * wf, 0.100 * hf),
            c4: Point::new(0.750 * wf, 0.500 * hf),
        },
        CubicBezier {
            c1: Point::new(0.750 * wf, 0.500 * hf),
            c2: Point::new(0.750 * wf, 0.900 * hf),
            c3: Point::new(0.250 * wf, 0.900 * hf),
            c4: Point::new(0.250 * wf, 0.500 * hf),
        },
    ];

    canvas.clear();

    if true {
        let then = Instant::now();
        let num = 20;
        for _ in 0..num {
            canvas.fill_cbc(&path, &Texture::Debug, SsaaConfig::None);
        }
        let avg_ms = then.elapsed().as_micros() / num;
        let fps = 1000000 / avg_ms;
        println!("{:?}: {}us = {} FPS", SsaaConfig::None, avg_ms, fps);
    }

    let mut rgb565 = vec![0; 2 * w * h];
    canvas.read_rgb565(&mut rgb565);

    let mut rgba = Vec::new();

    for chunk in rgb565.chunks_exact(2) {
        if let [rg, gb] = chunk {
            rgba.push(rg & 0xf8);
            rgba.push((rg << 5) | ((gb >> 3) & 0x1c));
            rgba.push(gb & 0x1f);
            rgba.push(255);
        }
    }

    // converting to a PNG image
    let file = File::create("output.png").unwrap();
    let ref mut writer = BufWriter::new(file);
    let mut encoder = png::Encoder::new(writer, w as _, h as _);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(&rgba).unwrap();
}

