use rgb::{ComponentBytes, AsPixels};
use wizdraw::{CubicBezier, Canvas, Color, Texture, Point, util, SsaaConfig};

use std::time::Instant;

use std::fs::File;
use std::io::BufWriter;

fn read_png(path: &str) -> (usize, usize, Vec<u8>) {
    let decoder = png::Decoder::new(File::open(path).unwrap());
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).unwrap();
    buf.truncate(info.buffer_size());
    (info.width as _, info.height as _, buf)
}

fn main() {
    let w = 1000;
    let h = 1000;

    let wf = w as f32;
    let hf = h as f32;

    let mut canvas_simd = wizdraw::cpu::Canvas::new_simd(w, h);
    let mut canvas_seq = wizdraw::cpu::Canvas::new_seq(w, h);

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

    let (tex_w, tex_h, tex_p) = read_png("/home/bitsneak/Pictures/discord-pp.png");

    let mut line = Vec::new();
    util::contour(path.as_slice(), 5.0, &mut line, 1.0);

    let myrtle = Color::new(255, 100, 100, 255);
    let contour = Texture::SolidColor(myrtle);

    for ssaa in [SsaaConfig::X4] {
        for canvas in [&mut canvas_seq, &mut canvas_simd] {
            canvas.clear();

            let bitmap = canvas.alloc_bitmap(tex_w, tex_h);
            canvas.fill_bitmap(bitmap, 0, 0, tex_w, tex_h, tex_p.as_pixels());

            let texture = Texture::QuadBitmap {
                top_left:  Point::new(0.400 * wf, 0.400 * hf),
                top_right: Point::new(0.600 * wf, 0.350 * hf),
                btm_left:  Point::new(0.400 * wf, 0.600 * hf),
                btm_right: Point::new(0.600 * wf, 0.650 * hf),
                bitmap,
            };

            if true {
                let then = Instant::now();
                let num = 100;
                for _ in 0..100 {
                    canvas.fill_cbc(&path, &Texture::SolidColor(Color::new(100, 50, 50, 255)), false, ssaa);
                    canvas.fill_cbc(&path, &texture, false, ssaa);
                    canvas.fill_cbc(&line, &contour, false, ssaa);
                }
                println!("{:?}: {}ms", ssaa, then.elapsed().as_millis() / num);
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

