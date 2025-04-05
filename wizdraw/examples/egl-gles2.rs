use std::io::BufWriter;
use std::fs::File;

use khronos_egl as egl;

mod _benchmark;

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

    let mut canvas = wizdraw::gles2::Canvas::init(gl, w as _, h as _).unwrap();

    _benchmark::benchmark(&mut canvas, wf, hf);

    let mut rgba5551 = vec![0; 2 * w * h];
    canvas.read_rgba5551(&mut rgba5551);

    let mut rgba = Vec::new();

    for chunk in rgba5551.chunks_exact(2) {
        if let [rg, gba] = chunk {
            let word = u16::from_le_bytes([*rg, *gba]);
            rgba.push(((word >> 8) & 0xf8) as u8);
            rgba.push(((word >> 3) & 0xf8) as u8);
            rgba.push(((word << 2) & 0xf8) as u8);
            rgba.push((word as u8 & 1) * 255);
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

