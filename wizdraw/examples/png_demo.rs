use rgb::{ComponentBytes, AsPixels};
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
    let w = 1280;
    let h = 720;

    let wf = w as f32;
    let hf = h as f32;

    // let mut canvas_simd = wizdraw::cpu::Canvas::new_simd(w, h);
    let mut canvas_seq = wizdraw::cpu::Canvas::new(w, h);

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

    for ssaa in [SsaaConfig::None, SsaaConfig::X2, SsaaConfig::X4] {
        for canvas in [&mut canvas_seq/*, &mut canvas_simd*/] {
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
                    canvas.fill_cbc(&path, &Texture::Debug, ssaa);
                    canvas.fill_cbc(&path, &texture, ssaa);
                    canvas.fill_cbc(&line, &contour, ssaa);
                }
                let avg_ms = then.elapsed().as_micros() / num;
                let fps = 1000000 / avg_ms;
                println!("{:?}: {}us = {} FPS", ssaa, avg_ms, fps);
            }
        }
    }

    let pixels = canvas_seq.pixels().as_bytes();

    // converting to a PNG image
    let file = File::create("output.png").unwrap();
    let ref mut writer = BufWriter::new(file);
    let mut encoder = png::Encoder::new(writer, w as _, h as _);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(pixels).unwrap();
}

