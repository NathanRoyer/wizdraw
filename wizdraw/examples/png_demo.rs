use rgb::ComponentBytes;

use std::fs::File;
use std::io::BufWriter;

mod _benchmark;

fn main() {
    let w = 1280;
    let h = 720;

    let wf = w as f32;
    let hf = h as f32;

    let mut canvas = wizdraw::cpu::Canvas::new(w, h);
    _benchmark::benchmark(&mut canvas, wf, hf);
    let pixels = canvas.pixels().as_bytes();

    // converting to a PNG image
    let file = File::create("output.png").unwrap();
    let ref mut writer = BufWriter::new(file);
    let mut encoder = png::Encoder::new(writer, w as _, h as _);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(pixels).unwrap();
}

