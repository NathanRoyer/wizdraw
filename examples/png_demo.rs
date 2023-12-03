use rgb::ComponentBytes;
use vek::vec::Vec2;
use wizdraw::{CubicBezier, rainbow, SsaaConfig};
use std::time::Instant;

fn main() {
    let w = 1000;
    let h = 1000;

    let wf = w as f32;
    let hf = h as f32;

    let path = [
        CubicBezier {
            c1: Vec2::new(0.250 * wf, 0.600 * hf),
            c2: Vec2::new(0.250 * wf, 0.250 * hf),
            c3: Vec2::new(0.750 * wf, 0.250 * hf),
            c4: Vec2::new(0.750 * wf, 0.600 * hf),
        },
        CubicBezier {
            c1: Vec2::new(0.750 * wf, 0.600 * hf),
            c2: Vec2::new(0.750 * wf, 0.400 * hf),
            c3: Vec2::new(0.250 * wf, 0.400 * hf),
            c4: Vec2::new(0.250 * wf, 0.600 * hf),
        },
    ];

    let mut canvas = wizdraw::Canvas::new(w, h);

    let configs = [SsaaConfig::None, SsaaConfig::X2, SsaaConfig::X4, SsaaConfig::X8, SsaaConfig::X16];
    for (simd, dbg) in [(false, "SEQ"), (true, "SIMD")] {
        for config in configs {
            let shots = 120;
            let before = Instant::now();
            for _ in 0..shots {
                canvas.clear();
                canvas.fill(&path, rainbow, simd, config);
            }
            let elapsed = before.elapsed().as_millis() as f32;
            let avg_frame_time = elapsed / (shots as f32);

            print!("[{}, {:?}] ", dbg, config);
            println!("frame time: {}ms = FPS {}", avg_frame_time.round(), (1000.0 / avg_frame_time).round());
        }
    }

    // converting the mask to a PNG image

    use std::fs::File;
    use std::io::BufWriter;

    let pixels = canvas.pixels().as_bytes();

    let file = File::create("output.png").unwrap();
    let ref mut writer = BufWriter::new(file);
    let mut encoder = png::Encoder::new(writer, w as _, h as _);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(pixels).unwrap();
}

