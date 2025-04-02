use rgb::AsPixels;
use wizdraw::{CubicBezier, Canvas, Color, Texture, Point, contour, SsaaConfig};

use std::time::Instant;

use std::fs::File;
use std::io::BufWriter;

const GRID_PNG: &'static [u8] = include_bytes!("../../misc/grid.png");

fn read_grid_png() -> (usize, usize, Vec<u8>) {
    let decoder = png::Decoder::new(GRID_PNG);
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).unwrap();
    buf.truncate(info.buffer_size());
    (info.width as _, info.height as _, buf)
}

fn main() {
    let w = 1280usize;
    let h = 720usize;

    let wf = w as f32;
    let hf = h as f32;

    let (gl, _window, _events_loop, _context) = {
        let sdl = sdl2::init().unwrap();
        let video = sdl.video().unwrap();
        let gl_attr = video.gl_attr();
        gl_attr.set_context_profile(sdl2::video::GLProfile::Core);
        gl_attr.set_context_version(2, 0);
        let window = video
            .window("Hello triangle!", 1024, 769)
            .opengl()
            .resizable()
            .build()
            .unwrap();
        let gl_context = window.gl_create_context().unwrap();
        let gl = unsafe {
            glow::Context::from_loader_function(|s| video.gl_get_proc_address(s) as *const _)
        };
        let event_loop = sdl.event_pump().unwrap();
        (gl, window, event_loop, gl_context)
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

    let (tex_w, tex_h, tex_p) = read_grid_png();

    let mut line = Vec::new();
    contour(path.as_slice(), 5.0, &mut line, 0.5);

    let green = Color::new(100, 200, 150, 255);
    let contour = Texture::SolidColor(green);

    canvas.clear();

    let bitmap = canvas.alloc_bitmap(tex_w, tex_h);
    canvas.fill_bitmap(bitmap, 0, 0, tex_w, tex_h, tex_p.as_pixels());

    let texture = Texture::QuadBitmap {
        top_left:  Point::new(0.410 * wf, 0.300 * hf),
        top_right: Point::new(0.590 * wf, 0.370 * hf),
        btm_left:  Point::new(0.410 * wf, 0.700 * hf),
        btm_right: Point::new(0.590 * wf, 0.630 * hf),
        bitmap,
    };

    if true {
        let then = Instant::now();
        let num = 20;
        for _ in 0..num {
            canvas.fill_cbc(&path, &Texture::Debug, SsaaConfig::None);
            canvas.fill_cbc(&path, &texture, SsaaConfig::None);
            canvas.fill_cbc(&line, &contour, SsaaConfig::None);
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

